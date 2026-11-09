use std::collections::HashMap;

use sottomano_desktop_sources::desktop_windows_apps::gtk_menus_menu_bar::{
    ApplicationActionPath, GtkActionGroups, GtkActionScope, GtkActionTarget, GtkMenuActivation,
    WindowActionPath, gtk_actions_proxy::gtk_actions_proxy,
};
use zbus::zvariant::{OwnedObjectPath, Value};

fn action_group_path(
    action_groups: &GtkActionGroups,
    scope: GtkActionScope,
) -> Option<&OwnedObjectPath> {
    match (
        scope,
        &action_groups.application_actions,
        &action_groups.window_actions,
    ) {
        (GtkActionScope::Application, ApplicationActionPath::Exported(application_path), _) => {
            Some(application_path)
        }
        (GtkActionScope::Window, _, WindowActionPath::Resolved(window_path)) => Some(window_path),
        (GtkActionScope::Application, ApplicationActionPath::Unexported, _)
        | (GtkActionScope::Window, _, WindowActionPath::Unresolved) => None,
    }
}

pub async fn activate_gtk_action(
    connection: zbus::Connection,
    action_groups: GtkActionGroups,
    activation: GtkMenuActivation,
) {
    let Some(object_path) = action_group_path(&action_groups, activation.scope) else {
        tracing::debug!(
            "global menu: {} needs an action group the application does not export",
            activation.action_name
        );
        return;
    };
    let parameter: Vec<Value<'_>> = match &activation.action_target {
        GtkActionTarget::Untargeted => Vec::new(),
        GtkActionTarget::Targeted(target_value) => {
            let target_value: &Value<'_> = target_value;
            vec![target_value.clone()]
        }
    };
    let activated = async {
        gtk_actions_proxy(&connection, &action_groups.bus_name, object_path)
            .await?
            .activate(&activation.action_name, &parameter, HashMap::new())
            .await
    };
    if let Err(activate_failure) = activated.await {
        tracing::warn!(
            "global menu: activate {}: {activate_failure}",
            activation.action_name
        );
    }
}
