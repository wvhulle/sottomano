mod notulist_dictation_reading;

use iced::{Subscription, Task};
use notulist_client::DictationCommand;
use sottomano_sources::{
    SourceDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope},
    voice_dictation_input::{
        notulist_dbus_dictation::{NotulistDictation, send_logged_dictation_command},
        notulist_dbus_engine::NotulistEngine,
        notulist_dbus_pipeline_activity::NotulistPipeline,
    },
};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::voice_dictation_group::notulist_dictation_reading::{
        DICTATION_DAEMON, engine_sections, pipeline_sections,
    },
    widgets::{
        continuous_reading::ContinuousReading,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        unit_switch::{UnitSwitch, UnitSwitchMessage},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

const DICTATION_UNIT: &str = "notulist.service";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceDictationTarget {
    Dictation,
    Daemon,
}

impl From<VoiceDictationTarget> for HoverTarget {
    fn from(target: VoiceDictationTarget) -> Self {
        HoverTarget::VoiceDictation(target)
    }
}

pub struct VoiceDictationGroup {
    dictation: ContinuousReading<NotulistDictation>,
    engine: ContinuousReading<NotulistEngine>,
    pipeline: ContinuousReading<NotulistPipeline>,
    daemon: UnitSwitch,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum VoiceDictationMessage {
    Dictation(SourceDelivery<NotulistDictation>),
    Engine(SourceDelivery<NotulistEngine>),
    Pipeline(SourceDelivery<NotulistPipeline>),
    ToggleRecording,
    Daemon(UnitSwitchMessage),
    Expansion(ExpansionMessage),
}

impl Default for VoiceDictationGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceDictationGroup {
    #[must_use]
    pub fn new() -> Self {
        let dictation_unit = ScopedSystemdUnit {
            scope: SystemdUnitScope::UserSession,
            unit_name: DICTATION_UNIT.to_owned(),
        };
        Self {
            dictation: ContinuousReading::default(),
            engine: ContinuousReading::default(),
            pipeline: ContinuousReading::default(),
            daemon: UnitSwitch::new(dictation_unit, DICTATION_DAEMON),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<VoiceDictationMessage> {
        Subscription::batch([
            source_subscription::<NotulistDictation>(buses.clone())
                .map(VoiceDictationMessage::Dictation),
            source_subscription::<NotulistEngine>(buses.clone()).map(VoiceDictationMessage::Engine),
            source_subscription::<NotulistPipeline>(buses.clone())
                .map(VoiceDictationMessage::Pipeline),
            self.daemon
                .subscription(buses)
                .map(VoiceDictationMessage::Daemon),
        ])
    }

    pub fn refresh(
        target: VoiceDictationTarget,
        buses: &DesktopBuses,
    ) -> Task<VoiceDictationMessage> {
        match target {
            VoiceDictationTarget::Dictation | VoiceDictationTarget::Daemon => {
                first_delivery::<NotulistDictation>(buses.clone())
                    .map(VoiceDictationMessage::Dictation)
            }
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(
        &mut self,
        msg: VoiceDictationMessage,
        clock: Clock,
    ) -> Task<VoiceDictationMessage> {
        match msg {
            VoiceDictationMessage::Dictation(delivery) => {
                self.dictation.receive_keeping_last_reading(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::Engine(delivery) => {
                self.engine.receive_keeping_last_reading(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::Pipeline(delivery) => {
                self.pipeline.receive_keeping_last_reading(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::ToggleRecording => {
                Task::future(send_logged_dictation_command(DictationCommand::Toggle)).discard()
            }
            VoiceDictationMessage::Daemon(m) => self
                .daemon
                .update(m, clock)
                .map(VoiceDictationMessage::Daemon),
            VoiceDictationMessage::Expansion(m) => {
                self.expansion.update(m);
                Task::none()
            }
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: VoiceDictationTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            VoiceDictationTarget::Dictation => self.dictation.tooltip_with_sections(
                self.pipeline.sections_described_by(pipeline_sections),
                clock,
            ),
            VoiceDictationTarget::Daemon => self.daemon.tooltip(true, clock).map(|tooltip| {
                tooltip.with_sections(self.engine.sections_described_by(engine_sections))
            }),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, VoiceDictationMessage> {
        let dictation = self.dictation.get();
        GroupContent::new(
            &self.expansion,
            VoiceDictationMessage::Expansion,
            [
                self.dictation
                    .group_item(VoiceDictationTarget::Dictation)
                    .map(|item| {
                        item.on_press(
                            dictation.map_or(
                                "start dictating at the cursor",
                                notulist_dictation_reading::press_does,
                            ),
                            VoiceDictationMessage::ToggleRecording,
                        )
                        .previewed_on_handle()
                    }),
                self.daemon.group_item(
                    true,
                    VoiceDictationTarget::Daemon,
                    VoiceDictationMessage::Daemon(UnitSwitchMessage::Press),
                ),
            ]
            .into_iter()
            .flatten(),
        )
        .face(self.handle_face())
    }

    fn handle_face(&self) -> HandleFace<'_> {
        let glyph = Phosphor::UserSound(Weight::Regular).into();
        let handle = GroupHandle::VoiceDictation;
        match self.dictation.display() {
            Some(reading) => HandleFace::IconColoredBy {
                glyph,
                reading,
                handle,
            },
            None => HandleFace::Icon { glyph, handle },
        }
    }
}
