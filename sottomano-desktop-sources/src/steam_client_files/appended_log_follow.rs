use std::{
    fs::File,
    future::ready,
    io::{self, Read, Seek, SeekFrom},
    mem,
    path::PathBuf,
    time::Duration,
};

use futures::Stream;

use crate::{
    delivery_cadence_drivers::{
        CarriedRead, ChangeTriggerStreams, ChangeWatchFailure, RereadSchedule,
    },
    system_protocol_clients::inotify_directory_changes::{self, WatchTarget},
};

const MINIMUM_REREAD_GAP: Duration = Duration::from_millis(250);

#[derive(Debug)]
struct AppendedLogFollow {
    log_path: PathBuf,
    read_offset: u64,
    unfinished_bytes: Vec<u8>,
}

impl AppendedLogFollow {
    fn appended_lines(&mut self) -> io::Result<Vec<String>> {
        let mut log_file = match File::open(&self.log_path) {
            Ok(log_file) => log_file,
            Err(open_failure) if open_failure.kind() == io::ErrorKind::NotFound => {
                self.read_offset = 0;
                self.unfinished_bytes.clear();
                return Ok(Vec::new());
            }
            Err(open_failure) => return Err(open_failure),
        };
        let current_length = log_file.metadata()?.len();
        if current_length < self.read_offset {
            self.read_offset = 0;
            self.unfinished_bytes.clear();
        }
        log_file.seek(SeekFrom::Start(self.read_offset))?;
        let appended_length = log_file.read_to_end(&mut self.unfinished_bytes)?;
        self.read_offset += appended_length as u64;
        let Some(last_newline) = self
            .unfinished_bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
        else {
            return Ok(Vec::new());
        };
        let carried_tail = self.unfinished_bytes.split_off(last_newline + 1);
        let finished_bytes = mem::replace(&mut self.unfinished_bytes, carried_tail);
        Ok(finished_bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                str::from_utf8(line)
                    .inspect_err(|decode_failure| {
                        tracing::debug!(
                            "steam log {}: skipped a line that is not UTF-8: {decode_failure}",
                            self.log_path.display()
                        );
                    })
                    .ok()
            })
            .map(|line| line.trim_end_matches('\r').to_owned())
            .collect())
    }
}

pub fn appended_log_lines(
    log_path: PathBuf,
) -> impl Stream<Item = Result<Vec<String>, ChangeWatchFailure>> + Send {
    let follow = AppendedLogFollow {
        log_path: log_path.clone(),
        read_offset: 0,
        unfinished_bytes: Vec::new(),
    };
    inotify_directory_changes::changes(vec![WatchTarget::File(log_path)]).paced_rereads_carrying(
        MINIMUM_REREAD_GAP,
        follow,
        |mut follow| {
            let snapshot_fetch = follow
                .appended_lines()
                .map_err(ChangeWatchFailure::FileWatch);
            ready(CarriedRead {
                carried_state: follow,
                snapshot_fetch,
            })
        },
        |_| RereadSchedule::WaitForTrigger,
    )
}

#[cfg(test)]
mod tests {
    use std::{env, fs, io::Write, process};

    use super::*;

    fn scratch_directory(test_name: &str) -> PathBuf {
        let directory =
            env::temp_dir().join(format!("sottomano-steam-log-{test_name}-{}", process::id()));
        fs::create_dir_all(&directory).expect("scratch directory");
        directory
    }

    fn fresh_log_follow(log_path: PathBuf) -> AppendedLogFollow {
        AppendedLogFollow {
            log_path,
            read_offset: 0,
            unfinished_bytes: Vec::new(),
        }
    }

    #[test]
    fn only_finished_lines_are_yielded_and_the_tail_waits() {
        let log_path = scratch_directory("tail").join("content_log.txt");
        fs::write(&log_path, "first\nsecond\nthi").expect("seed log");
        let mut follow = fresh_log_follow(log_path.clone());
        assert_eq!(follow.appended_lines().expect("read"), ["first", "second"]);
        let mut appender = fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("append handle");
        appender.write_all(b"rd\nfourth\n").expect("append");
        assert_eq!(follow.appended_lines().expect("read"), ["third", "fourth"]);
        assert!(follow.appended_lines().expect("read").is_empty());
    }

    #[test]
    fn a_truncated_log_is_read_again_from_the_start() {
        let log_path = scratch_directory("rotation").join("controller.txt");
        fs::write(&log_path, "old line one\nold line two\n").expect("seed log");
        let mut follow = fresh_log_follow(log_path.clone());
        follow.appended_lines().expect("read");
        fs::write(&log_path, "new\n").expect("rotate log");
        assert_eq!(follow.appended_lines().expect("read"), ["new"]);
    }

    #[test]
    fn a_missing_log_yields_nothing() {
        let mut follow = fresh_log_follow(scratch_directory("missing").join("absent.txt"));
        assert!(follow.appended_lines().expect("read").is_empty());
    }
}
