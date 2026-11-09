use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("reading task panicked")]
pub struct Panicked;

pub async fn blocking_poll<T, E>(
    read: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: From<Panicked> + Send + 'static,
{
    let caller_span = tracing::Span::current();
    spawn_blocking(move || caller_span.in_scope(read))
        .await
        .unwrap_or_else(|_| Err(E::from(Panicked)))
}

pub async fn blocking<T: Default + Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    let span = tracing::debug_span!("blocking", context);
    blocking_poll(move || Ok::<T, Panicked>(span.in_scope(work)))
        .await
        .unwrap_or_else(|Panicked| {
            tracing::error!("{context}: task panicked");
            T::default()
        })
}
