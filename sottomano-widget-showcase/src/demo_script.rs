use std::{borrow::Cow, time::Duration};

use sottomano_desktop_sources::{
    incus_instances::InstanceDaemon, login_session_presence::idle_session_actions::IdleAction,
};
use sottomano_widgets::{
    clock::Clock,
    group_menus::{
        group_menu_presentation::{
            GroupMenuLayout, GroupMenuOpenTrigger, GroupMenuPresentation, OpenGroupLimit,
        },
        group_menu_state::{GroupMenuPin, GroupMenuTopic},
    },
    hover_cards::HoverTarget,
    widget_groups::{
        appearance_presets_group::AppearancePresetsTarget,
        apptainer_containers_group::ApptainerContainerTarget,
        audio_group::{AudioTarget, ControlKind},
        caldav_group::CaldavTarget,
        cgroup_memory_limits_group::CgroupMemoryLimitTarget,
        ci_runs_group::{CiRunnerKind, CiRunsTarget},
        claude_usage_group::ClaudeUsageTarget,
        compute_group::ComputeTarget,
        docker_engines_group::DockerEngineTarget,
        flatpak_sandboxes_group::FlatpakSandboxTarget,
        gamepad_group::GamepadTarget,
        incus_instances_group::IncusInstanceTarget,
        login_session_group::LoginSessionTarget,
        lxc_containers_group::LxcContainerTarget,
        media_group::{MediaTarget, PlayerTarget},
        memory_board_group::MemoryBoardTarget,
        microvm_guests_group::MicrovmGuestTarget,
        nspawn_containers_group::NspawnContainerTarget,
        tailnet_group::TailnetTarget,
        virtual_machines_group::VirtualMachineTarget,
        virtualbox_machines_group::VirtualBoxMachineTarget,
        vmware_machines_group::VmwareMachineTarget,
        voice_dictation_group::VoiceDictationTarget,
        wallpaper_group::WallpaperTarget,
        waydroid_container_group::WaydroidContainerTarget,
        weather_group::WeatherTarget,
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        window_group::WindowTarget,
        wireguard_tunnels_group::WireguardTunnelTarget,
    },
    widgets::appearance_presets::AppearancePreset,
};

use crate::{
    fixtures::{
        app_launcher, appearance_presets, apptainer_containers, audio, caldav,
        cgroup_memory_limits, ci_runner_tasks, claude_usage, compute, desktop_session,
        docker_engines, flatpak_sandboxes, gamepad, group_menu_members, idle_lock, incus_instances,
        login_session, lxc_containers, media, memory_board, microvm_guests, notification_toasts,
        nspawn_containers, podman_engines, shell_settings, tailnet, unchanging, virtual_machines,
        virtualbox_machines, vmware_machines, voice_dictation, wallpaper, waydroid_container,
        weather, wireguard_tunnels,
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
        presentation: GroupMenuPresentation,
    },
    PinnedFrom {
        menu: GroupMenuTopic,
        group: WidgetGroupIdentity,
    },
}

pub struct Demo {
    pub name: &'static str,
    pub group: fn(Clock) -> ShowcasedGroup,
    pub live: fn(Clock) -> Vec<ShowcaseMessage>,
    pub shown: GroupShown,
    pub script: &'static [Step],
}

pub enum Step {
    Hover(HoverTarget),
    Press(HoverTarget),
    PressPin(GroupMenuPin),
    Drag { target: HoverTarget, by: f32 },
    HoverText(&'static str),
    HoverApp(&'static str),
    PressText(&'static str),
    RestFor(Duration),
}

pub const DEMOS: &[Demo] = &[
    Demo {
        name: "app_launcher_frequent",
        group: app_launcher::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Applications),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
            Step::PressText("Frequently launched on this workspace"),
            Step::HoverApp("firefox"),
        ],
    },
    Demo {
        name: "app_launcher_overflow",
        group: app_launcher::group,
        live: app_launcher::scrolled_item_list,
        shown: GroupShown::InMenu(GroupMenuTopic::Applications),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Applications)),
            Step::PressText("Utilities"),
            Step::HoverText("Utilities"),
        ],
    },
    Demo {
        name: "compute_cpu",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressText("Compute"),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuLoad)),
        ],
    },
    Demo {
        name: "compute_model",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressText("Compute"),
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuModel)),
        ],
    },
    Demo {
        name: "memory_board_memory",
        group: memory_board::group,
        live: memory_board::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressText("Memory"),
            Step::Hover(HoverTarget::MemoryBoard(MemoryBoardTarget::Memory)),
        ],
    },
    Demo {
        name: "tailnet_exit_node",
        group: tailnet::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::PressText("Tailscale"),
            Step::Hover(HoverTarget::Tailnet(TailnetTarget::ExitNodeChoice {
                exit_node_ip: Cow::Borrowed(tailnet::HOME_EXIT_NODE_IP),
            })),
        ],
    },
    Demo {
        name: "wireguard_tunnels",
        group: wireguard_tunnels::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::PressText("WireGuard"),
            Step::Hover(HoverTarget::WireguardTunnel(WireguardTunnelTarget {
                interface: Cow::Borrowed(wireguard_tunnels::HOME_SERVER_TUNNEL_NAME),
            })),
        ],
    },
    Demo {
        name: "tailnet_access",
        group: tailnet::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Network),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Network)),
            Step::PressText("Tailscale"),
            Step::Hover(HoverTarget::Tailnet(TailnetTarget::InboundAccess)),
        ],
    },
    Demo {
        name: "weather_conditions",
        group: weather::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::PressText("Weather"),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Temperature)),
        ],
    },
    Demo {
        name: "weather_precipitation",
        group: weather::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::PressText("Weather"),
            Step::Hover(HoverTarget::Weather(WeatherTarget::Rain)),
        ],
    },
    Demo {
        name: "podman_engines",
        group: podman_engines::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Podman containers"),
        ],
    },
    Demo {
        name: "virtual_machines",
        group: virtual_machines::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("VMs"),
            Step::Hover(HoverTarget::VirtualMachine(VirtualMachineTarget::of_pid(
                virtual_machines::INTERACTIVE_VM_PID,
            ))),
        ],
    },
    Demo {
        name: "microvm_guests_cloud_hypervisor",
        group: microvm_guests::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("MicroVMs"),
            Step::Hover(HoverTarget::MicrovmGuest(MicrovmGuestTarget::of_pid(
                microvm_guests::HOVERED_MICROVM_PID,
            ))),
        ],
    },
    Demo {
        name: "nspawn_containers",
        group: nspawn_containers::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("nspawn containers"),
            Step::Hover(HoverTarget::NspawnContainer(
                NspawnContainerTarget::of_machine(Cow::Borrowed(
                    nspawn_containers::HOVERED_CONTAINER_NAME,
                )),
            )),
        ],
    },
    Demo {
        name: "incus_instances_container",
        group: incus_instances::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Incus instances"),
            Step::Hover(HoverTarget::IncusInstance(
                IncusInstanceTarget::of_instance(
                    InstanceDaemon::Incus,
                    Cow::Borrowed(incus_instances::HOVERED_INSTANCE_NAME),
                ),
            )),
        ],
    },
    Demo {
        name: "lxc_containers_running",
        group: lxc_containers::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("LXC containers"),
            Step::Hover(HoverTarget::LxcContainer(LxcContainerTarget::of_container(
                Cow::Borrowed(lxc_containers::HOVERED_CONTAINER_NAME),
            ))),
        ],
    },
    Demo {
        name: "flatpak_sandboxes_app",
        group: flatpak_sandboxes::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Flatpak apps"),
            Step::Hover(HoverTarget::FlatpakSandbox(
                FlatpakSandboxTarget::of_instance(Cow::Borrowed(
                    flatpak_sandboxes::HOVERED_INSTANCE_ID,
                )),
            )),
        ],
    },
    Demo {
        name: "waydroid_container_session",
        group: waydroid_container::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Waydroid"),
            Step::Hover(HoverTarget::WaydroidContainer(WaydroidContainerTarget)),
        ],
    },
    Demo {
        name: "virtualbox_machines_headless",
        group: virtualbox_machines::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("VirtualBox machines"),
            Step::Hover(HoverTarget::VirtualBoxMachine(
                VirtualBoxMachineTarget::of_pid(virtualbox_machines::HOVERED_MACHINE_PID),
            )),
        ],
    },
    Demo {
        name: "vmware_machines_workstation",
        group: vmware_machines::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("VMware machines"),
            Step::Hover(HoverTarget::VmwareMachine(VmwareMachineTarget::of_pid(
                vmware_machines::HOVERED_MACHINE_PID,
            ))),
        ],
    },
    Demo {
        name: "apptainer_containers_image",
        group: apptainer_containers::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Apptainer containers"),
            Step::Hover(HoverTarget::ApptainerContainer(
                ApptainerContainerTarget::of_runtime(apptainer_containers::HOVERED_RUNTIME_PID),
            )),
        ],
    },
    Demo {
        name: "docker_engines_rootful",
        group: docker_engines::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Docker containers"),
            Step::Hover(HoverTarget::DockerEngine(DockerEngineTarget::of_socket(
                Cow::Borrowed(docker_engines::ROOTFUL_DOCKER_SOCKET),
            ))),
        ],
    },
    Demo {
        name: "cgroup_memory_limits_ci_runners",
        group: cgroup_memory_limits::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Virtualization),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Virtualization)),
            Step::PressText("Memory limits"),
            Step::Hover(HoverTarget::CgroupMemoryLimit(
                CgroupMemoryLimitTarget::of_cgroup(Cow::Borrowed(
                    cgroup_memory_limits::HOVERED_CGROUP_PATH,
                )),
            )),
        ],
    },
    Demo {
        name: "ci_runner_tasks",
        group: ci_runner_tasks::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Services),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
            Step::PressText("3 CI runs"),
            Step::Hover(HoverTarget::CiRuns(CiRunsTarget::run(
                CiRunnerKind::Woodpecker,
                ci_runner_tasks::SHOWN_RUNNER,
                ci_runner_tasks::HOVERED_RUN,
            ))),
        ],
    },
    Demo {
        name: "caldav_agenda",
        group: caldav::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Agenda),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)),
            Step::PressText("Calendar"),
            Step::Hover(HoverTarget::Caldav(CaldavTarget::UpcomingEvents)),
            Step::Hover(HoverTarget::Caldav(CaldavTarget::OpenTasks)),
        ],
    },
    Demo {
        name: "ai_usage_windows",
        group: claude_usage::group,
        live: claude_usage::live,
        shown: GroupShown::InMenu(GroupMenuTopic::Services),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Services)),
            Step::PressText("Claude usage"),
            Step::Hover(HoverTarget::ClaudeUsage(ClaudeUsageTarget::UsageWindows)),
        ],
    },
    Demo {
        name: "media_track",
        group: media::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("Spotify"),
            Step::Hover(HoverTarget::Media(PlayerTarget {
                bus: Cow::Borrowed(media::SPOTIFY_BUS),
                control: MediaTarget::Track,
            })),
        ],
    },
    Demo {
        name: "idle_screen_off",
        group: idle_lock::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Session),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
            Step::PressText("Screen off"),
            Step::Hover(HoverTarget::IdleTimeout(IdleAction::ScreenOff)),
        ],
    },
    Demo {
        name: "appearance_looks",
        group: appearance_presets::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Behaviour),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Behaviour)),
            Step::PressText("Looks"),
            Step::Hover(HoverTarget::AppearancePresets(
                AppearancePresetsTarget::Preset(AppearancePreset::FrostedDark),
            )),
            Step::PressText("Frosted dark"),
            Step::RestFor(Duration::from_millis(1200)),
        ],
    },
    Demo {
        name: "gamepad_controller",
        group: gamepad::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("DualSense Wireless Controller"),
            Step::Hover(HoverTarget::Gamepad(GamepadTarget::Controller {
                slot: gamepad::DUALSENSE_SLOT,
            })),
        ],
    },
    Demo {
        name: "audio_volume",
        group: audio::group,
        live: audio::live,
        shown: GroupShown::OnPanel,
        script: &[
            Step::Press(HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink))),
            Step::Press(HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink))),
            Step::Drag {
                target: HoverTarget::Audio(AudioTarget::Control(ControlKind::Sink)),
                by: -24.0,
            },
        ],
    },
    Demo {
        name: "voice_dictation_model",
        group: voice_dictation::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("Voice dictation"),
            Step::Hover(HoverTarget::VoiceDictation(
                VoiceDictationTarget::SpeechModel,
            )),
        ],
    },
    Demo {
        name: "login_session_kernel",
        group: login_session::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Session),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Session)),
            Step::PressText("Login session"),
            Step::Hover(HoverTarget::LoginSession(LoginSessionTarget::Kernel)),
            Step::Hover(HoverTarget::LoginSession(LoginSessionTarget::Compositor)),
        ],
    },
    Demo {
        name: "wallpaper_article",
        group: wallpaper::group,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Display),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Display)),
            Step::PressText("Wallpaper"),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Lookup)),
            Step::Hover(HoverTarget::Wallpaper(WallpaperTarget::Article)),
        ],
    },
    Demo {
        name: "bottom_workspaces",
        group: desktop_session::bottom_bar,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Workspace(2))],
    },
    Demo {
        name: "bottom_window_directory_groups",
        group: desktop_session::terminals_grouped_by_directory,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Pick(2)))],
    },
    Demo {
        name: "bottom_process_inspector",
        group: desktop_session::bottom_bar,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::Hover(HoverTarget::Window(WindowTarget::Inspect(2)))],
    },
    Demo {
        name: "toast_arrival",
        group: notification_toasts::arrival,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[
            Step::HoverText(notification_toasts::ARRIVAL_SUMMARY),
            Step::RestFor(Duration::from_millis(3500)),
        ],
    },
    Demo {
        name: "toast_progress",
        group: notification_toasts::progress,
        live: notification_toasts::downloading,
        shown: GroupShown::OnPanel,
        script: &[Step::RestFor(Duration::from_secs(5))],
    },
    Demo {
        name: "toast_actions",
        group: notification_toasts::actions,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[
            Step::HoverText("Later"),
            Step::HoverText("Decline"),
            Step::PressText("Accept"),
        ],
    },
    Demo {
        name: "toast_window_anchored",
        group: notification_toasts::next_to_window,
        live: unchanging,
        shown: GroupShown::OnPanel,
        script: &[Step::HoverText(notification_toasts::WINDOW_SUMMARY)],
    },
    Demo {
        name: "group_menu_accordion_several",
        group: group_menu_members::hardware_group_menu,
        live: group_menu_members::hardware_menu_live,
        shown: GroupShown::InMenuPresented {
            menu: GroupMenuTopic::Hardware,
            presentation: GroupMenuPresentation {
                layout: GroupMenuLayout::Accordion,
                open_trigger: GroupMenuOpenTrigger::Press,
                accordion_open_limit: OpenGroupLimit::Several,
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
        shown: GroupShown::InMenu(GroupMenuTopic::Media),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Media)),
            Step::PressText("Spotify"),
            Step::PressText("Audio"),
            Step::Hover(HoverTarget::Audio(AudioTarget::Control(
                ControlKind::Source,
            ))),
        ],
    },
    Demo {
        name: "group_menu_pin",
        group: group_menu_members::hardware_group_menu,
        live: group_menu_members::hardware_menu_live,
        shown: GroupShown::InMenu(GroupMenuTopic::Hardware),
        script: &[
            Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Hardware)),
            Step::PressPin(GroupMenuPin::Group(WidgetGroupIdentity::sole(
                WidgetGroupFamily::MemoryBoard,
            ))),
            Step::RestFor(Duration::from_millis(1500)),
        ],
    },
    Demo {
        name: "group_menu_appearance",
        group: shell_settings::sharing_a_unit,
        live: unchanging,
        shown: GroupShown::InMenu(GroupMenuTopic::Appearance),
        script: &[Step::Press(HoverTarget::GroupMenu(
            GroupMenuTopic::Appearance,
        ))],
    },
    Demo {
        name: "compute_cpu_pinned",
        group: compute::group,
        live: compute::live,
        shown: GroupShown::PinnedFrom {
            menu: GroupMenuTopic::Hardware,
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::Compute),
        },
        script: &[
            Step::Hover(HoverTarget::Compute(ComputeTarget::CpuLoad)),
            Step::Press(HoverTarget::Compute(ComputeTarget::CpuLoad)),
        ],
    },
];
