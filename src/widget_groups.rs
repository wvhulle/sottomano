//! Bar composition, ported from odin's `WidgetGroup`/`MenuGroup`/`shell.qml`. `layout`
//! encodes the exact top/bottom zone composition and order from `shell.qml`; each zone
//! holds ordered [`Group`]s, one file per bar section under this module, mirroring odin's
//! one-file-per-`WidgetGroup/*.qml` convention.

use iced::widget::Row;
use iced::{Alignment, Element};

use crate::bar_program::Message;
use crate::layer_surface::Position;
use crate::concrete_widgets::{Align, Context, MenuId, Widget, Zone};
use crate::visual_elements::{edge_overlay, hairline};

pub mod battery_group;
pub mod clock_group;
pub mod system_group;
pub mod window_list_group;
pub mod workspaces_group;

/// A unit on the bar. `Static`/`Menu` mirror odin's `WidgetGroup`/`MenuGroup`; the rest
/// are niri-native groups.
pub enum Group {
    /// A row of leaf widgets painted straight on the bar (odin's `WidgetGroup`).
    Static(Vec<Widget>),
    /// An expandable group (odin's `MenuGroup`): `handle` is always shown; `content`
    /// reveals in place when this menu is the open one.
    Menu {
        id: MenuId,
        handle: Widget,
        content: Vec<Widget>,
    },
    /// The niri workspace pager (bottom-left).
    Workspaces,
    /// The niri window list for the output's active workspace (bottom-center).
    WindowList,
}

impl Group {
    pub(crate) fn view<'a>(&self, ctx: &Context<'a>) -> Option<Element<'a, Message>> {
        match self {
            Group::Static(widgets) => static_group(ctx, widgets),
            Group::Menu { id, handle, content } => menu_group(ctx, *id, *handle, content),
            Group::Workspaces => {
                Some(crate::concrete_widgets::workspace_pager::workspaces(ctx))
            }
            Group::WindowList => crate::concrete_widgets::window_title_list::window_list(ctx),
        }
    }
}

/// A static group: the visible leaf widgets in a padded row, or `None` if all hidden.
fn static_group<'a>(ctx: &Context<'a>, widgets: &[Widget]) -> Option<Element<'a, Message>> {
    let items: Vec<Element<'a, Message>> = widgets.iter().filter_map(|w| w.view(ctx)).collect();
    if items.is_empty() {
        return None;
    }
    Some(
        Row::with_children(items)
            .spacing(ctx.config.layout.horizontal_widget_spacing as f32)
            .align_y(Alignment::Center)
            .into(),
    )
}

/// An expandable menu group: the handle, plus content when open, under a hairline that
/// marks it expandable (faint at rest, brighter on hover, solid when open). The whole
/// group toggles on click; hover drives the hairline. `None` if the handle is unavailable.
fn menu_group<'a>(
    ctx: &Context<'a>,
    id: MenuId,
    handle: Widget,
    content: &[Widget],
) -> Option<Element<'a, Message>> {
    let handle_view = handle.view(ctx)?;
    let open = ctx.open_menu == Some(id);

    let mut items = vec![handle_view];
    if open {
        items.extend(content.iter().filter_map(|w| w.view(ctx)));
    }
    let inner = Row::with_children(items)
        .spacing(ctx.config.layout.horizontal_widget_spacing as f32)
        .align_y(Alignment::Center);

    let opacity = if open {
        1.0
    } else if ctx.hovered_menu == Some(id) {
        0.5
    } else {
        0.2
    };
    let indicator = hairline(ctx, ctx.config.palette.power_tint.0, opacity);

    Some(
        iced::widget::mouse_area(edge_overlay(ctx, inner.into(), indicator))
            .on_press(Message::ToggleMenu(id))
            .on_enter(Message::HoverMenu(Some(id)))
            .on_exit(Message::HoverMenu(None))
            .into(),
    )
}

/// The top and bottom bars' zones, in `shell.qml` order. Groups whose data sources aren't
/// ported yet (Weather, network, media, audio, tray, notifications, quicklaunch, …) are
/// omitted for now rather than shown empty.
pub fn layout(position: Position) -> Vec<Zone> {
    match position {
        Position::Top => vec![
            Zone {
                align: Align::Start,
                groups: vec![battery_group::group(), system_group::group()],
            },
            Zone {
                align: Align::Center,
                groups: vec![clock_group::group()],
            },
        ],
        Position::Bottom => vec![
            Zone {
                align: Align::Start,
                groups: vec![workspaces_group::group()],
            },
            Zone {
                align: Align::Center,
                groups: vec![window_list_group::group()],
            },
        ],
    }
}
