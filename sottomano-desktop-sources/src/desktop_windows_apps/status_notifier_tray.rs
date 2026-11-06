use std::{
    future::ready,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::{Stream, StreamExt, stream};
use image::RgbaImage;
pub use system_tray::item::Status;
use system_tray::{
    client::{Client, Event, UpdateEvent},
    error::Error as TrayHostError,
    item::{IconPixmap, StatusNotifierItem},
    menu::{MenuItem, MenuType, ToggleState, ToggleType, TrayMenu},
};
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};

use crate::{
    SnapshotDelivery, SnapshotSource,
    desktop_windows_apps::desktop_entry_app_icons::DesktopEntries, non_empty_text::non_empty,
};

#[derive(Debug, Clone, PartialEq)]
pub enum IconSource {
    Pixmap(Arc<RgbaImage>),
    Themed(PathBuf),
    ThemedSymbolic(PathBuf),
}

const SYMBOLIC_ICON_SUFFIX: &str = "-symbolic";

impl IconSource {
    fn pixmap(&self) -> Option<&Arc<RgbaImage>> {
        match self {
            Self::Pixmap(pixmap) => Some(pixmap),
            Self::Themed(_) | Self::ThemedSymbolic(_) => None,
        }
    }

    fn from_theme_file(theme_file: &Path) -> Self {
        let trimmed_file = super::icon_whitespace_trim::trimmed(theme_file);
        let is_svg = theme_file
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"));
        let is_symbolic = is_svg
            && theme_file
                .file_stem()
                .and_then(|file_stem| file_stem.to_str())
                .is_some_and(|file_stem| file_stem.ends_with(SYMBOLIC_ICON_SUFFIX));
        if is_symbolic {
            Self::ThemedSymbolic(trimmed_file)
        } else {
            Self::Themed(trimmed_file)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuNode {
    pub id: i32,
    pub label: Option<String>,
    pub enabled: bool,
    pub kind: MenuKind,
    pub icon: Option<IconSource>,
    pub children: Vec<MenuNode>,
}

impl MenuNode {
    fn pixmaps(&self) -> Vec<&Arc<RgbaImage>> {
        self.icon
            .iter()
            .filter_map(IconSource::pixmap)
            .chain(self.children.iter().flat_map(MenuNode::pixmaps))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Standard,
    Separator,
    Submenu,
    Toggle(bool),
}

#[derive(Debug, Clone)]
pub struct Tip {
    pub title: Option<String>,
    pub body: Option<String>,
}

impl Tip {
    fn of(title: String, description: String) -> Option<Self> {
        let tip = Self {
            title: non_empty(title),
            body: non_empty(description),
        };
        (tip.title.is_some() || tip.body.is_some()).then_some(tip)
    }
}

#[derive(Debug, Clone)]
pub struct TrayItem {
    pub address: String,
    pub id: String,
    pub title: Option<String>,
    pub status: Status,
    pub tooltip: Option<Tip>,
    pub menu_path: Option<String>,
    pub icon: Option<IconSource>,
    pub menu: Vec<MenuNode>,
}

impl TrayItem {
    #[must_use]
    pub fn name(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.id)
    }

    pub fn pixmaps(&self) -> impl Iterator<Item = &Arc<RgbaImage>> {
        self.icon
            .iter()
            .filter_map(IconSource::pixmap)
            .chain(self.menu.iter().flat_map(MenuNode::pixmaps))
    }
}

#[derive(Debug, Clone)]
pub enum TrayUpdate {
    Connected(TrayHost),
    Items(Vec<TrayItem>),
    HostFailed(Arc<TrayHostError>),
}

#[derive(Debug, Clone)]
pub struct TrayHost(Arc<Client>);

impl TrayHost {
    #[must_use]
    pub fn tray_client(&self) -> &Client {
        &self.0
    }
}

fn best_pixmap(pixmaps: &[IconPixmap]) -> Option<RgbaImage> {
    let best = pixmaps
        .iter()
        .filter(|p| {
            p.width > 0
                && p.height > 0
                && i64::try_from(p.pixels.len()).unwrap_or(0)
                    == i64::from(p.width) * i64::from(p.height) * 4
        })
        .max_by_key(|p| i64::from(p.width) * i64::from(p.height))?;
    let (width, height) = (
        u32::try_from(best.width).ok()?,
        u32::try_from(best.height).ok()?,
    );
    let mut bytes = best.pixels.clone();
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.rotate_left(1);
    }
    RgbaImage::from_raw(width, height, bytes).map(super::icon_whitespace_trim::trim_rgba)
}

const UNIDENTIFIED_APP_ICON: &str = "application-x-executable";

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
    if let Some(pixmap) = icon_pixmap.and_then(best_pixmap) {
        return Some(IconSource::Pixmap(Arc::new(pixmap)));
    }
    icon_name
        .filter(|n| !n.is_empty())
        .and_then(|name| {
            symbolic_first_names(name)
                .into_iter()
                .find_map(|candidate_name| {
                    icon_theme_path
                        .and_then(|dir| from_theme_path(dir, &candidate_name))
                        .or_else(|| entries.theme_icon(&candidate_name))
                })
        })
        .or_else(|| entries.theme_icon(UNIDENTIFIED_APP_ICON))
        .map(|path| IconSource::from_theme_file(&path))
}

fn menu_icon(item: &MenuItem, entries: &DesktopEntries) -> Option<IconSource> {
    if let Some(decoded) = item
        .icon_data
        .as_deref()
        .and_then(|data| image::load_from_memory(data).ok())
    {
        return Some(IconSource::Pixmap(Arc::new(
            super::icon_whitespace_trim::trim_rgba(decoded.into_rgba8()),
        )));
    }
    let name = item.icon_name.as_deref().filter(|n| !n.is_empty())?;
    symbolic_first_names(name)
        .into_iter()
        .find_map(|candidate_name| entries.theme_icon(&candidate_name))
        .map(|path| IconSource::from_theme_file(&path))
}

fn symbolic_first_names(icon_name: &str) -> Vec<String> {
    if icon_name.ends_with(SYMBOLIC_ICON_SUFFIX) {
        vec![icon_name.to_string()]
    } else {
        vec![
            format!("{icon_name}{SYMBOLIC_ICON_SUFFIX}"),
            icon_name.to_string(),
        ]
    }
}

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
        label: item.label.as_deref().map(clean_label).and_then(non_empty),
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
    Tip::of(tip.title.clone(), tip.description.clone())
}

fn build_item(
    address: &str,
    item: &StatusNotifierItem,
    menu: Option<&TrayMenu>,
    entries: &DesktopEntries,
) -> TrayItem {
    TrayItem {
        address: address.to_string(),
        id: item.id.clone(),
        title: item.title.clone().and_then(non_empty),
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

pub fn updates() -> impl Stream<Item = TrayUpdate> + Send {
    stream::once(Client::new()).flat_map(|connection| match connection {
        Ok(client) => hosted_updates(Arc::new(client)).left_stream(),
        Err(host_failure) => {
            stream::iter([TrayUpdate::HostFailed(Arc::new(host_failure))]).right_stream()
        }
    })
}

fn hosted_updates(client: Arc<Client>) -> impl Stream<Item = TrayUpdate> + Send {
    let item_events = BroadcastStream::new(client.subscribe());
    let entries = DesktopEntries::empty();
    let listed = listed_items(&client, &entries);
    let host = TrayHost(client.clone());
    let item_changes = item_events.scan(listed.clone(), move |items, received| {
        match received {
            Ok(event) => apply(items, event, &client, &entries),
            Err(BroadcastStreamRecvError::Lagged(_)) => *items = listed_items(&client, &entries),
        }
        ready(Some(TrayUpdate::Items(items.clone())))
    });
    stream::iter([TrayUpdate::Connected(host), TrayUpdate::Items(listed)]).chain(item_changes)
}

impl SnapshotSource for Vec<TrayItem> {
    type Error = Arc<TrayHostError>;
    type StreamInputs = ();

    const NAME: &'static str = "Tray";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        updates()
            .filter_map(|update| {
                ready(match update {
                    TrayUpdate::Items(items) => Some(Ok(items)),
                    TrayUpdate::HostFailed(failure) => Some(Err(failure)),
                    TrayUpdate::Connected(_) => None,
                })
            })
            .map(SnapshotDelivery::live_push)
    }
}

fn listed_items(client: &Client, entries: &DesktopEntries) -> Vec<TrayItem> {
    client.items().lock().map_or_else(
        |_| Vec::new(),
        |map| {
            map.iter()
                .map(|(address, (item, menu))| build_item(address, item, menu.as_ref(), entries))
                .collect()
        },
    )
}

fn find_mut<'a>(items: &'a mut [TrayItem], address: &str) -> Option<&'a mut TrayItem> {
    items.iter_mut().find(|item| item.address == address)
}

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
            let menu = matches!(update, UpdateEvent::Menu(_) | UpdateEvent::MenuDiff(_))
                .then(|| menu_from_map(client, &address, entries));
            let Some(item) = find_mut(items, &address) else {
                return;
            };
            match update {
                UpdateEvent::Icon {
                    icon_name,
                    icon_pixmap,
                } => {
                    item.icon =
                        resolve_visual(icon_name.as_deref(), icon_pixmap.as_deref(), None, entries);
                }
                UpdateEvent::Status(status) => item.status = status,
                UpdateEvent::Title(title) => item.title = title.and_then(non_empty),
                UpdateEvent::Tooltip(tooltip) => {
                    item.tooltip = tooltip.and_then(|tip| Tip::of(tip.title, tip.description));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbolic_svg_is_tinted_by_the_panel() {
        let theme_file = Path::new("/absent/icons/page.codeberg.wvhulle.Bindrune-symbolic.svg");
        assert_eq!(
            IconSource::from_theme_file(theme_file),
            IconSource::ThemedSymbolic(theme_file.to_path_buf())
        );
    }

    #[test]
    fn full_colour_svg_keeps_its_colours() {
        let theme_file = Path::new("/absent/icons/page.codeberg.wvhulle.Bindrune.svg");
        assert_eq!(
            IconSource::from_theme_file(theme_file),
            IconSource::Themed(theme_file.to_path_buf())
        );
    }

    #[test]
    fn plain_icon_name_tries_its_symbolic_variant_first() {
        assert_eq!(
            symbolic_first_names("nm-device-wired-secure"),
            ["nm-device-wired-secure-symbolic", "nm-device-wired-secure"]
        );
    }

    #[test]
    fn symbolic_icon_name_is_looked_up_once() {
        assert_eq!(
            symbolic_first_names("network-wired-symbolic"),
            ["network-wired-symbolic"]
        );
    }

    #[test]
    fn symbolic_png_is_not_loaded_as_svg() {
        let theme_file = Path::new("/absent/icons/network-wireless-symbolic.png");
        assert_eq!(
            IconSource::from_theme_file(theme_file),
            IconSource::Themed(theme_file.to_path_buf())
        );
    }
}
