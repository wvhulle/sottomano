use std::{
    io,
    path::{Path, PathBuf},
};

use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::{
        UnixStream,
        unix::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::mpsc,
};

const CAPABILITIES_COMMAND: &[u8] = b"{\"execute\":\"qmp_capabilities\"}\n";
const QUERY_STATUS_COMMAND: &[u8] = b"{\"execute\":\"query-status\"}\n";
const POWERDOWN_COMMAND: &[u8] = b"{\"execute\":\"system_powerdown\"}\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestRunState {
    Running,
    Paused,
    ShuttingDown,
    Unreported,
}

#[derive(Debug, Clone)]
pub struct QmpPowerdownSender {
    socket_path: PathBuf,
    powerdown_requests: mpsc::UnboundedSender<()>,
}

impl QmpPowerdownSender {
    pub(crate) fn new(socket_path: PathBuf, powerdown_requests: mpsc::UnboundedSender<()>) -> Self {
        Self {
            socket_path,
            powerdown_requests,
        }
    }

    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn detached_fixture(socket_path: PathBuf) -> Self {
        Self::new(socket_path, mpsc::unbounded_channel().0)
    }

    pub fn request_powerdown(&self) -> Result<(), mpsc::error::SendError<()>> {
        self.powerdown_requests.send(())
    }

    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

impl PartialEq for QmpPowerdownSender {
    fn eq(&self, other: &Self) -> bool {
        self.socket_path == other.socket_path
            && self
                .powerdown_requests
                .same_channel(&other.powerdown_requests)
    }
}

fn run_state_of_status(status: &str) -> GuestRunState {
    match status {
        "running" => GuestRunState::Running,
        "shutdown" => GuestRunState::ShuttingDown,
        _ => GuestRunState::Paused,
    }
}

fn run_state_of_event(event_name: &str) -> Option<GuestRunState> {
    match event_name {
        "STOP" | "SUSPEND" => Some(GuestRunState::Paused),
        "RESUME" | "RESET" | "WAKEUP" => Some(GuestRunState::Running),
        "POWERDOWN" | "SHUTDOWN" => Some(GuestRunState::ShuttingDown),
        _ => None,
    }
}

#[must_use]
pub(crate) fn run_state_of_message(message_line: &str) -> Option<GuestRunState> {
    let message: Value = serde_json::from_str(message_line).ok()?;
    if let Some(event_name) = message.get("event").and_then(Value::as_str) {
        return run_state_of_event(event_name);
    }
    message
        .get("return")?
        .get("status")
        .and_then(Value::as_str)
        .map(run_state_of_status)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QmpMessageKind {
    CommandReply,
    AsynchronousEvent,
}

impl From<&str> for QmpMessageKind {
    fn from(message_line: &str) -> Self {
        match serde_json::from_str::<Value>(message_line) {
            Ok(message) if message.get("return").is_some() || message.get("error").is_some() => {
                QmpMessageKind::CommandReply
            }
            _ => QmpMessageKind::AsynchronousEvent,
        }
    }
}

struct QmpSession {
    replies: Lines<BufReader<OwnedReadHalf>>,
    commands: OwnedWriteHalf,
}

impl QmpSession {
    async fn negotiated(socket_path: &Path) -> io::Result<Self> {
        let (read_half, commands) = UnixStream::connect(socket_path).await?.into_split();
        let mut session = Self {
            replies: BufReader::new(read_half).lines(),
            commands,
        };
        session.next_message().await?;
        session.commands.write_all(CAPABILITIES_COMMAND).await?;
        session.reply_skipping_events().await?;
        Ok(session)
    }

    async fn next_message(&mut self) -> io::Result<String> {
        self.replies
            .next_line()
            .await?
            .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))
    }

    async fn reply_skipping_events(&mut self) -> io::Result<String> {
        loop {
            let message_line = self.next_message().await?;
            if QmpMessageKind::from(message_line.as_str()) == QmpMessageKind::CommandReply {
                return Ok(message_line);
            }
        }
    }
}

pub(crate) async fn follow_qmp_run_state(
    socket_path: &Path,
    mut powerdown_requests: mpsc::UnboundedReceiver<()>,
    mut on_run_state: impl FnMut(GuestRunState),
) -> io::Result<()> {
    let mut session = QmpSession::negotiated(socket_path).await?;
    session.commands.write_all(QUERY_STATUS_COMMAND).await?;
    on_run_state(
        run_state_of_message(&session.reply_skipping_events().await?)
            .unwrap_or(GuestRunState::Unreported),
    );
    let QmpSession { replies, commands } = &mut session;
    loop {
        tokio::select! {
            message_line = replies.next_line() => {
                let message_line = message_line?
                    .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;
                if let Some(run_state) = run_state_of_message(&message_line) {
                    on_run_state(run_state);
                }
            }
            powerdown_request = powerdown_requests.recv() => {
                if powerdown_request.is_none() {
                    return Ok(());
                }
                commands.write_all(POWERDOWN_COMMAND).await?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process, time::Duration};

    use tokio::{net::UnixListener, sync::mpsc::unbounded_channel, time::timeout};

    use super::*;

    #[test]
    fn status_replies_and_lifecycle_events_map_to_run_states() {
        assert_eq!(
            run_state_of_message(r#"{"return": {"status": "running", "running": true}}"#),
            Some(GuestRunState::Running)
        );
        assert_eq!(
            run_state_of_message(r#"{"return": {"status": "paused", "running": false}}"#),
            Some(GuestRunState::Paused)
        );
        assert_eq!(
            run_state_of_message(
                r#"{"timestamp": {"seconds": 1, "microseconds": 2}, "event": "STOP"}"#
            ),
            Some(GuestRunState::Paused)
        );
        assert_eq!(
            run_state_of_message(r#"{"event": "POWERDOWN"}"#),
            Some(GuestRunState::ShuttingDown)
        );
        assert_eq!(
            run_state_of_message(r#"{"event": "NIC_RX_FILTER_CHANGED"}"#),
            None
        );
        assert_eq!(run_state_of_message(r#"{"return": {}}"#), None);
    }

    #[tokio::test]
    async fn the_follower_reports_state_and_forwards_a_powerdown() {
        let socket_directory = env::temp_dir().join(format!("sottomano-qmp-{}", process::id()));
        fs::create_dir_all(&socket_directory).expect("socket directory is created");
        let socket_path = socket_directory.join("guest.qmp");
        let _ = fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).expect("fake qmp binds");
        let fake_qemu = tokio::spawn(async move {
            let (connection, _) = listener.accept().await.expect("follower connects");
            let (read_half, mut write_half) = connection.into_split();
            let mut commands = BufReader::new(read_half).lines();
            write_half
                .write_all(b"{\"QMP\": {\"version\": {}, \"capabilities\": []}}\n")
                .await
                .expect("greeting is sent");
            let mut received_commands = Vec::new();
            while let Ok(Some(command_line)) = commands.next_line().await {
                let reply: &[u8] = if command_line.contains("query-status") {
                    b"{\"return\": {\"status\": \"running\", \"running\": true}}\n{\"event\": \"STOP\"}\n"
                } else {
                    b"{\"return\": {}}\n"
                };
                write_half.write_all(reply).await.expect("reply is sent");
                received_commands.push(command_line.clone());
                if command_line.contains("system_powerdown") {
                    break;
                }
            }
            received_commands
        });
        let (powerdown_sender, powerdown_receiver) = unbounded_channel();
        let (run_state_sender, mut run_state_receiver) = unbounded_channel();
        let follower = tokio::spawn({
            let socket_path = socket_path.clone();
            async move {
                follow_qmp_run_state(&socket_path, powerdown_receiver, |run_state| {
                    let _ = run_state_sender.send(run_state);
                })
                .await
            }
        });
        assert_eq!(
            run_state_receiver.recv().await,
            Some(GuestRunState::Running)
        );
        assert_eq!(run_state_receiver.recv().await, Some(GuestRunState::Paused));
        powerdown_sender.send(()).expect("follower is listening");
        let received_commands = timeout(Duration::from_secs(5), fake_qemu)
            .await
            .expect("fake qemu finishes")
            .expect("fake qemu does not panic");
        assert!(
            received_commands
                .last()
                .is_some_and(|command| command.contains("system_powerdown"))
        );
        follower.abort();
        let _ = fs::remove_dir_all(&socket_directory);
    }
}
