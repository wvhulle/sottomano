use std::fmt::{Display, Formatter, Result as FmtResult};

use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panicked;

impl Display for Panicked {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("reading task panicked")
    }
}

pub async fn blocking_poll<T, E>(
    read: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: From<Panicked> + Send + 'static,
{
    spawn_blocking(read)
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
