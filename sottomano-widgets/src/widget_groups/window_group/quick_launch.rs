use std::{collections::HashMap, sync::Arc};

use iced::Task;
use sottomano_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::{desktop_entry_app_icons::DesktopEntries, niri_action_requests},
};

use crate::{
    desktop_index::DesktopIndex,
    group_items::group_item::{
        ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_handle::IconHandle,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const MAX_ITEMS: usize = 4;

#[derive(Default)]
pub struct QuickLaunch {
    ranked: HashMap<String, Vec<String>>,
    desktop_entries: Option<Arc<DesktopEntries>>,
    pub(super) expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum QuickLaunchMessage {
    Expansion(WidgetGroupExpansionMessage),
    Launch(String),
}

impl QuickLaunch {
    pub fn set_desktop_entries(&mut self, entries: Arc<DesktopEntries>) {
        self.desktop_entries = Some(entries);
    }

    pub fn set_ranked(&mut self, ranked: HashMap<String, Vec<String>>) {
        self.ranked = ranked;
    }

    #[must_use]
    pub fn hover_card_for(app_id: &str) -> HoverCard {
        HoverCard::sourced(
            HoverCardBody::dictionary([LabeledValue::new("App", app_id.to_owned())]),
            DeliveryTiming::LivePush,
        )
    }

    #[must_use]
    pub fn icon_keys(&self) -> Vec<String> {
        self.ranked.values().flatten().cloned().collect()
    }

    pub fn update(&mut self, msg: QuickLaunchMessage) -> Task<QuickLaunchMessage> {
        match msg {
            QuickLaunchMessage::Expansion(m) => {
                self.expansion.update(m);
            }
            QuickLaunchMessage::Launch(app_id) => return self.launch(app_id),
        }
        Task::none()
    }

    fn launch(&self, app_id: String) -> Task<QuickLaunchMessage> {
        let command = self
            .desktop_entries
            .as_ref()
            .and_then(|entries| entries.exec_command(&app_id))
            .unwrap_or_else(|| vec![app_id]);
        Task::future(blocking("quicklaunch launch", move || {
            niri_action_requests::send_action_or_warn(&niri_ipc::Action::Spawn { command });
        }))
        .discard()
    }

    pub fn group_content<'a>(
        &'a self,
        index: &'a DesktopIndex,
        output: &str,
    ) -> WidgetGroupContent<'a, QuickLaunchMessage> {
        let app_ids = self.ranked.get(output).map_or(&[][..], Vec::as_slice);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::QuickLaunch,
            &self.expansion,
            QuickLaunchMessage::Expansion,
            app_ids
                .iter()
                .filter_map(move |app_id| {
                    let icon_path = index.app_icon(app_id)?;
                    Some(
                        GroupItem::composed(
                            ComposedGroupItemFace {
                                name: app_id.clone().into(),
                                icon: GroupItemIcon::Image(IconHandle::at(icon_path)),
                            },
                            GroupItemHoverArea::WholeItem(HoverTarget::QuickLaunch(app_id.clone())),
                            GroupItemValue::Nothing,
                        )
                        .on_press("launch", QuickLaunchMessage::Launch(app_id.clone())),
                    )
                })
                .take(MAX_ITEMS),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::Rocket(Weight::Regular).into(),
        })
    }
}
