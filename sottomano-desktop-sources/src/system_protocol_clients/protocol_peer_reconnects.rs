use std::{future::Future, time::Duration};

use futures::{Stream, StreamExt, stream};
use tokio::time::sleep;

pub const PEER_RECONNECT_GAP: Duration = Duration::from_secs(5);

enum SubscriptionPhase<ConnectedStream> {
    Connecting,
    Streaming(ConnectedStream),
    Reconnecting,
}

pub fn resubscribed<SubscribeInputs, ConnectedStream, SubscribeFuture, ItemValue, Failure>(
    retry_gap: Duration,
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
        (SubscriptionPhase::Connecting, subscribe_again),
        move |(phase, mut subscribe)| async move {
            let mut phase = phase;
            loop {
                match phase {
                    SubscriptionPhase::Connecting | SubscriptionPhase::Reconnecting => {
                        if matches!(phase, SubscriptionPhase::Reconnecting) {
                            sleep(retry_gap).await;
                        }
                        phase = match subscribe().await {
                            Ok(connected) => SubscriptionPhase::Streaming(Box::pin(connected)),
                            Err(failure) => {
                                return Some((
                                    Err(failure),
                                    (SubscriptionPhase::Reconnecting, subscribe),
                                ));
                            }
                        };
                    }
                    SubscriptionPhase::Streaming(mut connected) => {
                        match connected.as_mut().next().await {
                            Some(delivered) => {
                                return Some((
                                    delivered,
                                    (SubscriptionPhase::Streaming(connected), subscribe),
                                ));
                            }
                            None => phase = SubscriptionPhase::Reconnecting,
                        }
                    }
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use std::{pin::pin, vec::IntoIter};

    use tokio::time;

    use super::*;

    const RETRY_GAP: Duration = Duration::from_secs(60);

    #[tokio::test(start_paused = true)]
    async fn a_resubscribed_stream_reconnects_after_its_source_ends() {
        let started = time::Instant::now();
        let mut planned_attempts = std::vec![
            stream::iter(vec![Ok(1)]),
            stream::iter(vec![Ok(0), Err("source ended")]),
        ];
        let mut resubscribed_events = pin!(resubscribed(RETRY_GAP, (), move |()| {
            let next_attempt = planned_attempts
                .pop()
                .unwrap_or_else(|| stream::iter(Vec::new()));
            async move { Ok(next_attempt) }
        }));
        assert_eq!(resubscribed_events.next().await, Some(Ok(0)));
        assert_eq!(resubscribed_events.next().await, Some(Err("source ended")));
        assert_eq!(started.elapsed(), Duration::ZERO);
        assert_eq!(resubscribed_events.next().await, Some(Ok(1)));
        assert_eq!(started.elapsed(), RETRY_GAP);
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_subscription_retries_after_the_gap() {
        let started = time::Instant::now();
        let mut resubscribed_events = pin!(resubscribed::<
            (),
            stream::Iter<IntoIter<Result<u8, &'static str>>>,
            _,
            u8,
            &'static str,
        >(RETRY_GAP, (), |()| async { Err("refused") }));
        assert_eq!(resubscribed_events.next().await, Some(Err("refused")));
        assert_eq!(started.elapsed(), Duration::ZERO);
        assert_eq!(resubscribed_events.next().await, Some(Err("refused")));
        assert_eq!(started.elapsed(), RETRY_GAP);
    }

    #[tokio::test(start_paused = true)]
    async fn an_ended_subscription_is_dropped_before_resubscribing() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        struct SubscriptionDropCounter(Arc<AtomicUsize>);
        impl Drop for SubscriptionDropCounter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let dropped_subscriptions = Arc::new(AtomicUsize::new(0));
        let subscription_counter = dropped_subscriptions.clone();
        let mut subscribe_count = 0_usize;
        let observed_drops_at_subscribe = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded_drops = observed_drops_at_subscribe.clone();
        let mut resubscribed_events = pin!(resubscribed(RETRY_GAP, (), move |()| {
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
        assert_eq!(resubscribed_events.next().await, Some(Ok(1)));
        assert_eq!(resubscribed_events.next().await, Some(Ok(2)));
        assert_eq!(*observed_drops_at_subscribe.lock().unwrap(), vec![0, 1]);
    }
}
