use std::{future::ready, process::Stdio};

use futures::{Stream, StreamExt, stream};
use serde::Deserialize;
use tokio::{
    io::{AsyncBufReadExt, BufReader, Lines},
    process::{Child, ChildStdout, Command},
};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, pushed_delivery_stream,
    stream_operators::StreamOperators, system_protocol_clients::shell_command_output,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DictationActivity {
    Idle,
    Recording,
    Streaming,
    Transcribing,
    Outputting,
    #[serde(other)]
    Stopped,
}

impl DictationActivity {
    #[must_use]
    pub const fn listening(self) -> bool {
        matches!(
            self,
            DictationActivity::Recording | DictationActivity::Streaming
        )
    }

    #[must_use]
    pub const fn busy(self) -> bool {
        matches!(
            self,
            DictationActivity::Recording
                | DictationActivity::Streaming
                | DictationActivity::Transcribing
                | DictationActivity::Outputting
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct RecognizerSetup {
    pub model: Option<String>,
    pub device: Option<String>,
    pub backend: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VoxtypeDictation {
    #[serde(rename = "alt")]
    pub activity: DictationActivity,
    #[serde(flatten)]
    pub setup: RecognizerSetup,
}

impl ReadingSource for VoxtypeDictation {
    type Error = shell_command_output::Error;
    type StreamInputs = ();

    const NAME: &'static str = "voxtype";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream((): ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(shell_command_output::exists("voxtype")).while_held(
            || pushed_delivery_stream(ready(followed_status_changes())),
            SourceDelivery::Absent,
        )
    }
}

struct StatusFollower {
    _process: Child,
    lines: Lines<BufReader<ChildStdout>>,
}

fn followed_status_changes() -> Result<
    impl Stream<Item = Result<VoxtypeDictation, shell_command_output::Error>> + Send,
    shell_command_output::Error,
> {
    let spawn_failure = |detail: String| shell_command_output::Error::Spawn {
        program: "voxtype".to_owned(),
        detail,
    };
    let mut process = Command::new("voxtype")
        .args(["status", "--follow", "--format", "json", "--extended"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| spawn_failure(error.to_string()))?;
    let stdout = process
        .stdout
        .take()
        .ok_or_else(|| spawn_failure("no stdout pipe".to_owned()))?;
    let follower = StatusFollower {
        _process: process,
        lines: BufReader::new(stdout).lines(),
    };
    Ok(stream::unfold(follower, |mut follower| async move {
        let line = follower.lines.next_line().await.ok().flatten()?;
        Some((line, follower))
    })
    .filter_map(|line| ready(serde_json::from_str(&line).ok().map(Ok))))
}

#[must_use]
pub fn toggle_recording() -> bool {
    shell_command_output::blocking::run("voxtype", &["record", "toggle"])
}

#[must_use]
pub fn cancel_recording() -> bool {
    shell_command_output::blocking::run("voxtype", &["record", "cancel"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_status_line_yields_activity_and_setup() {
        let line = r#"{"text": "S", "alt": "streaming", "class": "streaming", "tooltip": "Streaming live...", "model": "large-v3-turbo", "device": "default", "backend": "GPU (CUDA)"}"#;
        let dictation: VoxtypeDictation = serde_json::from_str(line).unwrap();
        assert_eq!(dictation.activity, DictationActivity::Streaming);
        assert_eq!(dictation.setup.model.as_deref(), Some("large-v3-turbo"));
        assert_eq!(dictation.setup.backend.as_deref(), Some("GPU (CUDA)"));
    }

    #[test]
    fn unknown_or_stopped_states_read_as_stopped() {
        for alt in ["stopped", "bogus"] {
            let line =
                format!(r#"{{"text": "X", "alt": "{alt}", "class": "{alt}", "tooltip": ""}}"#);
            let dictation: VoxtypeDictation = serde_json::from_str(&line).unwrap();
            assert_eq!(dictation.activity, DictationActivity::Stopped);
            assert_eq!(dictation.setup, RecognizerSetup::default());
        }
    }
}
