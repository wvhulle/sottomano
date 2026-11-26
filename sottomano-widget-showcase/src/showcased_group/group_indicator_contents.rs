use sottomano_toml_config::Config;
use sottomano_widgets::{
    clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent,
};

use crate::showcased_group::ShowcasedGroup;

impl ShowcasedGroup {
    pub(super) fn interactive_group_contents<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        let locale = config.calendar.locale;
        let sole_group = match self {
            ShowcasedGroup::Compute(group) => group.group_content(clock, locale),
            ShowcasedGroup::MemoryBoard(group) => group.group_content(locale),
            ShowcasedGroup::Weather(group) => group.group_content(config, clock),
            ShowcasedGroup::Location(group) => group.group_content(),
            ShowcasedGroup::ClaudeUsage(group) => group.group_content(clock),
            ShowcasedGroup::Wallpaper(group) => group.group_content(),
            ShowcasedGroup::Audio(group) => group.group_content(config),
            ShowcasedGroup::VoiceDictation(group) => group.group_content(),
            ShowcasedGroup::Caldav(group) => {
                group.group_content(&config.indicator_groups.caldav, clock, locale)
            }
            ShowcasedGroup::Tailnet(group) => group.group_content(clock),
            ShowcasedGroup::WireguardTunnels(group) => group.group_content(),
            ShowcasedGroup::LoginSession(group) => group.group_content(),
            ShowcasedGroup::SessionFocus(group) => group.group_content(config, clock),
            ShowcasedGroup::AppearancePresets(presets) => {
                presets.group.group_content(&presets.appearance)
            }
            listed_group => return listed_group.listed_group_contents(config, clock),
        };
        vec![sole_group]
    }

    fn listed_group_contents<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        match self {
            ShowcasedGroup::Media(media) => media
                .group
                .group_contents(config, &media.desktop_entries, clock)
                .collect(),
            ShowcasedGroup::IdleTimeouts(group) => vec![group.idle_chain_group(config, clock)],
            ShowcasedGroup::Gamepad(gamepad) => gamepad
                .group
                .group_contents(&gamepad.bindings, clock)
                .into_iter()
                .collect(),
            guest_group => guest_group.guest_group_contents(config, clock),
        }
    }

    fn guest_group_contents<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        let locale = config.calendar.locale;
        let guest_group = match self {
            ShowcasedGroup::PodmanEngines(group) => group.group_content(),
            ShowcasedGroup::VirtualMachines(group) => group.group_content(),
            ShowcasedGroup::NspawnContainers(group) => group.group_content(),
            ShowcasedGroup::DockerEngines(group) => group.group_content(),
            ShowcasedGroup::ApptainerContainers(group) => group.group_content(),
            ShowcasedGroup::VmwareMachines(group) => group.group_content(),
            ShowcasedGroup::VirtualBoxMachines(group) => group.group_content(),
            ShowcasedGroup::WaydroidContainer(group) => group.group_content(),
            ShowcasedGroup::FlatpakSandboxes(group) => group.group_content(),
            ShowcasedGroup::LxcContainers(group) => group.group_content(),
            ShowcasedGroup::IncusInstances(group) => group.group_content(),
            ShowcasedGroup::MicrovmGuests(group) => group.group_content(),
            ShowcasedGroup::CgroupMemoryLimits(group) => group.group_content(config),
            ShowcasedGroup::CiRuns(group) => group.group_content(clock, locale),
            _ => return Vec::new(),
        };
        vec![guest_group]
    }
}
