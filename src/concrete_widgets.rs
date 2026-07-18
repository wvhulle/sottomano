//! The bar-level rendering glue and odin's `ConcreteWidget` leaf widgets: `Context` is
//! everything a widget needs to render itself, `Zone` composes `widget_groups::Group`s
//! into an anchored row, and `Widget` is one source-backed leaf. See `widget_groups` for
//! the group composition/dispatch layer this feeds into, and `visual_elements` for the
//! shared drawing primitives leaf widgets are drawn with.

use iced::widget::text::Shaping;
use iced::widget::{Row, text};
use iced::{Alignment, Element};
use niri_ipc::state::EventStreamState;

use crate::bar_program::Message;
use crate::layer_surface::Position;
use crate::data_sources::BarWidget;
use crate::data_sources::app_icon::DesktopEntries;
use crate::user_config::Config;
use crate::visual_elements::{font_family, leaf};
use crate::widget_groups::Group;

pub mod window_title_list;
pub mod workspace_pager;

/// Coerce a concrete source reference to `&dyn BarWidget` for uniform rendering.
pub fn as_bar_widget<W: BarWidget>(w: &W) -> &dyn BarWidget {
    w
}

/// Which expandable menu group is which — odin's single-open `GroupMenuState` keys on the
/// group identity; this is the id it keys on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuId {
    System,
}

/// Everything a widget may read to render itself: shared bar data, the surface identity
/// (per-output widgets resolve to the right screen), and the menu interaction state.
pub struct Context<'a> {
    pub config: &'a Config,
    pub clock: &'a str,
    pub battery: Option<&'a dyn BarWidget>,
    pub cpu: Option<&'a dyn BarWidget>,
    pub memory: Option<&'a dyn BarWidget>,
    pub temperature: Option<&'a dyn BarWidget>,
    pub disk: Option<&'a dyn BarWidget>,
    pub niri: &'a EventStreamState,
    /// Installed `.desktop` entries for window→icon resolution; `None` until the startup
    /// scan completes (icons are simply omitted until then).
    pub desktop_entries: Option<&'a DesktopEntries>,
    pub output: &'a str,
    pub position: Position,
    /// The one expanded menu group, if any (global single-open).
    pub open_menu: Option<MenuId>,
    /// The menu group under the pointer, for the hairline's hover state.
    pub hovered_menu: Option<MenuId>,
}

/// Where a zone anchors within the bar. Odin's three alignments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

/// A zone (odin's `Zone`): an anchored, ordered list of groups.
pub struct Zone {
    pub align: Align,
    pub groups: Vec<Group>,
}

impl Zone {
    /// The zone as a row of its visible groups, or `None` if every group is hidden.
    pub fn view<'a>(&self, ctx: &Context<'a>) -> Option<Row<'a, Message>> {
        let groups: Vec<Element<'a, Message>> = self.groups.iter().filter_map(|g| g.view(ctx)).collect();
        if groups.is_empty() {
            return None;
        }
        Some(
            Row::with_children(groups)
                .spacing(ctx.config.layout.horizontal_group_spacing as f32)
                .align_y(Alignment::Center),
        )
    }
}

/// One leaf widget. Source-backed widgets resolve their `&dyn BarWidget` from `Context`.
#[derive(Debug, Clone, Copy)]
pub enum Widget {
    Battery,
    Cpu,
    Memory,
    Temperature,
    Disk,
    Time,
}

impl Widget {
    /// `None` means "render nothing" — an unavailable source (odin hides it).
    pub(crate) fn view<'a>(self, ctx: &Context<'a>) -> Option<Element<'a, Message>> {
        match self {
            Widget::Battery => leaf(ctx, ctx.battery),
            Widget::Cpu => leaf(ctx, ctx.cpu),
            Widget::Memory => leaf(ctx, ctx.memory),
            Widget::Temperature => leaf(ctx, ctx.temperature),
            Widget::Disk => leaf(ctx, ctx.disk),
            Widget::Time => {
                let size = ctx.config.font.size as f32;
                Some(
                    text(ctx.clock.to_string())
                        .size(size)
                        .font(font_family(&ctx.config.font.family))
                        .shaping(Shaping::Advanced)
                        .into(),
                )
            }
        }
    }
}
