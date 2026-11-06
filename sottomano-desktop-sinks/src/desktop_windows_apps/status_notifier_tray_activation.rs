use sottomano_desktop_sources::desktop_windows_apps::status_notifier_tray::TrayHost;
use system_tray::client::ActivateRequest;

pub async fn activate_tray_item(tray_host: TrayHost, command: TrayCommand) {
    if let Err(activate_failure) = tray_host
        .tray_client()
        .activate(activation_request(command))
        .await
    {
        tracing::warn!("tray: activate: {activate_failure}");
    }
}

fn activation_request(command: TrayCommand) -> ActivateRequest {
    match command {
        TrayCommand::Activate(address) => ActivateRequest::Default {
            address,
            x: 0,
            y: 0,
        },
        TrayCommand::Secondary(address) => ActivateRequest::Secondary {
            address,
            x: 0,
            y: 0,
        },
        TrayCommand::MenuClick {
            address,
            menu_path,
            id,
        } => ActivateRequest::MenuItem {
            address,
            menu_path,
            submenu_id: id,
        },
    }
}

#[derive(Debug, Clone)]
pub enum TrayCommand {
    Activate(String),
    Secondary(String),
    MenuClick {
        address: String,
        menu_path: String,
        id: i32,
    },
}
