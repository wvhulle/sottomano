use std::borrow::Cow;

use sottomano_widgets::{
    group_indicators::indicator_group_identity::{
        IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupInstance,
    },
    group_menus::group_menu_state::GroupMenuTopic,
    hover_cards::HoverTarget,
};

use crate::{
    demo_script::{Demo, DemoBackdrop, GroupShown, Step, UiPart, indicator_row, instance_row},
    fixtures::{audio, group_menu_members, session_focus, unchanging, weather},
};

pub(super) const TOUR_HARDWARE: Demo = Demo {
    name: "tour_hardware",
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
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
};

pub(super) const TOUR_NETWORK: Demo = Demo {
    name: "tour_network",
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
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
};

pub(super) const TOUR_WEATHER: Demo = Demo {
    name: "tour_weather",
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
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
};

pub(super) const FOCUS_TIMELINE: Demo = Demo {
    name: "focus_timeline",
    parts: &[UiPart::HoverCards, UiPart::PanelFaces],
    group: session_focus::group,
    live: |_| unchanging(),
    backdrop: DemoBackdrop::Transparent,
    shown: GroupShown::OnPanel,
    script: &[
        Step::PressFace(IndicatorGroupIdentity::sole(
            IndicatorGroupFamily::SessionFocus,
        )),
        Step::Hover(indicator_row(
            IndicatorGroupFamily::SessionFocus,
            "Timeline",
        )),
    ],
};

pub(super) const TOUR_MEDIA: Demo = Demo {
    name: "tour_media",
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
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
};

pub(super) const TOUR_VIRTUALIZATION: Demo = Demo {
    name: "tour_virtualization",
    parts: &[UiPart::GroupMenus, UiPart::HoverCards],
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
};
