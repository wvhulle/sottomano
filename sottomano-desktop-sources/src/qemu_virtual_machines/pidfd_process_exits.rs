use std::io;

use rustix::process::{Pid, PidfdFlags, pidfd_open};
use tokio::io::{Interest, unix::AsyncFd};

#[allow(deprecated)]
pub(crate) async fn process_exit(pid: i32) -> io::Result<()> {
    let watched_pid =
        Pid::from_raw(pid).ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let process_descriptor = pidfd_open(watched_pid, PidfdFlags::NONBLOCK)?;
    let exit_readiness = AsyncFd::with_interest(process_descriptor, Interest::READABLE)?;
    exit_readiness.readable().await?.retain_ready();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{process::Command, time::timeout};

    use super::*;

    #[tokio::test]
    async fn an_exiting_child_is_noticed_without_polling() {
        let mut child = Command::new("sleep")
            .arg("0.1")
            .spawn()
            .expect("sleep starts");
        let child_pid = i32::try_from(child.id().expect("child has a pid")).expect("pid fits");
        let noticed = timeout(Duration::from_secs(5), process_exit(child_pid)).await;
        child.wait().await.expect("child is reaped");
        assert!(matches!(noticed, Ok(Ok(()))));
    }

    #[tokio::test]
    async fn a_pid_that_is_not_running_cannot_be_watched() {
        assert!(process_exit(i32::MAX).await.is_err());
    }
}
