use futures::{Stream, StreamExt, stream};
use futures_rx::RxExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable,
}

pub trait AvailabilityStreams: Stream<Item = Availability> + Sized {
    fn while_available<InnerStream>(
        self,
        mut start: impl FnMut() -> InnerStream + Send,
        released: InnerStream::Item,
    ) -> impl Stream<Item = InnerStream::Item> + Send
    where
        Self: Send,
        InnerStream: Stream + Send,
        InnerStream::Item: Clone + Send,
    {
        self.distinct_until_changed()
            .switch_map(move |availability| {
                let (released_items, started_inner) = match availability {
                    Availability::Available => (Vec::new(), vec![start()]),
                    Availability::Unavailable => (vec![released.clone()], Vec::new()),
                };
                stream::iter(released_items).chain(stream::iter(started_inner).flatten())
            })
    }
}

impl<AvailabilityStream: Stream<Item = Availability>> AvailabilityStreams for AvailabilityStream {}

#[cfg(test)]
mod tests {
    use futures::channel::mpsc;

    use super::*;

    #[tokio::test]
    async fn losing_the_service_releases_and_regaining_it_restarts() {
        let (availability_sender, availability_receiver) = mpsc::unbounded();
        let mut started_count = 0_u32;
        let mut deliveries = Box::pin(availability_receiver.while_available(
            move || {
                started_count += 1;
                stream::iter([started_count]).chain(stream::pending())
            },
            0,
        ));
        availability_sender
            .unbounded_send(Availability::Available)
            .unwrap();
        assert_eq!(deliveries.next().await, Some(1));
        availability_sender
            .unbounded_send(Availability::Unavailable)
            .unwrap();
        assert_eq!(deliveries.next().await, Some(0));
        availability_sender
            .unbounded_send(Availability::Available)
            .unwrap();
        assert_eq!(deliveries.next().await, Some(2));
    }
}
