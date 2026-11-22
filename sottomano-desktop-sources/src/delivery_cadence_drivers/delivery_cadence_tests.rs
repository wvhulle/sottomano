use std::fmt::{Display, Formatter, Result as FmtResult};

use chrono::{Local, TimeZone};
use futures::{StreamExt, channel::mpsc, future::pending, stream};
use tokio::time::timeout;

use super::{
    periodic_polling_deliveries::{SHORTEST_POLL_PERIOD, until_next_boundary},
    *,
};

#[derive(Debug, Clone, PartialEq)]
struct CountedSnapshot {
    snapshot_count: u32,
}

#[derive(Debug, Clone)]
struct NeverFails;

impl Display for NeverFails {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(formatter, "never fails")
    }
}

impl SnapshotSource for CountedSnapshot {
    type Error = NeverFails;
    type StreamInputs = ();

    const NAME: &'static str = "Counted snapshot";

    fn delivery_stream(_inputs: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::empty()
    }
}

fn delivered_cadence(delivery: &SnapshotDelivery<CountedSnapshot>) -> DeliveryCadence {
    let SnapshotDelivery::SnapshotDelivered {
        delivery_cadence, ..
    } = delivery
    else {
        panic!("expected a delivered snapshot, got {delivery:?}");
    };
    *delivery_cadence
}

#[test]
fn a_zero_poll_period_is_raised_to_the_shortest_period() {
    assert_eq!(
        PeriodicPolling::every(Duration::ZERO).poll_period,
        SHORTEST_POLL_PERIOD
    );
    assert_eq!(
        PeriodicPolling::every(Duration::from_secs(5)).poll_period,
        Duration::from_secs(5)
    );
}

#[tokio::test(start_paused = true)]
async fn polled_deliveries_carry_their_period_and_repeats_arrive_unchanged() {
    let polling = PeriodicPolling::every(Duration::from_secs(3));
    let deliveries: Vec<_> = polling
        .poll_deliveries(|| ready(Ok(CountedSnapshot { snapshot_count: 7 })))
        .take(2)
        .collect()
        .await;
    assert_eq!(
        delivered_cadence(&deliveries[0]),
        DeliveryCadence::PeriodicPolling(polling)
    );
    assert!(matches!(deliveries[1], SnapshotDelivery::SnapshotUnchanged));
}

#[tokio::test(start_paused = true)]
async fn repeated_unavailability_is_never_reported_unchanged() {
    let deliveries: Vec<SnapshotDelivery<CountedSnapshot>> = stream::iter([
        SnapshotDelivery::SourceUnavailable,
        SnapshotDelivery::SourceUnavailable,
    ])
    .repeats_as_unchanged()
    .collect()
    .await;
    assert!(
        deliveries
            .iter()
            .all(|delivery| matches!(delivery, SnapshotDelivery::SourceUnavailable))
    );
}

#[tokio::test(start_paused = true)]
async fn spaced_rereads_drain_burst_triggers_and_wait_the_gap() {
    let minimum_trigger_gap = Duration::from_mins(1);
    let started = TokioClockInstant::now();
    let (trigger_sender, trigger_receiver) = mpsc::unbounded::<Result<(), &'static str>>();
    let mut read_count = 0_u32;
    let mut rereads = Box::pin(trigger_receiver.paced_rereads(
        minimum_trigger_gap,
        move || {
            read_count += 1;
            ready(Ok::<_, &'static str>(read_count))
        },
        |_| RereadSchedule::WaitForTrigger,
    ));
    assert_eq!(rereads.next().await, Some(Ok(1)));
    assert_eq!(started.elapsed(), Duration::ZERO);
    trigger_sender.unbounded_send(Ok(())).unwrap();
    trigger_sender.unbounded_send(Ok(())).unwrap();
    assert_eq!(rereads.next().await, Some(Ok(2)));
    assert_eq!(started.elapsed(), minimum_trigger_gap);
    assert!(
        timeout(minimum_trigger_gap * 10, rereads.next())
            .await
            .is_err()
    );
}

#[tokio::test(start_paused = true)]
async fn triggers_drain_while_a_read_is_pending() {
    let (trigger_sender, trigger_receiver) = mpsc::channel::<Result<(), &'static str>>(4);
    let mut rereads =
        Box::pin(trigger_receiver.triggered_rereads(pending::<Result<(), &'static str>>));
    let pending_read = tokio::spawn(async move { rereads.next().await });
    let sent_triggers = timeout(
        Duration::from_secs(1),
        stream::repeat(Ok(()))
            .take(200)
            .map(Ok)
            .forward(trigger_sender),
    )
    .await;
    pending_read.abort();
    assert!(
        matches!(sent_triggers, Ok(Ok(()))),
        "triggers backed up while a read was pending"
    );
}

#[tokio::test(start_paused = true)]
async fn a_trigger_failure_is_delivered_before_the_next_read() {
    let (trigger_sender, trigger_receiver) = mpsc::unbounded::<Result<(), &'static str>>();
    let mut read_count = 0_u32;
    let mut rereads = Box::pin(trigger_receiver.triggered_rereads(move || {
        read_count += 1;
        ready(Ok::<_, &'static str>(read_count))
    }));
    assert_eq!(rereads.next().await, Some(Ok(1)));
    trigger_sender.unbounded_send(Err("watch stopped")).unwrap();
    trigger_sender.unbounded_send(Ok(())).unwrap();
    assert_eq!(rereads.next().await, Some(Err("watch stopped")));
    assert_eq!(rereads.next().await, Some(Ok(2)));
}

#[tokio::test(start_paused = true)]
async fn triggered_deliveries_recheck_on_the_polling_period() {
    let recheck = PeriodicPolling::every(Duration::from_mins(15));
    let mut deliveries = Box::pin(
        recheck.triggered_deliveries(stream::pending::<Result<(), NeverFails>>(), || {
            ready(Ok(CountedSnapshot { snapshot_count: 1 }))
        }),
    );
    let first = deliveries.next().await.unwrap();
    assert_eq!(
        delivered_cadence(&first),
        DeliveryCadence::LivePushWithRecheck(recheck)
    );
    let started_waiting = TokioClockInstant::now();
    deliveries.next().await.unwrap();
    assert_eq!(started_waiting.elapsed(), recheck.poll_period);
}

#[test]
fn boundary_delay_completes_the_period() {
    let now = Local.timestamp_opt(1_000, 250_000_000).unwrap();
    assert_eq!(
        until_next_boundary(now, Duration::from_secs(1)),
        Duration::from_millis(750)
    );
    assert_eq!(
        until_next_boundary(now, Duration::from_secs(60)),
        Duration::from_millis(19750)
    );
}

#[test]
fn exact_boundary_waits_a_full_period() {
    let now = Local.timestamp_opt(1_000, 0).unwrap();
    assert_eq!(
        until_next_boundary(now, Duration::from_secs(1)),
        Duration::from_secs(1)
    );
}
