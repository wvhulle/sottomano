use std::{collections::HashMap, path::Path, sync::Arc};

use iced::{
    Element, Task,
    widget::{Row, hover, image, svg},
};
use sottomano_config::Config;
use sottomano_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::{desktop_entry_app_icons::DesktopEntries, niri_action_requests},
};

use crate::{
    desktop_index::DesktopIndex,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Freshness, KeyValue, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupEvent, GroupExpansion, HandleFace},
    },
};

const MAX_ITEMS: usize = 4;
const RESTING_OPACITY: f32 = 0.65;

#[derive(Default)]
pub struct QuickLaunch {
    ranked: HashMap<String, Vec<String>>,
    desktop_entries: Option<Arc<DesktopEntries>>,
    pub(super) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum QuickLaunchMessage {
    Expansion(ExpansionMessage),
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
    pub fn tooltip_for(app_id: &str) -> Tooltip {
        Tooltip::sourced(
            TooltipBody::dictionary([KeyValue::new("App", app_id.to_owned())]),
            Freshness::Live,
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
        config: &'a Config,
    ) -> GroupContent<'a, QuickLaunchMessage> {
        let app_ids = self.ranked.get(output).map_or(&[][..], Vec::as_slice);
        GroupContent::new(
            &self.expansion,
            QuickLaunchMessage::Expansion,
            app_ids
                .iter()
                .filter(|app_id| index.app_icon(app_id).is_some())
                .take(MAX_ITEMS)
                .map(move |app_id| {
                    GroupItem::hoverable_element(
                        HoverTarget::QuickLaunch(app_id.clone()),
                        move |reveal| {
                            icon_slot(config, index, app_id, reveal)
                                .unwrap_or_else(|| Row::new().into())
                                .map(GroupEvent::Group)
                        },
                    )
                    .on_press("launch", QuickLaunchMessage::Launch(app_id.clone()))
                }),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Rocket(Weight::Regular).into(),
            handle: GroupHandle::QuickLaunch,
        })
    }
}

fn icon_slot<'a>(
    config: &Config,
    index: &DesktopIndex,
    app_id: &str,
    reveal: f32,
) -> Option<Element<'a, QuickLaunchMessage>> {
    let path = index.app_icon(app_id)?;
    let side = config.theme.font.icon_size_px;
    let resting = icon_image(path, side, reveal * RESTING_OPACITY);
    let bright = icon_image(path, side, reveal);
    Some(hover(resting, bright))
}

fn icon_image<'a>(path: &Path, side: f32, opacity: f32) -> Element<'a, QuickLaunchMessage> {
    let is_svg = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"));
    if is_svg {
        svg(svg::Handle::from_path(path))
            .width(side)
            .height(side)
            .opacity(opacity)
            .into()
    } else {
        image(image::Handle::from_path(path))
            .width(side)
            .height(side)
            .opacity(opacity)
            .into()
    }
}
