use sottomano_desktop_sources::{
    login_session_presence::{
        logind_manager::{self, ManagerProxyBlocking},
        session_power_actions::SessionAction,
    },
    system_protocol_clients::shell_command_output,
};

use crate::compositor_window_requests::compositor_requests::{self, CompositorRequest};

fn with_logind_manager(call: impl FnOnce(&ManagerProxyBlocking<'static>) -> zbus::Result<()>) {
    if let Some(proxy) = logind_manager::logind_manager_proxy()
        && let Err(logind_failure) = call(&proxy)
    {
        tracing::warn!("logind: {logind_failure}");
    }
}

pub fn suspend() {
    with_logind_manager(|proxy| proxy.suspend(true));
}

pub fn hibernate() {
    with_logind_manager(|proxy| proxy.hibernate(true));
}

pub fn hybrid_sleep() {
    with_logind_manager(|proxy| proxy.hybrid_sleep(true));
}

pub fn suspend_then_hibernate() {
    with_logind_manager(|proxy| proxy.suspend_then_hibernate(true));
}

pub fn sleep() {
    with_logind_manager(|proxy| proxy.sleep(0));
}

pub fn reboot() {
    with_logind_manager(|proxy| proxy.reboot(true));
}

pub fn reboot_to_firmware_setup() {
    with_logind_manager(|proxy| {
        proxy.set_reboot_to_firmware_setup(true)?;
        proxy.reboot(true)
    });
}

pub fn reboot_to_boot_loader_menu() {
    with_logind_manager(|proxy| {
        proxy.set_reboot_to_boot_loader_menu(BOOT_LOADER_MENU_DEFAULT_TIMEOUT)?;
        proxy.reboot(true)
    });
}

pub fn halt() {
    with_logind_manager(|proxy| proxy.halt(true));
}

pub fn power_off() {
    with_logind_manager(|proxy| proxy.power_off(true));
}

pub fn cancel_scheduled_shutdown() {
    with_logind_manager(|proxy| proxy.cancel_scheduled_shutdown().map(|_| ()));
}

pub fn lock() {
    with_logind_manager(|proxy| proxy.lock_session(&logind_manager::session_id()));
}

pub fn lock_all_sessions() {
    with_logind_manager(ManagerProxyBlocking::lock_sessions);
}

pub fn terminate_session() {
    with_logind_manager(|proxy| proxy.terminate_session(&logind_manager::session_id()));
}

const BOOT_LOADER_MENU_DEFAULT_TIMEOUT: u64 = 0;

pub fn perform_session_action(session_action: SessionAction) {
    match session_action {
        SessionAction::Lock => lock(),
        SessionAction::LockAllSessions => lock_all_sessions(),
        SessionAction::ScreenOff => {
            compositor_requests::send_compositor_request_or_warn(
                &CompositorRequest::PowerOffMonitors,
            );
        }
        SessionAction::Sleep => sleep(),
        SessionAction::Suspend => suspend(),
        SessionAction::Hibernate => hibernate(),
        SessionAction::HybridSleep => hybrid_sleep(),
        SessionAction::SuspendThenHibernate => suspend_then_hibernate(),
        SessionAction::Reboot => reboot(),
        SessionAction::RebootToFirmwareSetup => reboot_to_firmware_setup(),
        SessionAction::RebootToBootLoaderMenu => reboot_to_boot_loader_menu(),
        SessionAction::Halt => halt(),
        SessionAction::PowerOff => power_off(),
        SessionAction::CancelScheduledShutdown => cancel_scheduled_shutdown(),
        SessionAction::LogOut => {
            compositor_requests::send_compositor_request_or_warn(&CompositorRequest::Quit);
        }
        SessionAction::TerminateSession => terminate_session(),
        SessionAction::RestartShell => {
            if let Err(restart_failure) = shell_command_output::blocking::run(
                "systemctl",
                &["--user", "restart", "sottomano.service"],
            ) {
                tracing::warn!("restarting sottomano.service: {restart_failure}");
            }
        }
    }
}
