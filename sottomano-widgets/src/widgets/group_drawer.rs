use std::{
    collections::{BTreeMap, BTreeSet},
    iter::once,
    mem,
    time::Instant,
    vec,
};

use chrono::Locale;
use iced::{
    Alignment, Element, Length, Task,
    alignment::Horizontal,
    widget::{Row, Space, container},
};
use sottomano_config::{Config, screen_edges::Position};
use tokio::time::sleep_until;

use crate::{
    clock::Clock,
    hover_tooltips::{GroupHandle, tooltip_surfaces::HIDE_GRACE},
    widgets::{
        activity_line::activity_line,
        aligned_quantity::AlignedQuantity,
        anchored_popup::{SheetPresence, reanchored_sheet},
        anchored_sheet::{PointerCrossing, anchored_sheet},
        bar_widget::Glyph,
        content_sized_table::content_sized_grid,
        pager_slot::PagerSlot,
        palette_colors::PaletteColors,
        shell_layer_appearance::LayerAppearance,
        text::bar_text,
        widget_group::{DrawerCells, EdgeIndicator, GroupContent, GroupEvent, GroupToastRequests},
    },
};

const RULE_OPACITY: f32 = 0.2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawerMessage {
    Reveal(GroupHandle),
    Close,
    PointerEntered { drawer: GroupHandle, output: String },
    PointerLeft,
    LeaveGraceElapsed(Instant),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DrawerShown {
    OnOutput { drawer: GroupHandle, output: String },
    Everywhere(GroupHandle),
}

impl DrawerShown {
    fn shows(&self, drawer: GroupHandle) -> bool {
        match self {
            DrawerShown::OnOutput { drawer: shown, .. } | DrawerShown::Everywhere(shown) => {
                *shown == drawer
            }
        }
    }

    fn shows_on(&self, drawer: GroupHandle, output: &str) -> bool {
        match self {
            DrawerShown::OnOutput {
                drawer: shown,
                output: shown_output,
            } => *shown == drawer && shown_output == output,
            DrawerShown::Everywhere(shown) => *shown == drawer,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum DrawerOpening {
    #[default]
    Closed,
    Hovered(DrawerShown),
    Leaving {
        shown: DrawerShown,
        hide_due: Instant,
    },
    Hiding(DrawerShown),
}

impl DrawerOpening {
    fn hidden(self) -> Self {
        match self {
            DrawerOpening::Hovered(shown)
            | DrawerOpening::Leaving { shown, .. }
            | DrawerOpening::Hiding(shown) => DrawerOpening::Hiding(shown),
            DrawerOpening::Closed => DrawerOpening::Closed,
        }
    }
}

#[derive(Default)]
pub struct GroupDrawers {
    opening: DrawerOpening,
}

impl GroupDrawers {
    pub fn update(&mut self, msg: DrawerMessage, now: Instant) -> Task<DrawerMessage> {
        match msg {
            DrawerMessage::PointerEntered { drawer, output } => {
                self.opening = DrawerOpening::Hovered(DrawerShown::OnOutput { drawer, output });
            }
            DrawerMessage::PointerLeft => {
                if let DrawerOpening::Hovered(shown) = &self.opening {
                    let hide_due = now + HIDE_GRACE;
                    self.opening = DrawerOpening::Leaving {
                        shown: shown.clone(),
                        hide_due,
                    };
                    return Task::future(async move {
                        sleep_until(hide_due.into()).await;
                        DrawerMessage::LeaveGraceElapsed(hide_due)
                    });
                }
            }
            DrawerMessage::LeaveGraceElapsed(elapsed) => {
                if matches!(self.opening, DrawerOpening::Leaving { hide_due, .. } if hide_due == elapsed)
                {
                    self.collapse();
                }
            }
            DrawerMessage::Reveal(drawer) => {
                if !matches!(&self.opening, DrawerOpening::Hovered(shown) if shown.shows(drawer)) {
                    self.opening = DrawerOpening::Hovered(DrawerShown::Everywhere(drawer));
                }
            }
            DrawerMessage::Close => self.collapse(),
        }
        Task::none()
    }

    pub fn collapse(&mut self) {
        self.opening = mem::take(&mut self.opening).hidden();
    }

    #[must_use]
    pub fn is_open_on(&self, drawer: GroupHandle, output: &str) -> bool {
        self.sheet_presence(drawer, output) == Some(SheetPresence::Arriving)
    }

    fn sheet_presence(&self, drawer: GroupHandle, output: &str) -> Option<SheetPresence> {
        match &self.opening {
            DrawerOpening::Closed => None,
            DrawerOpening::Hovered(shown) | DrawerOpening::Leaving { shown, .. } => shown
                .shows_on(drawer, output)
                .then_some(SheetPresence::Arriving),
            DrawerOpening::Hiding(shown) => shown
                .shows_on(drawer, output)
                .then_some(SheetPresence::Departing),
        }
    }
}

pub struct DrawerPlacement<'p> {
    pub drawers: &'p GroupDrawers,
    pub output: &'p str,
    pub bar_position: Position,
    pub float_offset_px: f32,
    pub sheet_appearance: LayerAppearance,
}

pub struct GroupDrawer<'a, M> {
    drawer: GroupHandle,
    glyph: Glyph,
    lift: fn(DrawerMessage) -> M,
    members: Vec<GroupContent<'a, M>>,
}

impl<'a, M: Clone + 'a> GroupDrawer<'a, M> {
    pub fn new(
        drawer: GroupHandle,
        glyph: impl Into<Glyph>,
        lift: fn(DrawerMessage) -> M,
        members: impl IntoIterator<Item = GroupContent<'a, M>>,
    ) -> Self {
        Self {
            drawer,
            glyph: glyph.into(),
            lift,
            members: members.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests<M> {
        let reveal_message = (self.lift)(DrawerMessage::Reveal(self.drawer));
        self.members
            .iter()
            .filter(|member| member.has_expandable_handle())
            .map(|member| member.toast_requests_revealed_by(&reveal_message, locale, clock))
            .collect()
    }

    pub fn view<Outer: From<GroupEvent<M>> + 'a>(
        self,
        config: &'a Config,
        placement: &DrawerPlacement<'_>,
        clock: Clock,
    ) -> Element<'a, Outer> {
        let presence = placement
            .drawers
            .sheet_presence(self.drawer, placement.output);
        let opened = presence == Some(SheetPresence::Arriving);
        let crossing = PointerCrossing {
            entered: GroupEvent::Group((self.lift)(DrawerMessage::PointerEntered {
                drawer: self.drawer,
                output: placement.output.to_owned(),
            })),
            left: GroupEvent::Group((self.lift)(DrawerMessage::PointerLeft)),
        };
        let handle = self.handle_view(config, placement.bar_position, opened, clock);
        let sheet = presence.and_then(|presence| {
            sheet_view(self.members, config, clock).map(|content| {
                reanchored_sheet(
                    content,
                    presence,
                    placement.bar_position,
                    config,
                    placement.sheet_appearance,
                )
            })
        });
        anchored_sheet(handle, sheet, placement.float_offset_px, crossing).map(Outer::from)
    }

    fn handle_view(
        &self,
        config: &'a Config,
        bar_position: Position,
        opened: bool,
        clock: Clock,
    ) -> Element<'a, GroupEvent<M>> {
        let highest_alert_severity = self
            .members
            .iter()
            .map(|member| member.highest_alert_severity(clock))
            .max()
            .unwrap_or_default();
        let foreground = PaletteColors::from(&config.theme.palette).bar_foreground;
        let handle = self.glyph.view(config.theme.font.icon_size_px, foreground);
        let indicator = EdgeIndicator {
            highest_alert_severity,
            cursor_hovering: false,
            expanded: opened,
        }
        .view(config, if opened { 1.0 } else { 0.0 });
        PagerSlot::new(handle, indicator).view(config, bar_position)
    }
}

struct MemberRemainder<'a, M> {
    aligned: BTreeMap<AlignedQuantity, Element<'a, GroupEvent<M>>>,
    unaligned: vec::IntoIter<Element<'a, GroupEvent<M>>>,
}

fn sheet_view<'a, M: Clone + 'a>(
    members: Vec<GroupContent<'a, M>>,
    config: &'a Config,
    clock: Clock,
) -> Option<Element<'a, GroupEvent<M>>> {
    let member_cells: Vec<DrawerCells<'a, M>> = members
        .into_iter()
        .filter_map(|member| member.drawer_cells(config, clock))
        .collect();
    if member_cells.is_empty() {
        return None;
    }
    let quantity_rows = aligned_quantity_rows(
        member_cells
            .iter()
            .map(|cells| cells.aligned.keys().copied()),
    );
    let unaligned_depth = member_cells
        .iter()
        .map(|cells| cells.unaligned.len())
        .max()
        .unwrap_or_default();
    let member_count = member_cells.len();
    let (faces, mut remainders): (Vec<_>, Vec<_>) = member_cells
        .into_iter()
        .map(|cells| {
            (
                container(cells.face).center_x(Length::Fill).into(),
                MemberRemainder {
                    aligned: cells.aligned,
                    unaligned: cells.unaligned.into_iter(),
                },
            )
        })
        .unzip();
    let rule_row = || {
        (0..=member_count)
            .map(|_| drawer_rule(config))
            .collect::<Vec<_>>()
    };
    let header_row: Vec<Element<'a, GroupEvent<M>>> =
        once(Space::new().into()).chain(faces).collect();
    let aligned_rows: Vec<Vec<Element<'a, GroupEvent<M>>>> = quantity_rows
        .iter()
        .map(|quantity| {
            once(quantity_label(config, *quantity))
                .chain(remainders.iter_mut().map(|remainder| {
                    remainder
                        .aligned
                        .remove(quantity)
                        .unwrap_or_else(|| Space::new().into())
                }))
                .collect()
        })
        .collect();
    let unaligned_rows: Vec<Vec<Element<'a, GroupEvent<M>>>> = (0..unaligned_depth)
        .map(|_| {
            once(Space::new().into())
                .chain(remainders.iter_mut().map(|remainder| {
                    remainder
                        .unaligned
                        .next()
                        .unwrap_or_else(|| Space::new().into())
                }))
                .collect()
        })
        .collect();
    let section_rule = (!aligned_rows.is_empty() && !unaligned_rows.is_empty()).then(rule_row);
    let at_least_bar_height =
        |row: Vec<Element<'a, GroupEvent<M>>>| -> Vec<Element<'a, GroupEvent<M>>> {
            row.into_iter()
                .map(|cell| at_least_height(config.layout.panel.height, cell))
                .collect()
        };
    let grid = content_sized_grid(
        vec![Horizontal::Left; member_count + 1],
        once(at_least_bar_height(header_row))
            .chain(once(rule_row()))
            .chain(aligned_rows.into_iter().map(at_least_bar_height))
            .chain(section_rule)
            .chain(unaligned_rows.into_iter().map(at_least_bar_height)),
    )
    .padding_x(config.layout.panel.widget_spacing)
    .padding_y(config.layout.overlay.row_spacing / 2.0);
    Some(
        container(grid)
            .padding(config.layout.panel.group_padding)
            .into(),
    )
}

fn aligned_quantity_rows(
    member_quantities: impl IntoIterator<Item = impl IntoIterator<Item = AlignedQuantity>>,
) -> Vec<AlignedQuantity> {
    member_quantities
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn at_least_height<'a, M: 'a>(minimum_height: f32, cell: Element<'a, M>) -> Element<'a, M> {
    Row::with_children([Space::new().height(minimum_height).into(), cell])
        .align_y(Alignment::Center)
        .into()
}

fn quantity_label<'a, M: 'a>(config: &Config, quantity: AlignedQuantity) -> Element<'a, M> {
    bar_text(&config.theme.font, quantity.to_string())
        .color(PaletteColors::from(&config.theme.palette).secondary_foreground)
        .into()
}

fn drawer_rule<'a, M: 'a>(config: &Config) -> Element<'a, M> {
    activity_line(
        PaletteColors::from(&config.theme.palette).bar_foreground,
        RULE_OPACITY,
        config.panel_bar.indicator_thickness,
    )
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{DrawerMessage, GroupDrawers, aligned_quantity_rows};
    use crate::{
        clock::Clock,
        hover_tooltips::{GroupHandle, tooltip_surfaces::HIDE_GRACE},
        widgets::aligned_quantity::AlignedQuantity,
    };

    const SETTINGS: GroupHandle = GroupHandle::SettingsDrawer;
    const OTHER: GroupHandle = GroupHandle::Tray;

    fn send(drawers: &mut GroupDrawers, msg: DrawerMessage, now: Instant) {
        drop(drawers.update(msg, now));
    }

    fn hover(drawer: GroupHandle, output: &str) -> DrawerMessage {
        DrawerMessage::PointerEntered {
            drawer,
            output: output.to_owned(),
        }
    }

    #[test]
    fn hovering_opens_the_drawer_only_on_that_output() {
        let now = Clock::real().instant;
        let mut drawers = GroupDrawers::default();
        send(&mut drawers, hover(SETTINGS, "DP-1"), now);
        assert!(drawers.is_open_on(SETTINGS, "DP-1"));
        assert!(!drawers.is_open_on(SETTINGS, "HDMI-A-1"));
        assert!(!drawers.is_open_on(OTHER, "DP-1"));
    }

    #[test]
    fn hovering_another_drawer_replaces_the_first() {
        let now = Clock::real().instant;
        let mut drawers = GroupDrawers::default();
        send(&mut drawers, hover(SETTINGS, "DP-1"), now);
        send(&mut drawers, hover(OTHER, "DP-1"), now);
        assert!(!drawers.is_open_on(SETTINGS, "DP-1"));
        assert!(drawers.is_open_on(OTHER, "DP-1"));
    }

    #[test]
    fn leaving_closes_the_drawer_once_the_grace_elapses() {
        let now = Clock::real().instant;
        let mut drawers = GroupDrawers::default();
        send(&mut drawers, hover(SETTINGS, "DP-1"), now);
        send(&mut drawers, DrawerMessage::PointerLeft, now);
        assert!(drawers.is_open_on(SETTINGS, "DP-1"));
        send(
            &mut drawers,
            DrawerMessage::LeaveGraceElapsed(now + HIDE_GRACE),
            now + HIDE_GRACE,
        );
        assert!(!drawers.is_open_on(SETTINGS, "DP-1"));
    }

    #[test]
    fn returning_within_the_grace_keeps_the_drawer_open() {
        let now = Clock::real().instant;
        let mut drawers = GroupDrawers::default();
        send(&mut drawers, hover(SETTINGS, "DP-1"), now);
        send(&mut drawers, DrawerMessage::PointerLeft, now);
        send(&mut drawers, hover(SETTINGS, "DP-1"), now);
        send(
            &mut drawers,
            DrawerMessage::LeaveGraceElapsed(now + HIDE_GRACE),
            now + HIDE_GRACE,
        );
        assert!(drawers.is_open_on(SETTINGS, "DP-1"));
    }

    #[test]
    fn an_earlier_leave_does_not_close_after_a_later_one() {
        let now = Clock::real().instant;
        let later = now + Duration::from_millis(100);
        let mut drawers = GroupDrawers::default();
        send(&mut drawers, hover(SETTINGS, "DP-1"), now);
        send(&mut drawers, DrawerMessage::PointerLeft, now);
        send(&mut drawers, hover(SETTINGS, "DP-1"), later);
        send(&mut drawers, DrawerMessage::PointerLeft, later);
        send(
            &mut drawers,
            DrawerMessage::LeaveGraceElapsed(now + HIDE_GRACE),
            now + HIDE_GRACE,
        );
        assert!(drawers.is_open_on(SETTINGS, "DP-1"));
    }

    #[test]
    fn revealing_a_closed_drawer_opens_it_everywhere() {
        let now = Clock::real().instant;
        let mut drawers = GroupDrawers::default();
        send(&mut drawers, DrawerMessage::Reveal(SETTINGS), now);
        assert!(drawers.is_open_on(SETTINGS, "DP-1"));
        assert!(drawers.is_open_on(SETTINGS, "HDMI-A-1"));
    }

    #[test]
    fn collapsing_closes_every_drawer() {
        let now = Clock::real().instant;
        let mut drawers = GroupDrawers::default();
        send(&mut drawers, hover(SETTINGS, "DP-1"), now);
        drawers.collapse();
        assert!(!drawers.is_open_on(SETTINGS, "DP-1"));
    }

    #[test]
    fn rows_are_the_union_of_member_quantities_in_declared_order() {
        let panels = [AlignedQuantity::Blur, AlignedQuantity::Opacity];
        let niri = [AlignedQuantity::CornerRadius];
        assert_eq!(
            aligned_quantity_rows([panels.to_vec(), niri.to_vec()]),
            [
                AlignedQuantity::Opacity,
                AlignedQuantity::CornerRadius,
                AlignedQuantity::Blur,
            ]
        );
    }

    #[test]
    fn a_quantity_shared_by_several_members_is_one_row() {
        let shared = [AlignedQuantity::CornerRadius];
        assert_eq!(
            aligned_quantity_rows([shared.to_vec(), shared.to_vec(), shared.to_vec()]),
            [AlignedQuantity::CornerRadius]
        );
    }

    #[test]
    fn members_without_aligned_items_add_no_rows() {
        assert!(aligned_quantity_rows(Vec::<Vec<AlignedQuantity>>::new()).is_empty());
    }
}
