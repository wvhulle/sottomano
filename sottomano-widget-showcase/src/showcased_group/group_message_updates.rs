use std::ops::ControlFlow;

use sottomano_widgets::clock::Clock;

use crate::{showcase_message::ShowcaseMessage, showcased_group::ShowcasedGroup};

impl ShowcasedGroup {
    pub fn update(
        &mut self,
        message: ShowcaseMessage,
        clock: Clock,
    ) -> Result<(), Box<ShowcaseMessage>> {
        match (self, message) {
            (ShowcasedGroup::Compute(group), ShowcaseMessage::Compute(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::MemoryBoard(group), ShowcaseMessage::MemoryBoard(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::Weather(group), ShowcaseMessage::Weather(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::ClaudeUsage(group), ShowcaseMessage::ClaudeUsage(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::Media(media), ShowcaseMessage::Media(delivery)) => {
                let _ = media.group.update(delivery, clock);
            }
            (ShowcasedGroup::IdleTimeouts(group), ShowcaseMessage::IdleTimeouts(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::Wallpaper(group), ShowcaseMessage::Wallpaper(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::Audio(group), ShowcaseMessage::Audio(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::Gamepad(gamepad), ShowcaseMessage::Gamepad(delivery)) => {
                let _ = gamepad.group.update(delivery, &gamepad.bindings);
            }
            (ShowcasedGroup::VoiceDictation(group), ShowcaseMessage::VoiceDictation(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::Caldav(group), ShowcaseMessage::Caldav(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (
                ShowcasedGroup::WireguardTunnels(group),
                ShowcaseMessage::WireguardTunnels(delivery),
            ) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::Tailnet(group), ShowcaseMessage::Tailnet(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::LoginSession(group), ShowcaseMessage::LoginSession(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (
                ShowcasedGroup::BottomPanel(session) | ShowcasedGroup::WorkspaceSidebar(session),
                ShowcaseMessage::WindowGroup(delivery),
            ) => {
                let _ = session.window_group.update(delivery, clock);
            }
            (ShowcasedGroup::MenuMembers(groups), unclaimed) => {
                return groups
                    .iter_mut()
                    .try_fold(unclaimed, |unclaimed, member| {
                        match member.update(unclaimed, clock) {
                            Ok(()) => ControlFlow::Break(()),
                            Err(still_unclaimed) => ControlFlow::Continue(*still_unclaimed),
                        }
                    })
                    .continue_value()
                    .map_or(Ok(()), |unclaimed| Err(Box::new(unclaimed)));
            }
            (guest_group, unclaimed) => return guest_group.update_guest_group(unclaimed, clock),
        }
        Ok(())
    }

    fn update_guest_group(
        &mut self,
        message: ShowcaseMessage,
        clock: Clock,
    ) -> Result<(), Box<ShowcaseMessage>> {
        match (self, message) {
            (ShowcasedGroup::PodmanEngines(group), ShowcaseMessage::PodmanEngines(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (
                ShowcasedGroup::VirtualMachines(group),
                ShowcaseMessage::VirtualMachines(delivery),
            ) => {
                let _ = group.update(delivery, clock);
            }
            (
                ShowcasedGroup::NspawnContainers(group),
                ShowcaseMessage::NspawnContainers(delivery),
            ) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::CiRuns(group), ShowcaseMessage::CiRuns(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::DockerEngines(group), ShowcaseMessage::DockerEngines(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (
                ShowcasedGroup::ApptainerContainers(group),
                ShowcaseMessage::ApptainerContainers(delivery),
            ) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::VmwareMachines(group), ShowcaseMessage::VmwareMachines(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (
                ShowcasedGroup::VirtualBoxMachines(group),
                ShowcaseMessage::VirtualBoxMachines(delivery),
            ) => {
                let _ = group.update(delivery, clock);
            }
            (
                ShowcasedGroup::WaydroidContainer(group),
                ShowcaseMessage::WaydroidContainer(delivery),
            ) => {
                let _ = group.update(delivery, clock);
            }
            (
                ShowcasedGroup::FlatpakSandboxes(group),
                ShowcaseMessage::FlatpakSandboxes(delivery),
            ) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::LxcContainers(group), ShowcaseMessage::LxcContainers(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::IncusInstances(group), ShowcaseMessage::IncusInstances(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (ShowcasedGroup::MicrovmGuests(group), ShowcaseMessage::MicrovmGuests(delivery)) => {
                let _ = group.update(delivery, clock);
            }
            (
                ShowcasedGroup::CgroupMemoryLimits(group),
                ShowcaseMessage::CgroupMemoryLimits(delivery),
            ) => {
                let _ = group.update(delivery, clock);
            }
            (_, unclaimed) => return Err(Box::new(unclaimed)),
        }
        Ok(())
    }
}
