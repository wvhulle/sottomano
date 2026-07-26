//! System tray via the freedesktop `StatusNotifierItem` protocol (odin's `Tray`),
//! backed by the `system-tray` crate — the SNI host ironbar uses. The crate
//! registers the watcher/host, discovers items, and parses each item's dbusmenu
//! into a tree delivered through its event stream, so this module is only a thin
//! adapter: fold the crate's events into our render model, decode icons, and
//! route click/menu actions back to the crate's `Client`.
//!
//! Icons are rendered as-is (odin's glyph map is intentionally not ported):
//! prefer the item's pixmap, else its themed icon name.

use std::path::{Path, PathBuf};

use iced::{
    Subscription,
    futures::{SinkExt, channel::mpsc::Sender},
    stream,
    widget::image,
};
use system_tray::{
    client::{ActivateRequest, Client, Event, UpdateEvent},
    item::{IconPixmap, StatusNotifierItem},
    menu::{MenuItem, MenuType, ToggleState, ToggleType, TrayMenu},
};
use tokio::{
    select,
    sync::{broadcast::error::RecvError, mpsc},
    time::{Duration, sleep},
};

use crate::{
    data_sources::app_icon::DesktopEntries,
    widgets::{
        context_menu::{MenuKind, MenuNode},
        native_icon::IconSource,
    },
};

pub use system_tray::item::Status;

/// The tooltip content an item advertises, when any.
#[derive(Debug, Clone)]
pub struct Tip {
    pub title: String,
    pub body: String,
}

/// One live tray item, adapted from the crate's `StatusNotifierItem` + `TrayMenu`.
#[derive(Debug, Clone)]
pub struct TrayItem {
    pub address: String,
    pub id: String,
    pub title: String,
    pub status: Status,
    pub tooltip: Option<Tip>,
    pub menu_path: Option<String>,
    pub icon: Option<IconSource>,
    pub menu: Vec<MenuNode>,
}

impl TrayItem {
    /// The label a tooltip/menu shows: the item's title, else its id.
    pub fn name(&self) -> &str {
        if self.title.is_empty() { &self.id } else { &self.title }
    }
}

/// What the subscription streams to the bar: first the command handle, then the
/// current item set on every change.
#[derive(Debug, Clone)]
pub enum TrayUpdate {
    Connected(TrayHandle),
    Items(Vec<TrayItem>),
}

/// An action the bar asks the tray to perform. Routed to the owning `Client` in
/// the subscription task, since `Client` is not `Clone`.
#[derive(Debug, Clone)]
pub enum TrayCommand {
    Activate(String),
    Secondary(String),
    MenuClick { address: String, menu_path: String, id: i32 },
}

/// A cheap, cloneable handle the bar keeps to issue [`TrayCommand`]s.
#[derive(Debug, Clone)]
pub struct TrayHandle(mpsc::Sender<TrayCommand>);

impl TrayHandle {
    pub fn send(&self, command: TrayCommand) {
        let _ = self.0.try_send(command);
    }
}

/// The largest valid pixmap, converted ARGB32→RGBA into an iced image handle.
fn best_pixmap(pixmaps: &[IconPixmap]) -> Option<image::Handle> {
    let best = pixmaps
        .iter()
        .filter(|p| {
            p.width > 0
                && p.height > 0
                && i64::try_from(p.pixels.len()).unwrap_or(0)
                    == i64::from(p.width) * i64::from(p.height) * 4
        })
        .max_by_key(|p| i64::from(p.width) * i64::from(p.height))?;
    let (width, height) = (u32::try_from(best.width).ok()?, u32::try_from(best.height).ok()?);
    let mut bytes = best.pixels.clone();
    for pixel in bytes.chunks_exact_mut(4) {
        pixel.rotate_left(1);
    }
    Some(image::Handle::from_rgba(width, height, bytes))
}

/// A themed icon shipped in the item's own `icon_theme_path`, tried before the
/// shared icon theme (some apps only ship icons in their private dir).
fn from_theme_path(theme_path: &str, name: &str) -> Option<PathBuf> {
    ["svg", "png"].into_iter().find_map(|ext| {
        let path = Path::new(theme_path).join(format!("{name}.{ext}"));
        path.exists().then_some(path)
    })
}

fn resolve_visual(
    icon_name: Option<&str>,
    icon_pixmap: Option<&[IconPixmap]>,
    icon_theme_path: Option<&str>,
    entries: &DesktopEntries,
) -> Option<IconSource> {
    if let Some(handle) = icon_pixmap.and_then(best_pixmap) {
        return Some(IconSource::Pixmap(handle));
    }
    let name = icon_name.filter(|n| !n.is_empty())?;
    icon_theme_path
        .and_then(|dir| from_theme_path(dir, name))
        .or_else(|| entries.theme_icon(name))
        .map(IconSource::Themed)
}

/// The entry's own icon: prefer inline `icon-data` (encoded image bytes), else a
/// themed `icon-name`.
fn menu_icon(item: &MenuItem, entries: &DesktopEntries) -> Option<IconSource> {
    if let Some(data) = &item.icon_data
        && !data.is_empty()
    {
        return Some(IconSource::Pixmap(image::Handle::from_bytes(data.clone())));
    }
    let name = item.icon_name.as_deref().filter(|n| !n.is_empty())?;
    entries.theme_icon(name).map(IconSource::Themed)
}

/// Adapt one crate `MenuItem` into a [`MenuNode`], dropping invisible entries.
fn adapt_menu_item(item: &MenuItem, entries: &DesktopEntries) -> Option<MenuNode> {
    if !item.visible {
        return None;
    }
    let children: Vec<MenuNode> = item
        .submenu
        .iter()
        .filter_map(|child| adapt_menu_item(child, entries))
        .collect();
    let kind = if item.menu_type == MenuType::Separator {
        MenuKind::Separator
    } else if !children.is_empty() || item.children_display.as_deref() == Some("submenu") {
        MenuKind::Submenu
    } else if item.toggle_type != ToggleType::CannotBeToggled {
        MenuKind::Toggle(item.toggle_state == ToggleState::On)
    } else {
        MenuKind::Standard
    };
    Some(MenuNode {
        id: item.id,
        label: clean_label(item.label.as_deref().unwrap_or_default()),
        enabled: item.enabled,
        kind,
        icon: menu_icon(item, entries),
        children,
    })
}

fn adapt_menu(menu: &TrayMenu, entries: &DesktopEntries) -> Vec<MenuNode> {
    menu.submenus
        .iter()
        .filter_map(|item| adapt_menu_item(item, entries))
        .collect()
}

/// Strip the `_` mnemonic markers dbusmenu labels carry (`"_Quit"` → `"Quit"`;
/// `"__"` collapses to a literal underscore).
fn clean_label(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            if chars.peek() == Some(&'_') {
                out.push('_');
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn adapt_tooltip(item: &StatusNotifierItem) -> Option<Tip> {
    let tip = item.tool_tip.as_ref()?;
    (!tip.title.is_empty() || !tip.description.is_empty()).then(|| Tip {
        title: tip.title.clone(),
        body: tip.description.clone(),
    })
}

/// Build a [`TrayItem`] from a crate item and its (optional) parsed menu.
fn build_item(
    address: &str,
    item: &StatusNotifierItem,
    menu: Option<&TrayMenu>,
    entries: &DesktopEntries,
) -> TrayItem {
    TrayItem {
        address: address.to_string(),
        id: item.id.clone(),
        title: item.title.clone().unwrap_or_default(),
        status: item.status,
        tooltip: adapt_tooltip(item),
        menu_path: item.menu.clone(),
        icon: resolve_visual(
            item.icon_name.as_deref(),
            item.icon_pixmap.as_deref(),
            item.icon_theme_path.as_deref(),
            entries,
        ),
        menu: menu.map(|m| adapt_menu(m, entries)).unwrap_or_default(),
    }
}

/// The signal-driven tray stream: the command handle once, then a fresh
/// `Vec<TrayItem>` on every change. Reconnects if the client drops.
pub fn subscription() -> Subscription<TrayUpdate> {
    Subscription::run(|| {
        stream::channel(64, |mut output: Sender<TrayUpdate>| async move {
            loop {
                run(&mut output).await;
                sleep(Duration::from_secs(2)).await;
            }
        })
    })
}

async fn run(output: &mut Sender<TrayUpdate>) {
    let client = match Client::new().await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("tray: {e}");
            return;
        }
    };
    let mut events = client.subscribe();
    let (command_tx, mut command_rx) = mpsc::channel::<TrayCommand>(64);
    if output.send(TrayUpdate::Connected(TrayHandle(command_tx))).await.is_err() {
        return;
    }

    let entries = DesktopEntries::empty();
    let mut items: Vec<TrayItem> = Vec::new();

    // Seed from the crate's current map: items discovered before/at subscribe are
    // not re-delivered over the broadcast, so read them directly.
    if let Ok(map) = client.items().lock() {
        items = map
            .iter()
            .map(|(address, (item, menu))| build_item(address, item, menu.as_ref(), &entries))
            .collect();
    }
    if output.send(TrayUpdate::Items(items.clone())).await.is_err() {
        return;
    }

    loop {
        select! {
            event = events.recv() => match event {
                Ok(event) => {
                    apply(&mut items, event, &client, &entries);
                    if output.send(TrayUpdate::Items(items.clone())).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
            command = command_rx.recv() => match command {
                Some(command) => run_command(&client, command).await,
                None => break,
            },
        }
    }
}

fn find_mut<'a>(items: &'a mut [TrayItem], address: &str) -> Option<&'a mut TrayItem> {
    items.iter_mut().find(|item| item.address == address)
}

/// Re-read one item's parsed menu from the crate's (already diff-applied) map.
fn menu_from_map(client: &Client, address: &str, entries: &DesktopEntries) -> Vec<MenuNode> {
    client
        .items()
        .lock()
        .ok()
        .and_then(|map| {
            map.get(address)
                .and_then(|(_, menu)| menu.as_ref())
                .map(|menu| adapt_menu(menu, entries))
        })
        .unwrap_or_default()
}

fn apply(items: &mut Vec<TrayItem>, event: Event, client: &Client, entries: &DesktopEntries) {
    match event {
        Event::Add(address, item) => {
            let built = build_item(&address, &item, None, entries);
            match find_mut(items, &address) {
                Some(existing) => *existing = built,
                None => items.push(built),
            }
        }
        Event::Remove(address) => items.retain(|item| item.address != address),
        Event::Update(address, update) => {
            // Menu re-reads come from the crate's map (below), so resolve that
            // before borrowing the item mutably.
            let menu = matches!(update, UpdateEvent::Menu(_) | UpdateEvent::MenuDiff(_))
                .then(|| menu_from_map(client, &address, entries));
            let Some(item) = find_mut(items, &address) else {
                return;
            };
            match update {
                UpdateEvent::Icon { icon_name, icon_pixmap } => {
                    item.icon = resolve_visual(
                        icon_name.as_deref(),
                        icon_pixmap.as_deref(),
                        None,
                        entries,
                    );
                }
                UpdateEvent::Status(status) => item.status = status,
                UpdateEvent::Title(title) => item.title = title.unwrap_or_default(),
                UpdateEvent::Tooltip(tooltip) => {
                    item.tooltip = tooltip.and_then(|tip| {
                        (!tip.title.is_empty() || !tip.description.is_empty()).then_some(Tip {
                            title: tip.title,
                            body: tip.description,
                        })
                    });
                }
                UpdateEvent::Menu(_) | UpdateEvent::MenuDiff(_) => {
                    item.menu = menu.unwrap_or_default();
                }
                UpdateEvent::MenuConnect(path) => item.menu_path = Some(path),
                UpdateEvent::AttentionIcon(_) | UpdateEvent::OverlayIcon(_) => {}
            }
        }
    }
}

async fn run_command(client: &Client, command: TrayCommand) {
    let request = match command {
        TrayCommand::Activate(address) => ActivateRequest::Default { address, x: 0, y: 0 },
        TrayCommand::Secondary(address) => ActivateRequest::Secondary { address, x: 0, y: 0 },
        TrayCommand::MenuClick { address, menu_path, id } => ActivateRequest::MenuItem {
            address,
            menu_path,
            submenu_id: id,
        },
    };
    if let Err(e) = client.activate(request).await {
        eprintln!("tray: activate: {e}");
    }
}
