use corredo::{gauges::activity_line, orientation::Axis, theming::primary_color};
use iced::{
    Alignment, Element,
    widget::{Column, container, mouse_area},
};
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        status_notifier_tray::{Status, TrayItem, TrayItemIcon},
        window_edge_placement::WindowOwner,
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
    theme_icons::icon_requests::{IconRequest, ItemThemePath, tray_item_icon_request},
};
use sottomano_toml_config::{
    Config,
    panel_menus::session_menu::tray::{ClickAction, TrayRule},
};

use super::TrayMessage;
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        indicator_group_content::GroupEvent,
        indicator_state::{IndicatorState, IndicatorStateTint},
    },
    hover_cards::{MouseGesture, PressAction},
    icon_catalog::drawable_icon_catalog::{DrawableIcon, DrawableIconSources, IconFreshness},
    widgets::shared_metrics::ACCENT_STROKE_PX,
};

const UNIDENTIFIED_APP_ICON: &str = "application-x-executable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TrayItemWindow {
    Open(WindowOwner),
    NotOpen,
}

impl TrayItemWindow {
    pub(super) fn resolve(
        rule: &TrayRule,
        item: &TrayItem,
        window_matches: &dyn Fn(&WindowOwner) -> bool,
    ) -> Self {
        let owner = match &rule.window_app_id {
            Some(window_app_id) => WindowOwner::App(window_app_id.clone()),
            None => match item.owner {
                OwnerProcess::Identified(pid) => WindowOwner::Process(pid),
                OwnerProcess::Unidentified => return Self::NotOpen,
            },
        };
        if window_matches(&owner) {
            Self::Open(owner)
        } else {
            Self::NotOpen
        }
    }
}

pub(super) fn click_message(
    action: ClickAction,
    item_window: &TrayItemWindow,
    item: &TrayItem,
) -> Option<TrayMessage> {
    match (action, item_window) {
        (ClickAction::ActivateItem, _) | (ClickAction::FocusAppWindow, TrayItemWindow::NotOpen) => {
            Some(TrayMessage::Activate(item.address.clone()))
        }
        (ClickAction::SecondaryActivateItem, _) => {
            Some(TrayMessage::Secondary(item.address.clone()))
        }
        (ClickAction::FocusAppWindow, TrayItemWindow::Open(owner)) => {
            Some(TrayMessage::FocusWindow {
                owner: owner.clone(),
                address: item.address.clone(),
            })
        }
        (ClickAction::Ignore, _) => None,
    }
}

pub(super) const fn click_hint(action: ClickAction, item_window: &TrayItemWindow) -> &'static str {
    match (action, item_window) {
        (ClickAction::FocusAppWindow, TrayItemWindow::NotOpen) => {
            ClickAction::ActivateItem.press_hint()
        }
        _ => action.press_hint(),
    }
}

pub(super) fn item_press_actions(
    rule: &TrayRule,
    item_window: &TrayItemWindow,
) -> Vec<PressAction> {
    [
        (MouseGesture::LeftClick, rule.left_click),
        (MouseGesture::MiddleClick, rule.middle_click),
        (MouseGesture::RightClick, rule.right_click),
    ]
    .into_iter()
    .filter(|(_, click_action)| *click_action != ClickAction::Ignore)
    .map(|(gesture, click_action)| PressAction::new(gesture, click_hint(click_action, item_window)))
    .collect()
}

pub(super) fn unidentified_tray_icon_request() -> IconRequest {
    tray_item_icon_request(UNIDENTIFIED_APP_ICON, &ItemThemePath::Unset)
}

pub(super) fn tray_item_drawable(
    icon_sources: DrawableIconSources<'_>,
    tray_item_icon: &TrayItemIcon,
) -> Option<DrawableIcon> {
    let catalog_drawable =
        |request: &IconRequest| icon_sources.icon_catalog.drawable_icon(request).cloned();
    let unidentified_drawable = || catalog_drawable(&unidentified_tray_icon_request());
    match tray_item_icon {
        TrayItemIcon::Pixmap(pixels) => Some(DrawableIcon {
            handle: icon_sources.pixmap_handles.pixmap_handle(pixels),
            design_side_px: pixels.width().max(pixels.height()) as f32,
            freshness: IconFreshness::Current,
        }),
        TrayItemIcon::Named {
            icon_name,
            item_theme_path,
        } => catalog_drawable(&tray_item_icon_request(icon_name, item_theme_path))
            .or_else(unidentified_drawable),
        TrayItemIcon::NoIcon => unidentified_drawable(),
    }
}

pub(super) fn item_view<'view>(
    config: &Config,
    icon_sources: DrawableIconSources<'_>,
    item: &TrayItem,
    item_window: &TrayItemWindow,
    reveal: f32,
) -> Element<'view, GroupEvent> {
    let side = config.theme.font.icon_size_px;
    let tint = IndicatorStateTint {
        state: IndicatorState::Idle,
        opacity: reveal,
    };
    let glyph: Element<'view, GroupEvent> = match tray_item_drawable(icon_sources, &item.icon) {
        Some(drawable_icon) => {
            drawable_icon.view_with_symbolic_tint(side, reveal, move |theme| tint.color(theme))
        }
        None => IconGlyph::from(Phosphor::Question(Weight::Regular))
            .view(side, move |theme| tint.color(theme)),
    };

    let marked: Element<'view, GroupEvent> = if item.status == Status::NeedsAttention {
        Column::with_children([
            glyph,
            container(activity_line(
                Axis::Horizontal,
                primary_color,
                reveal,
                ACCENT_STROKE_PX,
            ))
            .width(side)
            .into(),
        ])
        .align_x(Alignment::Center)
        .into()
    } else {
        glyph
    };

    let rule = config.panel_menus.session.tray.rule(&item.id);
    let left = click_message(rule.left_click, item_window, item);
    let middle = click_message(rule.middle_click, item_window, item);
    let right = click_message(rule.right_click, item_window, item);
    let mut clickable = mouse_area(marked);
    if let Some(message) = left {
        clickable = clickable.on_press(GroupEvent::from(message));
    }
    if let Some(message) = middle {
        clickable = clickable.on_middle_press(GroupEvent::from(message));
    }
    if let Some(message) = right {
        clickable = clickable.on_right_press(GroupEvent::from(message));
    }
    clickable.into()
}
