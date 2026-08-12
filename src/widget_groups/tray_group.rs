use iced::{
    Alignment, Element, Size,
    widget::{Column, Row, button, mouse_area},
    window::Id,
};

use crate::{
    config::Config,
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
        flat_button,
        menu_group::{MenuGroup, MenuMessage},
        native_icon::native_icon,
    },
};

#[derive(Default)]
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
    OpenMenu(usize),
    MenuItemClicked(i32),
    ToggleSubmenu(i32),
    MenuResized { surface: Id, size: Size },
    Tooltip(TooltipMessage),
}

impl TrayGroup {
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
            TrayMessage::OpenMenu(_)
            | TrayMessage::MenuItemClicked(_)
            | TrayMessage::ToggleSubmenu(_)
            | TrayMessage::MenuResized { .. }
            | TrayMessage::Tooltip(_) => {}
        }
    }

    pub fn command(&self, command: TrayCommand) {
        if let Some(handle) = &self.handle {
            handle.send(command);
        }
    }

    pub fn visible<'a>(&'a self, config: &Config) -> Vec<&'a TrayItem> {
        self.items
            .iter()
            .filter(|item| !denied(config, item))
            .collect()
    }

    pub fn view<'a>(
        &'a self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, TrayMessage>> {
        let visible = self.visible(config);
        if visible.is_empty() {
            return None;
        }
        let reveal = self.menu.reveal();

        let handle = button(
            Solid::LayerGroup
                .glyph()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(TrayMessage::Menu(MenuMessage::Toggle))
        .into();

        let icons = visible
            .iter()
            .enumerate()
            .map(|(index, item)| item_view(config, item, index, reveal));
        let content = Row::with_children(icons)
            .spacing(config.spacing.widget_spacing_px)
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

fn denied(config: &Config, item: &TrayItem) -> bool {
    let id = item.id.to_lowercase();
    config
        .tray_deny_ids
        .iter()
        .any(|deny| !deny.is_empty() && id.contains(deny))
}

fn item_tooltip(item: &TrayItem) -> Tooltip {
    let (title, body) = match &item.tooltip {
        Some(tip) if !tip.title.is_empty() => (tip.title.clone(), tip.body.clone()),
        Some(tip) => (item.name().to_string(), tip.body.clone()),
        None => (item.name().to_string(), String::new()),
    };
    Tooltip::of(TooltipBody::Text { title, body })
}

fn item_view<'a>(
    config: &Config,
    item: &TrayItem,
    index: usize,
    reveal: f32,
) -> Element<'a, TrayMessage> {
    let side = config.font.icon_size_px;
    let glyph: Element<'a, TrayMessage> = match &item.icon {
        Some(source) => native_icon(source, side, reveal),
        None => Solid::Question
            .glyph()
            .size(side)
            .color(config.palette.bar_foreground.scale_alpha(reveal))
            .into(),
    };

    let marked: Element<'a, TrayMessage> = if item.status == Status::NeedsAttention {
        Column::with_children([
            glyph,
            activity_line(
                config.palette.focus,
                reveal,
                config.widgets.focus_underline_thickness_px,
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
