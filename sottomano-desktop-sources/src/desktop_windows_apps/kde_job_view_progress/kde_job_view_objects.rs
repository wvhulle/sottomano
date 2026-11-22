use std::{collections::HashMap, sync::Arc};

use zbus::{
    fdo,
    object_server::ObjectServer,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use super::{JobPercent, PATH, Tracker};

pub(super) async fn open_view(
    tracker: &Arc<Tracker>,
    object_server: &ObjectServer,
    app: String,
) -> fdo::Result<OwnedObjectPath> {
    let id = tracker.open(app);
    let path = OwnedObjectPath::try_from(format!("{PATH}/JobView_{id}"))
        .map_err(|error| fdo::Error::Failed(error.to_string()))?;
    let view = JobView {
        id,
        path: path.clone(),
        tracker: tracker.clone(),
    };
    let modern = JobViewV3 {
        id,
        path: path.clone(),
        tracker: tracker.clone(),
    };
    object_server.at(&path, view).await?;
    object_server.at(&path, modern).await?;
    Ok(path)
}

struct JobViewV3 {
    id: u64,
    path: OwnedObjectPath,
    tracker: Arc<Tracker>,
}

#[zbus::interface(name = "org.kde.JobViewV3")]
#[allow(clippy::needless_pass_by_value)]
impl JobViewV3 {
    #[zbus(name = "update")]
    fn update(&self, properties: HashMap<String, OwnedValue>) {
        self.tracker.amend(self.id, |job| job.apply(&properties));
    }

    #[zbus(name = "terminate")]
    async fn terminate(
        &self,
        #[zbus(object_server)] object_server: &ObjectServer,
        error_code: u32,
        error_message: String,
        hints: HashMap<String, OwnedValue>,
    ) {
        let _ = (error_code, error_message, hints);
        self.tracker.close(self.id);
        let _ = object_server.remove::<Self, _>(&self.path).await;
        let _ = object_server.remove::<JobView, _>(&self.path).await;
    }
}

struct JobView {
    id: u64,
    path: OwnedObjectPath,
    tracker: Arc<Tracker>,
}

#[zbus::interface(name = "org.kde.JobViewV2")]
#[allow(clippy::unused_self, clippy::needless_pass_by_value)]
impl JobView {
    #[zbus(name = "terminate")]
    async fn terminate(
        &self,
        #[zbus(object_server)] object_server: &ObjectServer,
        error_message: String,
    ) {
        let _ = error_message;
        self.tracker.close(self.id);
        let _ = object_server.remove::<Self, _>(&self.path).await;
    }

    #[zbus(name = "setPercent")]
    fn set_percent(&self, percent: u32) {
        let percent =
            u8::try_from(percent.min(100)).expect("a percentage capped at 100 fits a byte");
        self.tracker
            .amend(self.id, |job| job.percent = JobPercent::Reported(percent));
    }

    #[zbus(name = "setTotalAmount")]
    fn set_total_amount(&self, amount: u64, unit: String) {
        if unit != "bytes" && unit != "files" && unit != "dirs" && unit != "items" {
            return;
        }
        self.tracker.amend(self.id, |job| job.total = amount);
    }

    #[zbus(name = "setProcessedAmount")]
    fn set_processed_amount(&self, amount: u64, unit: String) {
        if unit != "bytes" && unit != "files" && unit != "dirs" && unit != "items" {
            return;
        }
        self.tracker.amend(self.id, |job| job.processed = amount);
    }

    #[zbus(name = "setInfoMessage")]
    fn set_info_message(&self, message: String) {
        let _ = message;
    }

    #[zbus(name = "setDescriptionField")]
    fn set_description_field(&self, number: u32, name: String, value: String) -> bool {
        let _ = (number, name, value);
        false
    }

    #[zbus(name = "clearDescriptionField")]
    fn clear_description_field(&self, number: u32) {
        let _ = number;
    }

    #[zbus(name = "setSuspended")]
    fn set_suspended(&self, suspended: bool) {
        let _ = suspended;
    }

    #[zbus(name = "setSpeed")]
    fn set_speed(&self, bytes_per_second: u64) {
        let _ = bytes_per_second;
    }

    #[zbus(name = "setDestUrl")]
    fn set_dest_url(&self, dest_url: OwnedValue) {
        let _ = dest_url;
    }

    #[zbus(name = "setError")]
    fn set_error(&self, error_code: u32) {
        let _ = error_code;
    }
}
