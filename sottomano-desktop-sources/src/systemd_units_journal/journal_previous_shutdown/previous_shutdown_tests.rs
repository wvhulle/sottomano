use super::*;
use crate::system_protocol_clients::journal_follow::journal_record;

#[test]
fn stops_pair_by_job_and_a_stop_that_never_ended_ranks_first() {
    let at = |secs: u64| SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
    let job = |unit: &str, job_id: u32| StopJob {
        unit: OwningUnit::System(unit.to_owned()),
        job_id,
    };
    let events = [
        ShutdownEvent::Stopping {
            job: job("restarted.service", 1),
            at: at(5),
        },
        ShutdownEvent::Stopped {
            job: job("restarted.service", 1),
            at: at(6),
            job_result: JobResult::Done,
        },
        ShutdownEvent::Began { at: at(100) },
        ShutdownEvent::Stopping {
            job: job("nfs.mount", 7),
            at: at(101),
        },
        ShutdownEvent::Stopping {
            job: job("docker.service", 8),
            at: at(101),
        },
        ShutdownEvent::Stopped {
            job: job("docker.service", 8),
            at: at(109),
            job_result: JobResult::Done,
        },
    ];
    let shutdown =
        LastShutdown::assemble(events, at(190), Duration::from_secs(5)).expect("a shutdown");
    assert_eq!(shutdown.duration, Duration::from_secs(90));
    assert_eq!(
        shutdown.slowest_stops,
        [
            UnitStop {
                unit: OwningUnit::System("nfs.mount".to_owned()),
                outcome: StopOutcome::Unfinished,
            },
            UnitStop {
                unit: OwningUnit::System("docker.service".to_owned()),
                outcome: StopOutcome::Finished {
                    duration: Duration::from_secs(8),
                    job_result: JobResult::Done,
                    severity: Severity::Warning,
                },
            },
        ]
    );
}

#[test]
fn a_stopped_unit_record_decodes_its_job_and_result() {
    let record = decode_record(journal_record([
        ("MESSAGE_ID", "9d1aaa27d60140bd96365438aad20286"),
        ("UNIT", "docker.service"),
        ("JOB_ID", "8"),
        ("JOB_RESULT", "timeout"),
    ]))
    .expect("a well-formed stop record");
    let Some(ShutdownEvent::Stopped {
        job, job_result, ..
    }) = ShutdownEvent::recorded_at(record, SystemTime::UNIX_EPOCH)
    else {
        panic!("a stopped unit event");
    };
    assert_eq!(
        job,
        StopJob {
            unit: OwningUnit::System("docker.service".to_owned()),
            job_id: 8,
        }
    );
    assert_eq!(job_result, JobResult::Timeout);
}
