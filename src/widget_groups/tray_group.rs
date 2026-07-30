//! odin's `Tray`: the `StatusNotifierItem` host rendered on the bar. Each item
//! is a native icon (never a substituted glyph); the whole row lives behind a
//! [`MenuGroup`] handle that unfolds it with the shell's shared reveal
//! animation. Left-click activates an item, middle-click is its secondary
//! action, and right-click opens its `com.canonical.dbusmenu` context menu
//! (rendered by [`widgets::context_menu`](crate::widgets::context_menu) on a
//! popup surface the bar owns — see [`tray_menu`](super::tray_menu)).
//!
//! Items the shell already surfaces natively (network, bluetooth, media) are
//! hidden by `config.tray_deny_ids`; an item advertising `NeedsAttention` is
//! marked with the focus accent instead of relying on the app's own blink.

use iced::{
    Alignment, Element, Size,
    widget::{Column, Row, mouse_area},
    window::Id,
};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{
        Tooltip, TooltipBody,
        font_awesome::Solid,
        tray::{self, Status, TrayCommand, TrayHandle, TrayItem, TrayUpdate},
    },
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        activity_line::activity_line,
        menu_group::{MenuGroup, MenuMessage},
        native_icon::native_icon,
    },
};

pub struct TrayGroup {
    items: Vec<TrayItem>,
    handle: Option<TrayHandle>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum TrayMessage {
    Update(TrayUpdate),
    Menu(MenuMessage),
    Activate(String),
    Secondary(String),
    /// Right-click: open (or re-close) the item's context menu, by its index in
    /// the visible row. The bar owns the popup surface, so it intercepts this
    /// and every variant below.
    OpenMenu(usize),
    /// A standard/toggle menu entry was clicked (dbusmenu node id).
    MenuItemClicked(i32),
    /// A submenu header was clicked (dbusmenu node id): expand/collapse it.
    ToggleSubmenu(i32),
    /// The menu popup surface reported its measured size.
    MenuResized {
        surface: Id,
        size: Size,
    },
    Tooltip(TooltipMessage),
}

impl TrayGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            items: Vec::new(),
            handle: None,
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn subscription() -> iced::Subscription<TrayMessage> {
        tray::subscription().map(TrayMessage::Update)
    }

    pub fn update(&mut self, message: TrayMessage) {
        match message {
            TrayMessage::Update(TrayUpdate::Connected(handle)) => self.handle = Some(handle),
            TrayMessage::Update(TrayUpdate::Items(items)) => self.items = items,
            TrayMessage::Menu(m) => {
                self.menu.update(m);
            }
            TrayMessage::Activate(address) => self.command(TrayCommand::Activate(address)),
            TrayMessage::Secondary(address) => self.command(TrayCommand::Secondary(address)),
            // Every context-menu message is intercepted by the bar (it owns the
            // popup surface), so they never reach here.
            TrayMessage::OpenMenu(_)
            | TrayMessage::MenuItemClicked(_)
            | TrayMessage::ToggleSubmenu(_)
            | TrayMessage::MenuResized { .. }
            | TrayMessage::Tooltip(_) => {}
        }
    }

    /// Issue a tray command over the connected handle (a no-op until
    /// connected).
    pub fn command(&self, command: TrayCommand) {
        if let Some(handle) = &self.handle {
            handle.send(command);
        }
    }

    /// The tray items the shell should show — those not hidden by the
    /// deny-list. Order matches the underlying item order, so a visible
    /// index is stable enough to address a context menu within one frame.
    pub fn visible<'a>(&'a self, config: &ResolvedConfig) -> Vec<&'a TrayItem> {
        self.items
            .iter()
            .filter(|item| !denied(config, item))
            .collect()
    }

    pub fn view<'a>(
        &'a self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, TrayMessage>> {
        let visible = self.visible(config);
        if visible.is_empty() {
            return None;
        }
        let reveal = self.menu.reveal();

        let handle = mouse_area(
            Solid::LayerGroup
                .widget()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .on_press(TrayMessage::Menu(MenuMessage::Toggle))
        .into();

        let icons = visible
            .iter()
            .enumerate()
            .map(|(index, item)| item_view(config, item, index, reveal));
        let content = Row::with_children(icons)
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);

        Some(self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            TrayMessage::Menu,
        ))
    }
}

/// Whether an item is hidden because the shell already surfaces it natively.
fn denied(config: &ResolvedConfig, item: &TrayItem) -> bool {
    let id = item.id.to_lowercase();
    config
        .tray_deny_ids
        .iter()
        .any(|deny| !deny.is_empty() && id.contains(deny))
}

/// The static tooltip content an item carries: its advertised
/// title/description, falling back to the item name so a hover always
/// identifies the app.
fn item_tooltip(item: &TrayItem) -> Tooltip {
    let (title, body) = match &item.tooltip {
        Some(tip) if !tip.title.is_empty() => (tip.title.clone(), tip.body.clone()),
        Some(tip) => (item.name().to_string(), tip.body.clone()),
        None => (item.name().to_string(), String::new()),
    };
    Tooltip {
        body: TooltipBody::Text { title, body },
        footer: None,
    }
}

fn item_view<'a>(
    config: &ResolvedConfig,
    item: &TrayItem,
    index: usize,
    reveal: f32,
) -> Element<'a, TrayMessage> {
    let side = config.font.icon_size_px;
    let glyph: Element<'a, TrayMessage> = match &item.icon {
        Some(source) => native_icon(source, side, reveal),
        None => Solid::Question
            .widget()
            .size(side)
            .color(config.palette.bar_foreground.scale_alpha(reveal))
            .into(),
    };

    // An item needing attention gets a focus-accent underline — the shell's own
    // active-state vocabulary — instead of the app's blink.
    let marked: Element<'a, TrayMessage> = if item.status == Status::NeedsAttention {
        Column::with_children([
            glyph,
            activity_line(
                config.palette.focus,
                reveal,
                config.layout.focus_underline_thickness_px,
            ),
        ])
        .spacing(1.0)
        .align_x(Alignment::Center)
        .into()
    } else {
        glyph
    };

    let hover = hoverable(
        marked,
        HoverTarget::Tray(Box::new(item_tooltip(item))),
        TrayMessage::Tooltip,
    );
    mouse_area(hover)
        .on_press(TrayMessage::Activate(item.address.clone()))
        .on_middle_press(TrayMessage::Secondary(item.address.clone()))
        .on_right_press(TrayMessage::OpenMenu(index))
        .into()
}
