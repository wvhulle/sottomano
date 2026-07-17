//! Bar widgets and zones. Mirrors odin's `shell.qml` model: a bar is a list of zones,
//! each zone an ordered list of widgets rendered into a row and anchored to an edge
//! (`leftZone`/`centerZone`/`rightZone`). Layout is data here, not a hardcoded widget
//! row, so Phase 2/3 grow the `Widget` enum and `layout()` rather than editing `view`.

use iced::widget::{Row, text};
use iced::{Alignment, Element, Font};
use niri_ipc::state::EventStreamState;

use crate::app::{Message, Position};
use crate::config::Config;
use crate::niri;
use crate::script::WidgetData;

/// Everything a widget may read to render itself: the shared bar data plus the identity
/// of the surface being painted, so per-output widgets (workspaces) resolve to the right
/// screen.
pub struct Context<'a> {
    pub config: &'a Config,
    pub clock: &'a str,
    pub battery: Option<&'a WidgetData>,
    pub niri: &'a EventStreamState,
    pub output: &'a str,
    pub position: Position,
}

/// Where a zone anchors within the bar. Odin's three alignments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

/// An ordered group of widgets anchored to one edge — odin's `Zone` (a `RowLayout`).
pub struct Zone {
    pub align: Align,
    pub widgets: Vec<Widget>,
}

impl Zone {
    pub fn view<'a>(&self, ctx: &Context<'a>) -> Row<'a, Message> {
        Row::with_children(self.widgets.iter().map(|w| w.view(ctx)))
            .spacing(ctx.config.layout.horizontal_widget_spacing as f32)
            .align_y(Alignment::Center)
    }
}

/// One bar widget. The spike's four; grows as data sources and QML widgets are ported.
#[derive(Debug, Clone, Copy)]
pub enum Widget {
    /// Dev marker naming the output + edge, so stacked screenshots are unambiguous next
    /// to the live odin bars. Not an odin widget — removed once we cut over.
    Marker,
    Workspaces,
    Battery,
    Clock,
}

impl Widget {
    fn view<'a>(self, ctx: &Context<'a>) -> Element<'a, Message> {
        let sans = font_family(&ctx.config.font.family);
        let size = ctx.config.font.size as f32;
        let label = match self {
            Widget::Marker => format!("odin-rs {} {} ▸", ctx.output, ctx.position.label()),
            Widget::Workspaces => workspaces(ctx),
            Widget::Battery => battery(ctx),
            Widget::Clock => ctx.clock.to_string(),
        };
        let element = text(label).size(size).font(sans);
        match self {
            // Marker keeps the palette's time/orange tint; other widgets inherit the
            // white bar foreground set on the chrome container.
            Widget::Marker => element.color(ctx.config.palette.time_tint.0),
            _ => element,
        }
        .into()
    }
}

fn workspaces(ctx: &Context<'_>) -> String {
    niri::workspaces_for_output(ctx.niri, ctx.output)
        .iter()
        .map(|p| {
            let label = p.name.clone().unwrap_or_else(|| (p.idx + 1).to_string());
            if p.is_active {
                format!("[{label}]")
            } else {
                format!(" {label} ")
            }
        })
        .collect()
}

fn battery(ctx: &Context<'_>) -> String {
    match ctx.battery {
        Some(d) if d.available => match d.percent() {
            Some(p) => format!("{} {p}%", glyph(&d.icon)),
            None => glyph(&d.icon),
        },
        _ => "—".to_string(),
    }
}

/// The zones for a bar at `position`, in odin's `shell.qml` order. Identical top/bottom
/// for the spike; `position` stays in the signature because odin's top and bottom bars
/// carry different widgets, which this will encode as they are ported.
pub fn layout(_position: Position) -> Vec<Zone> {
    vec![
        Zone {
            align: Align::Start,
            widgets: vec![Widget::Marker, Widget::Workspaces],
        },
        Zone {
            align: Align::Center,
            widgets: vec![Widget::Clock],
        },
        Zone {
            align: Align::End,
            widgets: vec![Widget::Battery],
        },
    ]
}

/// Map an odin font-family string to an iced `Font`. odin uses fontconfig aliases
/// (`sans-serif`/`monospace`) that iced can't resolve by name, so map those to iced's
/// built-in defaults; any other value is taken as a real font name.
fn font_family(family: &str) -> Font {
    match family {
        "sans-serif" | "" => Font::DEFAULT,
        "monospace" => Font::MONOSPACE,
        name => Font::with_name(Box::leak(name.to_string().into_boxed_str())),
    }
}

/// Map an Odin icon name to a stand-in glyph. Odin renders Font Awesome via an icon
/// font; the spike just needs something visible, so a couple of ASCII/emoji stand-ins.
/// Real icon-font glyphs come with the Phase 3 widget port.
fn glyph(icon: &str) -> String {
    match icon {
        "plug-circle-bolt" => "⚡".to_string(),
        "car-battery" => "🔋".to_string(),
        other if other.is_empty() => "·".to_string(),
        other => other.to_string(),
    }
}
