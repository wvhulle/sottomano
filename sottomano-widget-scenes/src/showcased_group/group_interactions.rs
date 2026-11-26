use std::ops::ControlFlow;

use sottomano_toml_config::Config;
use sottomano_widgets::{
    clock::Clock, group_indicators::indicator_group_interaction::IndicatorGroupInteraction,
    panel_menus::settings_menu::appearance_presets_group::AppearancePresetsMessage,
};

use crate::showcased_group::ShowcasedGroup;

impl ShowcasedGroup {
    pub fn interact(
        &mut self,
        group_interaction: IndicatorGroupInteraction,
        config: &Config,
        clock: Clock,
    ) -> Result<(), Box<IndicatorGroupInteraction>> {
        match (self, group_interaction) {
            (
                ShowcasedGroup::AppLauncher(launcher),
                IndicatorGroupInteraction::AppLauncher(launcher_message),
            ) => {
                let _ = launcher.group.update(launcher_message, config);
            }
            (ShowcasedGroup::Compute(group), IndicatorGroupInteraction::Compute(interaction)) => {
                let _ = group.interact(interaction, config, clock);
            }
            (
                ShowcasedGroup::MemoryBoard(group),
                IndicatorGroupInteraction::MemoryBoard(interaction),
            ) => {
                let _ = group.interact(interaction, config, clock);
            }
            (ShowcasedGroup::Weather(group), IndicatorGroupInteraction::Weather(interaction)) => {
                let _ = group.interact(interaction);
            }
            (ShowcasedGroup::Location(group), IndicatorGroupInteraction::Location(interaction)) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::ClaudeUsage(group),
                IndicatorGroupInteraction::ClaudeUsage(interaction),
            ) => {
                let _ = group.interact(interaction, config);
            }
            (ShowcasedGroup::Media(media), IndicatorGroupInteraction::Media(interaction)) => {
                let _ = media.group.interact(interaction, clock);
            }
            (
                ShowcasedGroup::IdleTimeouts(group),
                IndicatorGroupInteraction::IdleTimeouts(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (ShowcasedGroup::Wallpaper(group), IndicatorGroupInteraction::Wallpaper(expansion)) => {
                let _ = group.interact(expansion);
            }
            (ShowcasedGroup::Audio(group), IndicatorGroupInteraction::Audio(message)) => {
                let _ = group.update(message, clock);
            }
            (ShowcasedGroup::Gamepad(gamepad), IndicatorGroupInteraction::Gamepad(interaction)) => {
                let _ = gamepad.group.interact(interaction);
            }
            (
                ShowcasedGroup::VoiceDictation(group),
                IndicatorGroupInteraction::VoiceDictation(interaction),
            ) => {
                let _ = group.interact(interaction, config, clock);
            }
            (ShowcasedGroup::Caldav(group), IndicatorGroupInteraction::Caldav(expansion)) => {
                let _ = group.interact(expansion);
            }
            (
                ShowcasedGroup::WireguardTunnels(group),
                IndicatorGroupInteraction::WireguardTunnels(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (ShowcasedGroup::Tailnet(group), IndicatorGroupInteraction::Tailnet(interaction)) => {
                let _ = group.interact(interaction, config, clock);
            }
            (
                ShowcasedGroup::BottomPanel(session) | ShowcasedGroup::WorkspaceSidebar(session),
                IndicatorGroupInteraction::OutputWindows(interaction),
            ) => {
                let _ = session.output_windows.interact(interaction);
            }
            (
                ShowcasedGroup::AppearancePresets(presets),
                IndicatorGroupInteraction::AppearancePresets(presets_message),
            ) => match presets_message {
                AppearancePresetsMessage::Apply(preset) => {
                    presets.appearance = preset.applied_to(presets.appearance);
                }
                AppearancePresetsMessage::Expansion(expansion) => {
                    presets.group.update_expansion(expansion);
                }
            },
            (ShowcasedGroup::MenuMembers(groups), unclaimed) => {
                return groups
                    .iter_mut()
                    .try_fold(unclaimed, |unclaimed, member| {
                        match member.interact(unclaimed, config, clock) {
                            Ok(()) => ControlFlow::Break(()),
                            Err(still_unclaimed) => ControlFlow::Continue(*still_unclaimed),
                        }
                    })
                    .continue_value()
                    .map_or(Ok(()), |unclaimed| Err(Box::new(unclaimed)));
            }
            (other_group, unclaimed) => {
                return other_group.interact_session_groups(unclaimed, config, clock);
            }
        }
        Ok(())
    }

    fn interact_session_groups(
        &mut self,
        group_interaction: IndicatorGroupInteraction,
        config: &Config,
        clock: Clock,
    ) -> Result<(), Box<IndicatorGroupInteraction>> {
        match (self, group_interaction) {
            (
                ShowcasedGroup::LoginSession(group),
                IndicatorGroupInteraction::LoginSession(interaction),
            ) => {
                let _ = group.interact(interaction, config, clock);
            }
            (
                ShowcasedGroup::SessionFocus(group),
                IndicatorGroupInteraction::SessionFocus(message),
            ) => {
                let _ = group.update(message, &config.panel_menus.agenda.session_focus, clock);
            }
            (guest_group, unclaimed) => return guest_group.interact_guest_group(unclaimed),
        }
        Ok(())
    }

    fn interact_guest_group(
        &mut self,
        group_interaction: IndicatorGroupInteraction,
    ) -> Result<(), Box<IndicatorGroupInteraction>> {
        match (self, group_interaction) {
            (
                ShowcasedGroup::PodmanEngines(group),
                IndicatorGroupInteraction::PodmanEngines(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::VirtualMachines(group),
                IndicatorGroupInteraction::VirtualMachines(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::NspawnContainers(group),
                IndicatorGroupInteraction::NspawnContainers(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (ShowcasedGroup::CiRuns(group), IndicatorGroupInteraction::CiRuns(interaction)) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::DockerEngines(group),
                IndicatorGroupInteraction::DockerEngines(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::ApptainerContainers(group),
                IndicatorGroupInteraction::ApptainerContainers(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::VmwareMachines(group),
                IndicatorGroupInteraction::VmwareMachines(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::VirtualBoxMachines(group),
                IndicatorGroupInteraction::VirtualBoxMachines(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::WaydroidContainer(group),
                IndicatorGroupInteraction::WaydroidContainer(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::FlatpakSandboxes(group),
                IndicatorGroupInteraction::FlatpakSandboxes(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::LxcContainers(group),
                IndicatorGroupInteraction::LxcContainers(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::IncusInstances(group),
                IndicatorGroupInteraction::IncusInstances(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::MicrovmGuests(group),
                IndicatorGroupInteraction::MicrovmGuests(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (
                ShowcasedGroup::CgroupMemoryLimits(group),
                IndicatorGroupInteraction::CgroupMemoryLimits(interaction),
            ) => {
                let _ = group.interact(interaction);
            }
            (_, unclaimed) => return Err(Box::new(unclaimed)),
        }
        Ok(())
    }
}
