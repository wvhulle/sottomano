//! The bar's widget tree, ported from odin's `WidgetGroup`/`MenuGroup`/`shell.qml`.
//!
//! A bar is a list of [`Zone`]s (start/center/end); each zone holds ordered [`Group`]s;
//! each group is either a static row of widgets (odin's `WidgetGroup`), an expandable
//! menu (`MenuGroup` — a handle that toggles content in-place with a [`Hairline`]
//! affordance), or a niri-native group (workspaces pager, window list). `layout(position)`
//! encodes the exact top/bottom composition and order from `shell.qml`.
//!
//! Source-backed leaf widgets render through the `BarWidget` trait: a Font Awesome icon
//! plus a thin vertical [`gauge`] fill (odin's `PercentGauge`), tinted by `level`. An
//! unavailable source renders nothing, so a group with no live widgets leaves no empty
//! pill — matching odin's `visible: row.implicitWidth > 0`.

use iced::alignment::Vertical;
use iced::widget::text::Shaping;
use iced::widget::{Row, container, image, mouse_area, row, stack, text};
use iced::{Alignment, Border, Color, Element, Font, Length};
use iced_font_awesome::fa_icon_solid;
use niri_ipc::state::EventStreamState;

use crate::app::{Message, Position};
use crate::config::Config;
use crate::desktop::DesktopEntries;
use crate::niri;
use crate::source::{BarWidget, Level};

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
    fn view<'a>(&self, ctx: &Context<'a>) -> Option<Element<'a, Message>> {
        match self {
            Group::Static(widgets) => static_group(ctx, widgets),
            Group::Menu { id, handle, content } => menu_group(ctx, *id, *handle, content),
            Group::Workspaces => Some(workspaces(ctx)),
            Group::WindowList => window_list(ctx),
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
        mouse_area(edge_overlay(ctx, inner.into(), indicator))
            .on_press(Message::ToggleMenu(id))
            .on_enter(Message::HoverMenu(Some(id)))
            .on_exit(Message::HoverMenu(None))
            .into(),
    )
}

/// Gap (px) between a widget and its inner-edge hairline indicator.
const HAIRLINE_GAP: f32 = 3.0;

/// Place a state `indicator` (a hairline) against the inner screen edge of `content` —
/// the shared basis for menu affordances and pager focus/urgent bars. A real in-flow
/// `Column` sibling (not a `Stack` overlay): the indicator's height is unconditionally
/// part of the layout, so it never depends on resolving `Length::Fill` against a
/// `Shrink`-sized ancestor (a real bug: the overlay approach silently failed to paint
/// when this widget's parent row didn't happen to stretch to the bar's full height).
/// The indicator sits below `content` on a top bar, above it on a bottom bar, so it
/// always points into the screen; a spacer of `HAIRLINE_GAP` separates the two.
fn edge_overlay<'a>(
    ctx: &Context<'a>,
    content: Element<'a, Message>,
    indicator: Element<'a, Message>,
) -> Element<'a, Message> {
    let gap = container(iced::widget::Space::new()).height(HAIRLINE_GAP);
    match ctx.position {
        Position::Top => iced::widget::column![content, gap, indicator],
        Position::Bottom => iced::widget::column![indicator, gap, content],
    }
    .align_x(Alignment::Center)
    .into()
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
    fn view<'a>(self, ctx: &Context<'a>) -> Option<Element<'a, Message>> {
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

/// A source-backed leaf: Font Awesome icon + thin vertical gauge (or a text label for a
/// label-only source), tinted by severity. `None` when the source is unavailable.
fn leaf<'a>(ctx: &Context<'a>, widget: Option<&dyn BarWidget>) -> Option<Element<'a, Message>> {
    let widget = widget?;
    let size = ctx.config.font.size as f32;
    let color = level_color(ctx.config, widget.level());
    let icon = fa_icon_solid(widget.icon()).size(size).color(color);

    match widget.value() {
        Some(value) => Some(
            row![icon, gauge(value.percent(), color, size)]
                .spacing(ctx.config.layout.horizontal_widget_spacing as f32)
                .align_y(Alignment::Center)
                .into(),
        ),
        None => match widget.label() {
            Some(label) if !label.is_empty() => Some(
                row![icon, text(label).size(size).color(color).shaping(Shaping::Advanced)]
                    .spacing(ctx.config.layout.horizontal_widget_spacing as f32)
                    .align_y(Alignment::Center)
                    .into(),
            ),
            _ => Some(icon.into()),
        },
    }
}

/// odin's `PercentGauge`: a thin vertical fill bar for a 0..100 value. A faint full-height
/// track keeps the frame visible; a solid bar fills from the bottom in proportion.
fn gauge<'a>(percent: i64, color: Color, font_size: f32) -> Element<'a, Message> {
    let frac = (percent as f32 / 100.0).clamp(0.0, 1.0);
    let fill_height = (font_size * frac).max(1.0);
    let bar = |height: f32, alpha: f32| {
        container(iced::widget::Space::new())
            .width(3.0)
            .height(height)
            .style(move |_| container::Style {
                background: Some(Color { a: alpha, ..color }.into()),
                border: Border::default().rounded(1.0),
                ..Default::default()
            })
    };
    // Fill sits at the bottom of the track's band; the outer box adds odin's +3 padding.
    let track = bar(font_size, 0.27);
    let fill = container(bar(fill_height, 1.0))
        .height(font_size)
        .align_y(Vertical::Bottom);
    container(stack![track, fill])
        .height(font_size + 3.0)
        .align_y(Vertical::Center)
        .into()
}

/// odin's `Hairline`: a thin rounded bar marking an expandable group. A fixed-height,
/// content-width sibling (see `menu_group`), inset a little horizontally, at the caller's
/// opacity. Its vertical placement (which screen edge) is handled by the column order in
/// `menu_group`, not here.
fn hairline<'a>(ctx: &Context<'a>, color: Color, opacity: f32) -> Element<'a, Message> {
    let thickness = ctx.config.layout.focus_underline_thickness;
    container(iced::widget::Space::new().width(Length::Fill))
        .width(Length::Fill)
        .height(thickness)
        .style(move |_| container::Style {
            background: Some(Color { a: opacity, ..color }.into()),
            border: Border::default().rounded(thickness),
            ..Default::default()
        })
        .into()
}

/// The workspace pager (odin's `Workspaces`): this output's workspaces in index order, as
/// clickable slots. Focus and urgency are shown by a hairline along the inner edge of the
/// active/urgent slot (powerTint / criticalTint), not by tinting the label — so labels
/// stay neutral, matching odin. Clicking a workspace focuses it.
fn workspaces<'a>(ctx: &Context<'a>) -> Element<'a, Message> {
    let slots = niri::workspaces_for_output(ctx.niri, ctx.output).into_iter().map(|p| {
        let label = workspace_label(ctx, &p);
        // Always the same widget tree shape (a Stack with a hairline layer), varying only
        // the indicator's color/opacity — never conditionally omitted. A structural swap
        // between "plain label" and "Stack(label, hairline)" at the same list position,
        // frame to frame, defeats iced's widget diffing: the indicator can end up stuck on
        // whichever slot first got the Stack shape instead of tracking the live state.
        let (color, opacity) = if p.is_urgent {
            (ctx.config.palette.critical_tint.0, 1.0)
        } else if p.is_active {
            (ctx.config.palette.power_tint.0, 1.0)
        } else {
            (ctx.config.palette.power_tint.0, 0.0)
        };
        let slot = edge_overlay(ctx, label, hairline(ctx, color, opacity));
        mouse_area(slot)
            .on_press(Message::FocusWorkspace(p.id))
            .into()
    });
    Row::with_children(slots)
        .spacing(ctx.config.layout.horizontal_widget_spacing as f32 * 1.5)
        .align_y(Alignment::Center)
        .into()
}

/// A workspace's neutral label (odin's slot content): a named workspace in the
/// keyboard-reachable range (index ≤ 10, the 1–9/0 keys) gets a small raised index prefix
/// before its name; otherwise just the name, or the bare index when unnamed.
fn workspace_label<'a>(ctx: &Context<'a>, p: &niri::WsPill) -> Element<'a, Message> {
    let size = ctx.config.font.size as f32;
    let sans = font_family(&ctx.config.font.family);
    // Workspace 10 is bound to "0"; above 10 the number is useless as a hint.
    let key = if p.idx == 10 { "0".to_string() } else { p.idx.to_string() };
    let named = p.name.as_deref().is_some_and(|n| !n.is_empty());

    match &p.name {
        Some(name) if named && p.idx <= 10 => {
            // Smaller, dimmed, top-aligned index reads as a superscript before the name.
            let dim = Color { a: 0.5, ..Color::WHITE };
            row![
                text(key).size(size * 0.7).font(sans).color(dim).shaping(Shaping::Advanced),
                text(name.clone()).size(size).font(sans).shaping(Shaping::Advanced),
            ]
            .align_y(Alignment::Start)
            .into()
        }
        Some(name) if named => {
            text(name.clone()).size(size).font(sans).shaping(Shaping::Advanced).into()
        }
        _ => text(key).size(size).font(sans).shaping(Shaping::Advanced).into(),
    }
}

/// The window list (odin's `Windowlist`): icon + title of the windows on the output's
/// active workspace, in on-screen order. The focused entry carries a hairline indicator
/// along the inner edge (powerTint), an urgent one criticalTint — the same sliding-focus
/// affordance the pager uses, not a text-color highlight (odin's underline, not tint,
/// marks focus; the label itself only dims for inactive entries). `None` when the
/// workspace has no windows. Clicking an entry focuses that window.
fn window_list<'a>(ctx: &Context<'a>) -> Option<Element<'a, Message>> {
    let workspace = niri::active_workspace_id(ctx.niri, ctx.output)?;
    let windows = niri::windows_on_workspace(ctx.niri, workspace);
    if windows.is_empty() {
        return None;
    }
    let size = ctx.config.font.size as f32;
    let sans = font_family(&ctx.config.font.family);
    let entries = windows.into_iter().map(|w| {
        let label_color = if w.is_focused || w.is_urgent {
            Color::WHITE
        } else {
            ctx.config.palette.muted.0
        };
        let mut parts: Vec<Element<'a, Message>> = Vec::new();
        if let Some(icon) = window_icon(ctx, &w) {
            parts.push(icon);
        }
        parts.push(
            text(window_label(&w)).size(size).font(sans).color(label_color).shaping(Shaping::Advanced).into(),
        );
        let content: Element<'a, Message> = Row::with_children(parts)
            .spacing(ctx.config.layout.horizontal_widget_spacing as f32 * 0.6)
            .align_y(Alignment::Center)
            .into();

        // Always the same widget tree shape (a Stack with a hairline layer) — see the
        // comment on the same pattern in `workspaces` for why the indicator must never be
        // structurally omitted.
        let (color, opacity) = if w.is_urgent {
            (ctx.config.palette.critical_tint.0, 1.0)
        } else if w.is_focused {
            (ctx.config.palette.power_tint.0, 1.0)
        } else {
            (ctx.config.palette.power_tint.0, 0.0)
        };
        let entry = edge_overlay(ctx, content, hairline(ctx, color, opacity));

        // Clicking a window focuses it (odin's Windowlist `onActivated`).
        mouse_area(entry).on_press(Message::FocusWindow(w.id)).into()
    });
    Some(
        Row::with_children(entries)
            .spacing(ctx.config.layout.horizontal_widget_spacing as f32 * 2.0)
            .align_y(Alignment::Center)
            .into(),
    )
}

/// A window's icon, resolved from its `app_id` via the installed `.desktop` entries and
/// the icon theme (odin's `WindowIcon._resolveAppId`). `None` before the desktop-entry
/// scan completes, or when nothing resolves (odin's generic fallback icon still applies
/// inside `icon_for`, so this is only `None` on that final failure — kept as a graceful
/// gap rather than a broken-image placeholder).
fn window_icon<'a>(ctx: &Context<'a>, w: &niri::WinEntry) -> Option<Element<'a, Message>> {
    let path = ctx.desktop_entries?.icon_for(&w.app_id)?;
    let side = ctx.config.image_icon_size() as f32;
    Some(image(image::Handle::from_path(path)).width(side).height(side).into())
}

/// A short label for a window: its title, else its app id.
fn window_label(w: &niri::WinEntry) -> String {
    let raw = if w.title.is_empty() { &w.app_id } else { &w.title };
    let trimmed: String = raw.chars().take(24).collect();
    if raw.chars().count() > 24 {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

/// The top and bottom bars' zones, in `shell.qml` order. Groups whose data sources aren't
/// ported yet (Weather, network, media, audio, tray, notifications, quicklaunch, …) are
/// omitted for now rather than shown empty.
pub fn layout(position: Position) -> Vec<Zone> {
    match position {
        Position::Top => vec![
            Zone {
                align: Align::Start,
                groups: vec![
                    Group::Static(vec![Widget::Battery]),
                    // odin's System group: cpu handle expands to temperature/memory/disk.
                    Group::Menu {
                        id: MenuId::System,
                        handle: Widget::Cpu,
                        content: vec![Widget::Temperature, Widget::Memory, Widget::Disk],
                    },
                ],
            },
            Zone {
                align: Align::Center,
                groups: vec![Group::Static(vec![Widget::Time])],
            },
        ],
        Position::Bottom => vec![
            Zone {
                align: Align::Start,
                groups: vec![Group::Workspaces],
            },
            Zone {
                align: Align::Center,
                groups: vec![Group::WindowList],
            },
        ],
    }
}

/// Map an odin font-family string to an iced `Font`. odin's `sans-serif`/`monospace` are
/// fontconfig generic aliases; Qt resolves them to concrete system fonts (here IBM Plex
/// Sans / Lilex Nerd Font Mono) and renders those. iced loads system fonts too, so we
/// resolve the aliases to the same concrete families to match odin's rendering rather
/// than falling back to iced's built-in Fira Sans. TODO: resolve the alias via
/// fontconfig at startup instead of hardcoding the current machine's default.
fn font_family(family: &str) -> Font {
    match family {
        "sans-serif" | "" => Font::with_name("IBM Plex Sans"),
        "monospace" => Font::with_name("Lilex Nerd Font Mono"),
        name => Font::with_name(Box::leak(name.to_string().into_boxed_str())),
    }
}

/// Severity → foreground, mirroring odin's per-level tints: ok stays the white bar
/// foreground; warning/critical take the palette's brightness/critical tints.
fn level_color(config: &Config, level: Level) -> Color {
    match level {
        Level::Ok => Color::WHITE,
        Level::Warning => config.palette.brightness_tint.0,
        Level::Critical => config.palette.critical_tint.0,
    }
}
