use sottomano_desktop_sources::{
    network_connectivity::{
        tailscale_exit_nodes::ExitRoutingSwitch,
        tailscale_status::{ADMIN_CONSOLE_URL, ExitOfferSwitch, TailscaleSwitch},
    },
    system_protocol_clients::shell_command_output,
};

pub async fn switch_tailscale(tailscale_switch: TailscaleSwitch) {
    let subcommand = match tailscale_switch {
        TailscaleSwitch::Up => "up",
        TailscaleSwitch::Down => "down",
    };
    if let Err(command_error) = shell_command_output::output("tailscale", &[subcommand]).await {
        tracing::warn!("tailscale {subcommand}: {command_error}");
    }
}

pub async fn switch_exit_offer(exit_offer_switch: ExitOfferSwitch) {
    let set_argument = exit_offer_argument(exit_offer_switch);
    if let Err(command_error) =
        shell_command_output::output("tailscale", &["set", set_argument]).await
    {
        tracing::warn!("tailscale set {set_argument}: {command_error}");
    }
}

pub fn open_admin() -> Result<(), shell_command_output::Error> {
    shell_command_output::blocking::run("xdg-open", &[ADMIN_CONSOLE_URL])
}

fn exit_offer_argument(exit_offer_switch: ExitOfferSwitch) -> &'static str {
    match exit_offer_switch {
        ExitOfferSwitch::StartOffering => "--advertise-exit-node=true",
        ExitOfferSwitch::StopOffering => "--advertise-exit-node=false",
    }
}

pub async fn switch_exit_routing(exit_routing_switch: ExitRoutingSwitch) {
    let set_argument = exit_routing_argument(&exit_routing_switch);
    if let Err(command_error) =
        shell_command_output::output("tailscale", &["set", &set_argument]).await
    {
        tracing::warn!("tailscale set {set_argument}: {command_error}");
    }
}

fn exit_routing_argument(exit_routing_switch: &ExitRoutingSwitch) -> String {
    match exit_routing_switch {
        ExitRoutingSwitch::UseExitNode { ip } => format!("--exit-node={ip}"),
        ExitRoutingSwitch::StopUsingExitNode => "--exit-node=".to_owned(),
        ExitRoutingSwitch::AllowLanAccess => "--exit-node-allow-lan-access=true".to_owned(),
        ExitRoutingSwitch::BlockLanAccess => "--exit-node-allow-lan-access=false".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_map_to_tailscale_set_arguments() {
        assert_eq!(
            exit_routing_argument(&ExitRoutingSwitch::UseExitNode {
                ip: "100.1.1.1".to_owned()
            }),
            "--exit-node=100.1.1.1"
        );
        assert_eq!(
            exit_routing_argument(&ExitRoutingSwitch::StopUsingExitNode),
            "--exit-node="
        );
        assert_eq!(
            exit_routing_argument(&ExitRoutingSwitch::AllowLanAccess),
            "--exit-node-allow-lan-access=true"
        );
    }

    #[test]
    fn offer_switches_map_to_tailscale_set_arguments() {
        assert_eq!(
            exit_offer_argument(ExitOfferSwitch::StopOffering),
            "--advertise-exit-node=false"
        );
        assert_eq!(
            exit_offer_argument(ExitOfferSwitch::StartOffering),
            "--advertise-exit-node=true"
        );
    }
}
