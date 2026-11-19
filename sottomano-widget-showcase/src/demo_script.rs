use std::{borrow::Cow, time::Duration};

use corredo::popovers::{
    NestedMenuLayout, NestedMenuOpenTrigger, NestedMenuPresentation, OpenGroupLimit,
    SubmenuConnector,
};
use iced::Size;
use sottomano_widgets::{
    clock::Clock,
    group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic},
    hover_cards::{HoverTarget, IndicatorKey},
    indicator_groups::{
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupInstance,
        },
        window_group::WindowTarget,
    },
};

use crate::{
    animation_recording::{CANVAS, PHOTO_CANVAS},
    fixtures::{
        app_launcher, appearance_presets, apptainer_containers, audio, caldav,
        cgroup_memory_limits, ci_runner_tasks, claude_usage, compute, desktop_session,
        flatpak_sandboxes, gamepad, group_menu_members, idle_lock, login_session, lxc_containers,
        microvm_guests, notification_toasts, shell_settings, tailnet, unchanging,
        virtualbox_machines, vmware_machines, voice_dictation, wallpaper, waydroid_container,
        weather,
    },
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupShown {
    OnPanel,
    InMenu(GroupMenuTopic),
    InMenuPresented {
        menu: GroupMenuTopic,
        presentation: NestedMenuPresentation,
    },
    PinnedFrom {
        menu: GroupMenuTopic,
        group: IndicatorGroupIdentity,
    },
}

pub struct Demo {
    pub name: &'static str,
    pub group: fn(Clock) -> ShowcasedGroup,
    pub live: fn(Clock) -> Vec<ShowcaseMessage>,
    pub shown: GroupShown,
    pub script: &'static [Step],
    pub backdrop: DemoBackdrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoBackdrop {
    Transparent,
    FrostablePhoto,
}

impl DemoBackdrop {
    #[must_use]
    pub fn canvas(self) -> Size {
        match self {
            DemoBackdrop::Transparent => CANVAS,
            DemoBackdrop::FrostablePhoto => PHOTO_CANVAS,
        }
    }
}

pub enum Step {
    Hover(HoverTarget),
    Press(HoverTarget),
    HoverFace(IndicatorGroupIdentity),
    PressFace(IndicatorGroupIdentity),
    PressPin(GroupMenuPin),
    DragFace {
        group: IndicatorGroupIdentity,
        by: f32,
    },
    HoverText(&'static str),
    PressText(&'static str),
    RestFor(Duration),
}

const fn instance_row(group: IndicatorGroupIdentity, row_name: &'static str) -> HoverTarget {
    HoverTarget::Indicator(IndicatorKey {
        group,
        name: Cow::Borrowed(row_name),
    })
}

const fn indicator_row(family: IndicatorGroupFamily, row_name: &'static str) -> HoverTarget {
    instance_row(IndicatorGroupIdentity::sole(family), row_name)
}

pub const DEMOS: &[Demo] = &[
    Demo {
        name: "tour_hardware",
        group: group_menu_members::hardware_group_menu,
        live: group_menu_members::hardware_menu_live,
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressText("Compute"),
            Step::Hover(indicator_row(IndicatorGroupFamily::Compute, "CPU load")),
            Step::Hover(indicator_row(IndicatorGroupFamily::Compute, "CPU model")),
            Step::PressText("Memory"),
            Step::Hover(indicator_row(IndicatorGroupFamily::MemoryBoard, "RAM")),
        ],
    },
    Demo {
        name: "tour_network",
        group: group_menu_members::network_group_menu,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::PressText("Tailscale"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Tailnet,
                "Reachable from",
            )),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Tailnet,
                "Exit via home-server",
            )),
            Step::PressText("WireGuard"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::WireguardTunnels,
                "wg-home",
            )),
        ],
    },
    Demo {
        name: "tour_weather",
        group: weather::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::PressText("Weather"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Weather,
                "Temperature forecast",
            )),
            Step::Hover(indicator_row(IndicatorGroupFamily::Weather, "Rain")),
        ],
    },
    Demo {
        name: "tour_media",
        group: group_menu_members::media_group_menu,
        live: audio::live,
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("Spotify"),
            Step::Hover(instance_row(
                IndicatorGroupIdentity {
                    family: IndicatorGroupFamily::Media,
                    instance: IndicatorGroupInstance::Session {
                        durable: Cow::Borrowed("Spotify"),
                        session: Cow::Borrowed("org.mpris.MediaPlayer2.spotify"),
                    },
                },
                "Track",
            )),
            Step::PressText("Audio"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Audio,
                "Microphone volume",
            )),
            Step::PressText("Voice dictation"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::VoiceDictation,
                "Speech model",
            )),
        ],
    },
    Demo {
        name: "tour_virtualization",
        group: group_menu_members::virtualization_group_menu,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("QEMU machines"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::VirtualMachines,
                "sample-vm-slow-services from bootvine pid 4100",
            )),
            Step::PressText("Incus instances"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::IncusInstances,
                "build-box",
            )),
            Step::PressText("Podman containers"),
            Step::PressText("Docker containers"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::DockerEngines,
                "postgres as root",
            )),
        ],
    },
    Demo {
        name: "app_launcher_frequent",
        group: |_| app_launcher::group(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Applications),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
            Step::PressText("Frequently launched on this workspace"),
            Step::HoverText("Firefox"),
        ],
    },
    Demo {
        name: "app_launcher_overflow",
        group: |_| app_launcher::group(),
        live: |_| app_launcher::scrolled_item_list(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Applications),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
            Step::PressText("Utilities"),
            Step::HoverText("Utilities"),
        ],
    },
    Demo {
        name: "tailnet_exit_node",
        group: tailnet::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::PressText("Tailscale"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Tailnet,
                "Exit via home-server",
            )),
        ],
    },
    Demo {
        name: "weather_conditions",
        group: weather::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::PressText("Weather"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Weather,
                "Temperature forecast",
            )),
        ],
    },
    Demo {
        name: "microvm_guests_cloud_hypervisor",
        group: microvm_guests::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("microvm.nix guests"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::MicrovmGuests,
                "ci-builder",
            )),
        ],
    },
    Demo {
        name: "lxc_containers_running",
        group: lxc_containers::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("LXC containers"),
            Step::Hover(indicator_row(IndicatorGroupFamily::LxcContainers, "media")),
        ],
    },
    Demo {
        name: "flatpak_sandboxes_app",
        group: flatpak_sandboxes::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Flatpak apps"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::FlatpakSandboxes,
                "com.spotify.Client 1849302754",
            )),
        ],
    },
    Demo {
        name: "waydroid_container_session",
        group: waydroid_container::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Waydroid"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::WaydroidContainer,
                "Session",
            )),
        ],
    },
    Demo {
        name: "virtualbox_machines_headless",
        group: virtualbox_machines::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("VirtualBox machines"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::VirtualBoxMachines,
                "windows11 pid 61402",
            )),
        ],
    },
    Demo {
        name: "vmware_machines_workstation",
        group: vmware_machines::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("VMware machines"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::VmwareMachines,
                "Ubuntu 26.04",
            )),
        ],
    },
    Demo {
        name: "apptainer_containers_image",
        group: apptainer_containers::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Apptainer containers"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::ApptainerContainers,
                "pytorch_24.08 pid 70215",
            )),
        ],
    },
    Demo {
        name: "cgroup_memory_limits_ci_runners",
        group: cgroup_memory_limits::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Memory limits"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::CgroupMemoryLimits,
                "ci-runners.slice",
            )),
        ],
    },
    Demo {
        name: "ci_runner_tasks",
        group: ci_runner_tasks::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Services),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
            Step::PressText("3 CI runs"),
            Step::Hover(indicator_row(IndicatorGroupFamily::CiRuns, "website #185")),
        ],
    },
    Demo {
        name: "caldav_agenda",
        group: caldav::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::PressText("Calendar"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Caldav,
                "Upcoming events",
            )),
            Step::Hover(indicator_row(IndicatorGroupFamily::Caldav, "Open tasks")),
        ],
    },
    Demo {
        name: "ai_usage_windows",
        group: claude_usage::group,
        live: claude_usage::live,
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Services),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
            Step::PressText("Claude usage"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::ClaudeUsage,
                "Subscription windows",
            )),
        ],
    },
    Demo {
        name: "idle_screen_off",
        group: idle_lock::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Session),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
            Step::PressText("Screen off"),
            Step::Hover(instance_row(
                IndicatorGroupIdentity {
                    family: IndicatorGroupFamily::IdleTimeouts,
                    instance: IndicatorGroupInstance::Durable(Cow::Borrowed("screen_off")),
                },
                "Screen-off timeout",
            )),
        ],
    },
    Demo {
        name: "appearance_looks",
        group: |_| appearance_presets::group(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::FrostablePhoto,
        shown: GroupShown::InMenu(GroupMenuTopic::Behaviour),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Behaviour)),
            Step::PressText("Looks"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::AppearancePresets,
                "Frosted dark",
            )),
            Step::PressText("Frosted dark"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::AppearancePresets,
                "Clear dark",
            )),
            Step::RestFor(Duration::from_millis(600)),
        ],
    },
    Demo {
        name: "gamepad_controller",
        group: |_| gamepad::group(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("DualSense Wireless Controller"),
            Step::Hover(instance_row(
                IndicatorGroupIdentity {
                    family: IndicatorGroupFamily::Gamepad,
                    instance: IndicatorGroupInstance::Durable(Cow::Borrowed("d4:2f:4b:69:6b:2f")),
                },
                "Game controller",
            )),
        ],
    },
    Demo {
        name: "audio_volume",
        group: audio::group,
        live: audio::live,
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::OnPanel,
        script: &[
            Step::PressFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Audio)),
            Step::PressFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Audio)),
            Step::DragFace {
                group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Audio),
                by: -24.0,
            },
        ],
    },
    Demo {
        name: "voice_dictation_model",
        group: voice_dictation::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("Voice dictation"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::VoiceDictation,
                "Speech model",
            )),
        ],
    },
    Demo {
        name: "login_session_kernel",
        group: login_session::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Session),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
            Step::PressText("Login session"),
            Step::Hover(indicator_row(IndicatorGroupFamily::LoginSession, "Kernel")),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::LoginSession,
                "Compositor",
            )),
        ],
    },
    Demo {
        name: "wallpaper_article",
        group: wallpaper::group,
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Display),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Display)),
            Step::PressText("Wallpaper"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Wallpaper,
                "Wallpaper lookup",
            )),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Wallpaper,
                "Wallpaper article",
            )),
        ],
    },
    Demo {
        name: "bottom_workspaces",
        group: |_| desktop_session::bottom_bar(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Workspace(2))],
    },
    Demo {
        name: "bottom_window_directory_groups",
        group: |_| desktop_session::terminals_grouped_by_directory(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Usage(2)))],
    },
    Demo {
        name: "bottom_process_inspector",
        group: |_| desktop_session::bottom_bar(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Inspect(2)))],
    },
    Demo {
        name: "toast_arrival",
        group: |_| notification_toasts::arrival(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::OnPanel,
        script: &[
            Step::HoverText(notification_toasts::ARRIVAL_SUMMARY),
            Step::RestFor(Duration::from_millis(3500)),
        ],
    },
    Demo {
        name: "toast_progress",
        group: |_| notification_toasts::progress(),
        live: notification_toasts::downloading,
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::OnPanel,
        script: &[Step::RestFor(Duration::from_secs(5))],
    },
    Demo {
        name: "toast_actions",
        group: |_| notification_toasts::actions(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::OnPanel,
        script: &[
            Step::HoverText("Later"),
            Step::HoverText("Decline"),
            Step::PressText("Accept"),
        ],
    },
    Demo {
        name: "toast_window_anchored",
        group: |_| notification_toasts::next_to_window(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::OnPanel,
        script: &[Step::HoverText(notification_toasts::WINDOW_SUMMARY)],
    },
    Demo {
        name: "group_menu_accordion_several",
        group: group_menu_members::hardware_group_menu,
        live: group_menu_members::hardware_menu_live,
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenuPresented {
            menu: GroupMenuTopic::Hardware,
            presentation: NestedMenuPresentation {
                layout: NestedMenuLayout::Accordion,
                open_trigger: NestedMenuOpenTrigger::Press,
                accordion_open_limit: OpenGroupLimit::Several,
                submenu_connector: SubmenuConnector::Hidden,
            },
        },
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressText("Compute"),
            Step::PressText("Memory"),
        ],
    },
    Demo {
        name: "group_menu_media",
        group: group_menu_members::media_group_menu,
        live: audio::live,
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("Spotify"),
            Step::PressText("Audio"),
            Step::Hover(indicator_row(
                IndicatorGroupFamily::Audio,
                "Microphone volume",
            )),
        ],
    },
    Demo {
        name: "group_menu_pin",
        group: group_menu_members::hardware_group_menu,
        live: group_menu_members::hardware_menu_live,
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressPin(GroupMenuPin::Group(IndicatorGroupIdentity::sole(
                IndicatorGroupFamily::MemoryBoard,
            ))),
            Step::RestFor(Duration::from_millis(750)),
        ],
    },
    Demo {
        name: "group_menu_appearance",
        group: |_| shell_settings::sharing_a_unit(),
        live: |_| unchanging(),
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::InMenu(GroupMenuTopic::Appearance),
        script: &[Step::Press(HoverTarget::GroupMenu(
            GroupMenuTopic::Appearance,
        ))],
    },
    Demo {
        name: "compute_cpu_pinned",
        group: compute::group,
        live: compute::live,
        backdrop: DemoBackdrop::Transparent,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Hardware,
            group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute),
        },
        script: &[
            Step::HoverFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute)),
            Step::PressFace(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute)),
        ],
    },
];

#[cfg(test)]
mod tests {
    use sottomano_toml_config::Config;
    use sottomano_widgets::{
        clock::Clock, hover_cards::HoverTarget,
        indicator_groups::indicator_group_content::IndicatorGroupContent,
    };

    use super::{DEMOS, Step};

    #[test]
    fn every_demo_group_names_its_pinnable_items_apart_from_each_other_and_the_group() {
        let config = Config::default();
        let clock = Clock::real();
        let clashes: Vec<(String, Vec<String>)> = DEMOS
            .iter()
            .flat_map(|demo| {
                let showcased_group = (demo.group)(clock);
                showcased_group
                    .group_contents(&config, clock)
                    .iter()
                    .map(|group| (demo.name.to_owned(), group.clashing_indicator_names()))
                    .filter(|(_, clashing_names)| !clashing_names.is_empty())
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(clashes, Vec::new());
    }

    #[test]
    fn every_demo_card_titles_its_sections_apart_from_the_item_group_and_source() {
        let config = Config::default();
        let clock = Clock::real();
        let clashes: Vec<(String, String, String)> = DEMOS
            .iter()
            .flat_map(|demo| {
                let showcased_group = (demo.group)(clock);
                showcased_group
                    .group_contents(&config, clock)
                    .iter()
                    .flat_map(|group| group.card_title_clashes(clock))
                    .map(|clash| {
                        (
                            demo.name.to_owned(),
                            clash.indicator_name,
                            clash.clashing_title,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(clashes, Vec::new());
    }

    #[test]
    fn every_scripted_row_is_a_row_of_its_demo_group() {
        let config = Config::default();
        let clock = Clock::real();
        let missing: Vec<(String, String, Vec<String>)> = DEMOS
            .iter()
            .flat_map(|demo| {
                let showcased_group = (demo.group)(clock);
                let contents = showcased_group.group_contents(&config, clock);
                let known_rows: Vec<String> = contents
                    .iter()
                    .flat_map(IndicatorGroupContent::indicator_keys)
                    .map(|indicator_key| format!("{indicator_key:?}"))
                    .collect();
                demo.script
                    .iter()
                    .filter_map(|step| match step {
                        Step::Hover(HoverTarget::Indicator(indicator_key))
                        | Step::Press(HoverTarget::Indicator(indicator_key)) => Some(indicator_key),
                        _ => None,
                    })
                    .filter(|indicator_key| {
                        !contents
                            .iter()
                            .any(|group| group.indicator(indicator_key).is_some())
                    })
                    .map(|indicator_key| {
                        (
                            demo.name.to_owned(),
                            format!("{indicator_key:?}"),
                            known_rows.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(missing, Vec::new());
    }
}
