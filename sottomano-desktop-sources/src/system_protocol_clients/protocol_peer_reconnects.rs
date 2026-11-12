use std::{future::Future, time::Duration};

use futures::{Stream, StreamExt, stream};
use tokio::time::sleep;

const FIRST_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const LONGEST_RECONNECT_DELAY: Duration = Duration::from_mins(5);
const LONGEST_DOUBLING_COUNT: u32 = 16;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ReconnectBackoff {
    failures_since_delivery: u32,
}

impl ReconnectBackoff {
    fn next_delay(&mut self) -> Duration {
        let doubling_count = self.failures_since_delivery.min(LONGEST_DOUBLING_COUNT);
        self.failures_since_delivery = self.failures_since_delivery.saturating_add(1);
        let unjittered_delay = FIRST_RECONNECT_DELAY
            .saturating_mul(1 << doubling_count)
            .min(LONGEST_RECONNECT_DELAY);
        unjittered_delay
            .mul_f64(0.75 + fastrand::f64() * 0.5)
            .min(LONGEST_RECONNECT_DELAY)
    }

    fn delivered(&mut self) {
        self.failures_since_delivery = 0;
    }
}

enum SubscriptionPhase<ConnectedStream> {
    Connecting,
    Streaming(ConnectedStream),
    Reconnecting,
}

struct SubscriptionProgress<ConnectedStream, SubscribeFn> {
    phase: SubscriptionPhase<ConnectedStream>,
    backoff: ReconnectBackoff,
    subscribe: SubscribeFn,
}

pub fn subscribed<SubscribeInputs, ConnectedStream, SubscribeFuture, ItemValue, Failure>(
    subscribe_inputs: SubscribeInputs,
    mut subscribe: impl FnMut(SubscribeInputs) -> SubscribeFuture + Send,
) -> impl Stream<Item = Result<ItemValue, Failure>> + Send
where
    SubscribeInputs: Clone + Send,
    ConnectedStream: Stream<Item = Result<ItemValue, Failure>> + Send,
    SubscribeFuture: Future<Output = Result<ConnectedStream, Failure>> + Send,
    ItemValue: Send,
    Failure: Send,
{
    let subscribe_again = move || subscribe(subscribe_inputs.clone());
    stream::unfold(
        SubscriptionProgress {
            phase: SubscriptionPhase::Connecting,
            backoff: ReconnectBackoff::default(),
            subscribe: subscribe_again,
        },
        |mut progress| async move {
            loop {
                match progress.phase {
                    SubscriptionPhase::Connecting | SubscriptionPhase::Reconnecting => {
                        if matches!(progress.phase, SubscriptionPhase::Reconnecting) {
                            sleep(progress.backoff.next_delay()).await;
                        }
                        match (progress.subscribe)().await {
                            Ok(connected) => {
                                progress.phase = SubscriptionPhase::Streaming(Box::pin(connected));
                            }
                            Err(failure) => {
                                progress.phase = SubscriptionPhase::Reconnecting;
                                return Some((Err(failure), progress));
                            }
                        }
                    }
                    SubscriptionPhase::Streaming(mut connected) => {
                        match connected.as_mut().next().await {
                            Some(delivered) => {
                                progress.backoff.delivered();
                                progress.phase = SubscriptionPhase::Streaming(connected);
                                return Some((delivered, progress));
                            }
                            None => progress.phase = SubscriptionPhase::Reconnecting,
                        }
                    }
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use std::{
        pin::pin,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        vec::IntoIter,
    };

    use tokio::time;

    use super::*;

    fn within_jitter_of(elapsed: Duration, unjittered_delay: Duration) -> bool {
        elapsed >= unjittered_delay.mul_f64(0.75) && elapsed <= unjittered_delay.mul_f64(1.25)
    }

    #[test]
    fn reconnect_delays_double_up_to_the_cap_and_reset_after_a_delivery() {
        let mut backoff = ReconnectBackoff::default();
        let first_delays: Vec<Duration> = std::iter::repeat_with(|| backoff.next_delay())
            .take(3)
            .collect();
        assert!(within_jitter_of(first_delays[0], Duration::from_secs(1)));
        assert!(within_jitter_of(first_delays[1], Duration::from_secs(2)));
        assert!(within_jitter_of(first_delays[2], Duration::from_secs(4)));
        let capped_delay = std::iter::repeat_with(|| backoff.next_delay())
            .take(40)
            .last()
            .unwrap();
        assert!(capped_delay <= LONGEST_RECONNECT_DELAY);
        assert!(within_jitter_of(capped_delay, LONGEST_RECONNECT_DELAY));
        backoff.delivered();
        assert!(within_jitter_of(
            backoff.next_delay(),
            Duration::from_secs(1)
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn a_subscribed_stream_reconnects_after_its_source_ends() {
        let started = time::Instant::now();
        let mut planned_attempts = std::vec![
            stream::iter(vec![Ok(1)]),
            stream::iter(vec![Ok(0), Err("source ended")]),
        ];
        let mut subscribed_events = pin!(subscribed((), move |()| {
            let next_attempt = planned_attempts
                .pop()
                .unwrap_or_else(|| stream::iter(Vec::new()));
            async move { Ok(next_attempt) }
        }));
        assert_eq!(subscribed_events.next().await, Some(Ok(0)));
        assert_eq!(subscribed_events.next().await, Some(Err("source ended")));
        assert_eq!(started.elapsed(), Duration::ZERO);
        assert_eq!(subscribed_events.next().await, Some(Ok(1)));
        assert!(within_jitter_of(started.elapsed(), FIRST_RECONNECT_DELAY));
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_subscribe_failures_back_off_exponentially() {
        let mut subscribed_events = pin!(subscribed::<
            (),
            stream::Iter<IntoIter<Result<u8, &'static str>>>,
            _,
            u8,
            &'static str,
        >((), |()| async { Err("refused") }));
        assert_eq!(subscribed_events.next().await, Some(Err("refused")));
        let second_attempt_at = time::Instant::now();
        assert_eq!(subscribed_events.next().await, Some(Err("refused")));
        assert!(within_jitter_of(
            second_attempt_at.elapsed(),
            Duration::from_secs(1)
        ));
        let third_attempt_at = time::Instant::now();
        assert_eq!(subscribed_events.next().await, Some(Err("refused")));
        assert!(within_jitter_of(
            third_attempt_at.elapsed(),
            Duration::from_secs(2)
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn an_ended_subscription_is_dropped_before_resubscribing() {
        struct SubscriptionDropCounter(Arc<AtomicUsize>);
        impl Drop for SubscriptionDropCounter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let dropped_subscriptions = Arc::new(AtomicUsize::new(0));
        let subscription_counter = dropped_subscriptions.clone();
        let mut subscribe_count = 0_usize;
        let observed_drops_at_subscribe = Arc::new(Mutex::new(Vec::new()));
        let recorded_drops = observed_drops_at_subscribe.clone();
        let mut subscribed_events = pin!(subscribed((), move |()| {
            subscribe_count += 1;
            recorded_drops
                .lock()
                .unwrap()
                .push(subscription_counter.load(Ordering::SeqCst));
            let drop_guard = SubscriptionDropCounter(subscription_counter.clone());
            let attempt = subscribe_count;
            async move {
                Ok::<_, &'static str>(stream::iter([Ok(attempt)]).map(move |delivered| {
                    let _ = &drop_guard;
                    delivered
                }))
            }
        }));
        assert_eq!(subscribed_events.next().await, Some(Ok(1)));
        assert_eq!(subscribed_events.next().await, Some(Ok(2)));
        assert_eq!(*observed_drops_at_subscribe.lock().unwrap(), vec![0, 1]);
    }
}
