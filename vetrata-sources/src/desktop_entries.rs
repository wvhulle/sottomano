use std::{sync::Arc, time::Duration};

use crate::{app_icon::DesktopEntries, background_work::blocking};

pub const RELOAD: Duration = Duration::from_secs(300);

pub async fn load() -> Arc<DesktopEntries> {
    Arc::new(blocking("desktop entries", DesktopEntries::load).await)
}
