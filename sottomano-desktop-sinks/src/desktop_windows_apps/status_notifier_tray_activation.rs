use sottomano_desktop_sources::desktop_windows_apps::status_notifier_tray::TrayHost;
use system_tray::client::ActivateRequest;

const STATUS_NOTIFIER_ITEM_PATH: &str = "/StatusNotifierItem";
const STATUS_NOTIFIER_ITEM_INTERFACE: &str = "org.kde.StatusNotifierItem";

pub async fn activate_tray_item(tray_host: TrayHost, command: TrayCommand) {
    tracing::debug!(
        "tray {}: activating without xdg activation token",
        command.address()
    );
    send_tray_command(&tray_host, command).await;
}

pub async fn activate_tray_item_with_token(
    tray_host: TrayHost,
    command: TrayCommand,
    activation_token: String,
) {
    provide_activation_token(&tray_host, command.address(), activation_token).await;
    send_tray_command(&tray_host, command).await;
}

async fn send_tray_command(tray_host: &TrayHost, command: TrayCommand) {
    if let Err(activate_failure) = tray_host
        .tray_client()
        .activate(activation_request(command))
        .await
    {
        tracing::warn!("tray: activate: {activate_failure}");
    }
}

async fn provide_activation_token(tray_host: &TrayHost, address: &str, activation_token: String) {
    match tray_host
        .session_bus()
        .call_method(
            Some(address),
            STATUS_NOTIFIER_ITEM_PATH,
            Some(STATUS_NOTIFIER_ITEM_INTERFACE),
            "ProvideXdgActivationToken",
            &(activation_token,),
        )
        .await
    {
        Ok(_) => tracing::debug!("tray {address}: provided xdg activation token"),
        Err(provide_failure) => {
            tracing::debug!("tray {address}: item ignores xdg activation token: {provide_failure}");
        }
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

impl TrayCommand {
    #[must_use]
    pub fn address(&self) -> &str {
        match self {
            TrayCommand::Activate(address)
            | TrayCommand::Secondary(address)
            | TrayCommand::MenuClick { address, .. } => address,
        }
    }
}
