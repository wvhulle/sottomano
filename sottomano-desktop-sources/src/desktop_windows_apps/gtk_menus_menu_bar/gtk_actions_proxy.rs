use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zbus::{
    names::OwnedBusName,
    proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, OwnedValue, Signature, Type, Value},
};

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct GtkActionDescription {
    pub(crate) enabled: bool,
    pub(crate) parameter_type: Signature,
    pub(crate) state: Vec<OwnedValue>,
}

#[proxy(interface = "org.gtk.Actions", assume_defaults = false)]
pub trait GtkActions {
    fn describe_all(&self) -> zbus::Result<HashMap<String, GtkActionDescription>>;

    fn activate(
        &self,
        action_name: &str,
        parameter: &[Value<'_>],
        platform_data: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn changed(
        &self,
        removals: Vec<String>,
        enable_changes: HashMap<String, bool>,
        state_changes: HashMap<String, OwnedValue>,
        additions: HashMap<String, GtkActionDescription>,
    ) -> zbus::Result<()>;
}

pub async fn gtk_actions_proxy<'proxy>(
    connection: &zbus::Connection,
    bus_name: &OwnedBusName,
    object_path: &OwnedObjectPath,
) -> zbus::Result<GtkActionsProxy<'proxy>> {
    GtkActionsProxy::builder(connection)
        .destination(bus_name.clone())?
        .path(object_path.clone())?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}
