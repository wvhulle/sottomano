use sottomano_desktop_sources::desktop_notifications::{
    notification_data::NotificationData, vendor_hints::StackTag,
};

use super::{NotificationLayer, ToastLayerHost};

impl<Routed: ToastLayerHost> NotificationLayer<Routed> {
    pub(super) fn stacked_predecessors(&self, data: &NotificationData) -> Vec<u32> {
        match &data.stack_tag {
            StackTag::Tagged(_) => self
                .showing()
                .filter(|shown| shown.data.id != data.id)
                .filter(|shown| shown.data.app_name == data.app_name)
                .filter(|shown| shown.data.stack_tag == data.stack_tag)
                .map(|shown| shown.data.id)
                .collect(),
            StackTag::Untagged => Vec::new(),
        }
    }
}
