use sottomano_compositor_model::compositor_requests::CompositorRequest;
use sottomano_compositors::compositor_connection::send_compositor_request;
use sottomano_desktop_sources::{
    login_session_presence::{
        logind_manager::{self, ManagerProxy},
        session_power_actions::SessionAction,
    },
    system_protocol_clients::shell_command_output,
};

use crate::sink_write_errors::{BusWriteError, SessionActionError};

const BOOT_LOADER_MENU_DEFAULT_TIMEOUT: u64 = 0;

pub async fn perform_session_action(
    session_action: SessionAction,
) -> Result<(), SessionActionError> {
    match session_action {
        SessionAction::ScreenOff => {
            send_compositor_request(CompositorRequest::PowerOffMonitors).await?;
        }
        SessionAction::LogOut => {
            send_compositor_request(CompositorRequest::Quit).await?;
        }
        SessionAction::RestartShell => {
            shell_command_output::output("systemctl", &["--user", "restart", "sottomano.service"])
                .await?;
        }
        logind_action => {
            perform_logind_action(logind_action)
                .await
                .map_err(BusWriteError::from)?;
        }
    }
    Ok(())
}

async fn perform_logind_action(session_action: SessionAction) -> zbus::Result<()> {
    let manager = ManagerProxy::new(&zbus::Connection::system().await?).await?;
    match session_action {
        SessionAction::ScreenOff | SessionAction::LogOut | SessionAction::RestartShell => Ok(()),
        SessionAction::Lock => manager.lock_session(&logind_manager::session_id()).await,
        SessionAction::LockAllSessions => manager.lock_sessions().await,
        SessionAction::Sleep => manager.sleep(0).await,
        SessionAction::Suspend => manager.suspend(true).await,
        SessionAction::Hibernate => manager.hibernate(true).await,
        SessionAction::HybridSleep => manager.hybrid_sleep(true).await,
        SessionAction::SuspendThenHibernate => manager.suspend_then_hibernate(true).await,
        SessionAction::Reboot => manager.reboot(true).await,
        SessionAction::RebootToFirmwareSetup => {
            manager.set_reboot_to_firmware_setup(true).await?;
            manager.reboot(true).await
        }
        SessionAction::RebootToBootLoaderMenu => {
            manager
                .set_reboot_to_boot_loader_menu(BOOT_LOADER_MENU_DEFAULT_TIMEOUT)
                .await?;
            manager.reboot(true).await
        }
        SessionAction::Halt => manager.halt(true).await,
        SessionAction::PowerOff => manager.power_off(true).await,
        SessionAction::CancelScheduledShutdown => {
            manager.cancel_scheduled_shutdown().await.map(drop)
        }
        SessionAction::TerminateSession => {
            manager
                .terminate_session(&logind_manager::session_id())
                .await
        }
    }
}
