use iced::Task;

use crate::{
    app::Message,
    config::Config,
    state::PanelLayout,
    wayland_layers::{
        niri_rules::{NiriRulesFile, StoredRules},
        tooltip::HoverTarget,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        toggle_control::icon_toggle_item,
        tooltip_body::{Tooltip, row},
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
    },
};

#[derive(Debug, Clone)]
pub enum NiriMessage {
    Loaded(StoredRules),
    Expansion(ExpansionMessage),
    ToggleBlur,
    ToggleXray,
}

pub struct NiriGroup {
    pub(crate) expansion: GroupExpansion,
    rules: NiriRulesFile,
}

pub fn switch_tooltip(label: &str, on: bool, described: &str) -> Tooltip {
    Tooltip::table(vec![row(label, if on { described } else { "off" })])
}

impl NiriGroup {
    pub fn new(rules: NiriRulesFile) -> Self {
        Self {
            expansion: GroupExpansion::default(),
            rules,
        }
    }

    pub fn load(&self) -> Task<Message> {
        self.rules
            .read()
            .map(|stored| Message::NiriStyling(NiriMessage::Loaded(stored)))
    }

    pub fn restyle(&mut self, layout: &PanelLayout) -> Task<Message> {
        self.rules.write(layout).discard()
    }

    pub fn tooltip_for(target: &HoverTarget, layout: &PanelLayout) -> Option<Tooltip> {
        let niri = layout.niri;
        match target {
            HoverTarget::BlurBackground => Some(switch_tooltip(
                "Blur",
                niri.blurred,
                "on — frosted behind the panels",
            )),
            HoverTarget::Xray => Some(switch_tooltip(
                "Xray",
                niri.xray,
                "on — blur shows only the wallpaper",
            )),
            _ => None,
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, msg: NiriMessage, layout: &mut PanelLayout) -> Task<Message> {
        match msg {
            NiriMessage::Loaded(stored) => {
                self.rules.adopt(stored, layout);
                self.restyle(layout)
            }
            NiriMessage::Expansion(menu) => {
                self.expansion.update(menu);
                Task::none()
            }
            NiriMessage::ToggleBlur => {
                layout.niri.blurred = !layout.niri.blurred;
                self.restyle(layout)
            }
            NiriMessage::ToggleXray => {
                layout.niri.xray = !layout.niri.xray;
                self.restyle(layout)
            }
        }
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
        layout: &PanelLayout,
    ) -> WidgetGroup<'a, NiriMessage> {
        let niri = layout.niri;
        WidgetGroup::new(&self.expansion, NiriMessage::Expansion)
            .icon(Phosphor::AppWindow(Weight::Regular))
            .item(icon_toggle_item(
                config,
                Phosphor::Snowflake(Weight::Regular),
                niri.blurred,
                NiriMessage::ToggleBlur,
                HoverTarget::BlurBackground,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::Eye(Weight::Regular),
                niri.xray,
                NiriMessage::ToggleXray,
                HoverTarget::Xray,
            ))
    }
}
