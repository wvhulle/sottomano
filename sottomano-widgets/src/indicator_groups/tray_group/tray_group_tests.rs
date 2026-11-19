use corredo::{orientation::Axis, transitions::reveal};
use iced::{
    Settings, Size,
    widget::{row, text},
};
use iced_test::Simulator;
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        status_notifier_tray::{Status, TrayItem, TrayItemIcon},
        window_edge_placement::WindowOwner,
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use sottomano_toml_config::{
    Config,
    indicator_groups::tray::{ClickAction, TrayRule},
};

use super::{TrayItemWindow, TrayMessage, click_hint, click_message, item_view};
use crate::icon_catalog::{
    drawable_icon_catalog::{DrawableIconSources, IconCatalog},
    pixmap_handles::PixmapHandles,
};

const PANEL_WIDTH: f32 = 800.0;
const MARKER_AFTER_ITEM: &str = "marker after the tray item";

fn item_needing_attention() -> TrayItem {
    TrayItem {
        address: ":1.4167".to_owned(),
        owner: OwnerProcess::Identified(4167),
        id: "chat".to_owned(),
        title: None,
        status: Status::NeedsAttention,
        tooltip: None,
        menu_path: None,
        icon: TrayItemIcon::NoIcon,
        menu: Vec::new(),
    }
}

#[test]
fn an_item_needing_attention_keeps_a_finite_width_while_revealed() {
    let config = Config::default();
    let pixmap_handles = PixmapHandles::default();
    let icon_catalog = IconCatalog::default();
    let item = item_needing_attention();
    let revealed = reveal(
        item_view(
            &config,
            DrawableIconSources {
                icon_catalog: &icon_catalog,
                pixmap_handles: &pixmap_handles,
            },
            &item,
            &TrayItemWindow::NotOpen,
            1.0,
        ),
        1.0,
        Axis::Horizontal,
    );
    let mut simulator = Simulator::with_size(
        Settings::default(),
        Size::new(PANEL_WIDTH, 40.0),
        row![revealed, text(MARKER_AFTER_ITEM)],
    );
    let marker = simulator
        .find(MARKER_AFTER_ITEM)
        .expect("the marker is laid out")
        .bounds();
    assert!(
        marker.x.is_finite() && marker.x < PANEL_WIDTH,
        "the tray item pushed the marker to x = {}",
        marker.x
    );
}

fn every_window_matches(_owner: &WindowOwner) -> bool {
    true
}

fn no_window_matches(_owner: &WindowOwner) -> bool {
    false
}

#[test]
fn item_without_override_is_matched_by_its_process() {
    let item = item_needing_attention();
    assert_eq!(
        TrayItemWindow::resolve(&TrayRule::default(), &item, &every_window_matches),
        TrayItemWindow::Open(WindowOwner::Process(4167))
    );
}

#[test]
fn window_app_id_overrides_the_process() {
    let item = item_needing_attention();
    let overriding_rule = TrayRule {
        window_app_id: Some("signal".to_owned()),
        ..TrayRule::default()
    };
    assert_eq!(
        TrayItemWindow::resolve(&overriding_rule, &item, &every_window_matches),
        TrayItemWindow::Open(WindowOwner::App("signal".to_owned()))
    );
}

#[test]
fn item_with_unidentified_process_has_no_open_window() {
    let item = TrayItem {
        owner: OwnerProcess::Unidentified,
        ..item_needing_attention()
    };
    assert_eq!(
        TrayItemWindow::resolve(&TrayRule::default(), &item, &every_window_matches),
        TrayItemWindow::NotOpen
    );
}

#[test]
fn item_without_matching_window_has_no_open_window() {
    let item = item_needing_attention();
    assert_eq!(
        TrayItemWindow::resolve(&TrayRule::default(), &item, &no_window_matches),
        TrayItemWindow::NotOpen
    );
}

#[test]
fn focus_click_on_open_window_focuses_it() {
    let item = item_needing_attention();
    let item_window = TrayItemWindow::Open(WindowOwner::Process(4167));
    assert!(matches!(
        click_message(ClickAction::FocusAppWindow, &item_window, &item),
        Some(TrayMessage::FocusWindow {
            owner: WindowOwner::Process(4167),
            ..
        })
    ));
    assert_eq!(
        click_hint(ClickAction::FocusAppWindow, &item_window),
        "focus its window"
    );
}

#[test]
fn focus_click_without_open_window_activates() {
    let item = item_needing_attention();
    assert!(matches!(
        click_message(ClickAction::FocusAppWindow, &TrayItemWindow::NotOpen, &item),
        Some(TrayMessage::Activate(_))
    ));
    assert_eq!(
        click_hint(ClickAction::FocusAppWindow, &TrayItemWindow::NotOpen),
        "activate"
    );
}
