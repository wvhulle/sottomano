use std::future::ready;

use futures::{Stream, StreamExt, stream};
use zbus::{
    proxy,
    zvariant::{OwnedValue, Value},
};

use crate::desktop_windows_apps::desktop_entry_app_icons::IconTheme;

const INTERFACE_SETTINGS_NAMESPACE: &str = "org.gnome.desktop.interface";
const ICON_THEME_KEY: &str = "icon-theme";

#[proxy(
    interface = "org.freedesktop.portal.Settings",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait PortalSettings {
    fn read_one(&self, namespace: &str, key: &str) -> zbus::Result<OwnedValue>;

    #[zbus(signal)]
    fn setting_changed(&self, namespace: &str, key: &str, value: Value<'_>) -> zbus::Result<()>;
}

pub fn icon_theme_changes(
    session_bus: &zbus::Connection,
) -> impl Stream<Item = IconTheme> + Send + use<> {
    let session_bus = session_bus.clone();
    stream::once(async move { followed_icon_themes(session_bus).await }).flat_map(
        |followed_themes| match followed_themes {
            Ok(icon_themes) => icon_themes.boxed(),
            Err(portal_failure) => {
                tracing::warn!("icon theme: cannot follow the settings portal: {portal_failure}");
                stream::empty().boxed()
            }
        },
    )
}

async fn followed_icon_themes(
    session_bus: zbus::Connection,
) -> zbus::Result<impl Stream<Item = IconTheme> + Send> {
    let portal_settings = PortalSettingsProxy::new(&session_bus).await?;
    let setting_changes = portal_settings.receive_setting_changed().await?;
    let current_theme = portal_settings
        .read_one(INTERFACE_SETTINGS_NAMESPACE, ICON_THEME_KEY)
        .await?;
    let changed_themes = setting_changes.filter_map(|setting_change| {
        ready(
            setting_change
                .args()
                .inspect_err(|signal_failure| {
                    tracing::warn!("icon theme: unreadable setting change: {signal_failure}");
                })
                .ok()
                .filter(|changed| {
                    changed.namespace == INTERFACE_SETTINGS_NAMESPACE
                        && changed.key == ICON_THEME_KEY
                })
                .and_then(|changed| named_icon_theme(&changed.value)),
        )
    });
    Ok(stream::iter(named_icon_theme(&current_theme)).chain(changed_themes))
}

fn named_icon_theme(setting_value: &Value<'_>) -> Option<IconTheme> {
    match setting_value {
        Value::Str(theme_name) if !theme_name.is_empty() => {
            Some(IconTheme::Named(theme_name.to_string()))
        }
        Value::Value(boxed_value) => named_icon_theme(boxed_value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boxed_theme_name_is_unwrapped() {
        let boxed_name = Value::Value(Box::new(Value::from("Colloid-Light")));
        assert_eq!(
            named_icon_theme(&boxed_name),
            Some(IconTheme::Named("Colloid-Light".to_owned()))
        );
    }

    #[test]
    fn an_empty_or_non_text_setting_names_no_theme() {
        assert_eq!(named_icon_theme(&Value::from("")), None);
        assert_eq!(named_icon_theme(&Value::from(3_u32)), None);
    }
}
