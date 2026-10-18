use tokio::task::spawn_blocking;

pub async fn blocking<T: Default + Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    let span = tracing::debug_span!("blocking", context);
    spawn_blocking(move || span.in_scope(work))
        .await
        .unwrap_or_else(|e| {
            tracing::error!("{context}: task panicked: {e}");
            T::default()
        })
}
