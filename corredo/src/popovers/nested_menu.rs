use std::{iter::once, time::Duration};

use iced::{
    Border, Element, Length, Theme,
    border::Radius,
    mouse::ScrollDelta,
    widget::{Space, container, mouse_area, rule, stack},
};

use crate::{
    drag_reorder::ListEndCorners,
    orientation::Axis,
    popovers::{
        CascadeOrientation, NestedMenuLayout, NestedMenuMetrics, NestedMenuRowContent,
        NestedMenuRowEmphasis, NestedMenuRowLook, NestedMenuRowPress, NotchRowAnchor,
        ResolvedPanelTop, RowHoverReaction, SubmenuConnector, anchor_marker, nested_menu_row,
        notched_panel,
    },
    selection_scroll::{ScrollEdgeFade, ScrollPacing, selection_following_scroll},
    theming::{
        FrameStyle, background_strong_color, background_weak_color, style_metrics::OUTLINE_WIDTH_PX,
    },
    transitions::ExpansionState,
    width_layout::equal_width_column,
};

const DEFAULT_MORPH: Duration = Duration::from_millis(160);
const DEFAULT_WHEEL_LINE_PX: f32 = 40.0;

pub enum RowGesture<Payload> {
    Inert,
    Publishes(Payload),
}

type ScrollAdjustment<'a, Message> = Box<dyn Fn(ScrollDelta) -> Message + 'a>;

pub struct NestedMenuRow<'a, Message> {
    pub content: NestedMenuRowContent<'a, Message>,
    pub emphasis: NestedMenuRowEmphasis,
    pub press: NestedMenuRowPress<Message>,
    pub hover_reaction: RowHoverReaction<Message>,
    pub entered: RowGesture<Message>,
    pub right_pressed: RowGesture<Message>,
    pub scrolled: RowGesture<ScrollAdjustment<'a, Message>>,
}

pub enum BesideRow<'a, Message> {
    Nothing,
    Card(Element<'a, Message>),
}

pub struct NestedMenuItem<'a, RowKey, Message> {
    pub key: RowKey,
    pub row: NestedMenuRow<'a, Message>,
    pub beside: BesideRow<'a, Message>,
}

pub struct NestedMenuGroup<'a, RowKey, Message> {
    pub header: NestedMenuRow<'a, Message>,
    pub expansion_state: ExpansionState,
    pub items: Vec<NestedMenuItem<'a, RowKey, Message>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SatelliteRole {
    ItemList,
    Beside,
}

pub struct NestedMenuSatellite<'a, Message> {
    pub role: SatelliteRole,
    pub element: Element<'a, Message>,
}

pub struct NestedMenuSurfaces<'a, Message> {
    pub primary: Element<'a, Message>,
    pub satellites: Vec<NestedMenuSatellite<'a, Message>>,
}

type ScrollReport<'a, Message> = Box<dyn Fn(f32) -> Message + 'a>;

struct ControlledScroll<'a, Message> {
    offset_px: f32,
    on_scroll: ScrollReport<'a, Message>,
}

pub struct NestedMenu<'a, RowKey, Message> {
    groups: Vec<NestedMenuGroup<'a, RowKey, Message>>,
    list_scroll: ControlledScroll<'a, Message>,
    item_list_scroll: ControlledScroll<'a, Message>,
    chrome: MenuChrome,
}

#[derive(Debug, Clone, Copy)]
struct MenuChrome {
    layout: NestedMenuLayout,
    orientation: CascadeOrientation,
    metrics: NestedMenuMetrics,
    style: NestedMenuStyle,
    wheel_line_px: f32,
    pacing: ScrollPacing,
}

#[derive(Debug, Clone, Copy)]
pub struct NestedMenuStyle {
    pub list_frame: FrameStyle,
    pub beside_frame: FrameStyle,
    pub morph: Duration,
    pub gap_to_parent_px: f32,
    pub submenu_connector: SubmenuConnector,
    pub fade_extent_px: f32,
    pub item_indent_px: f32,
}

impl NestedMenuStyle {
    #[must_use]
    pub fn for_metrics(metrics: NestedMenuMetrics) -> Self {
        Self {
            list_frame: FrameStyle::default(),
            beside_frame: FrameStyle::default(),
            morph: DEFAULT_MORPH,
            gap_to_parent_px: 0.0,
            submenu_connector: SubmenuConnector::default(),
            fade_extent_px: metrics.row_height_px,
            item_indent_px: metrics.icon_slot_px,
        }
    }

    fn frame_corners(self) -> Radius {
        Radius::from(self.list_frame.corner_radius().top_left)
    }
}

pub fn nested_menu<'a, RowKey, Message: 'a>(
    groups: Vec<NestedMenuGroup<'a, RowKey, Message>>,
    layout: NestedMenuLayout,
    orientation: CascadeOrientation,
    metrics: NestedMenuMetrics,
    on_list_scroll: impl Fn(f32) -> Message + 'a,
    on_item_list_scroll: impl Fn(f32) -> Message + 'a,
) -> NestedMenu<'a, RowKey, Message> {
    NestedMenu {
        groups,
        chrome: MenuChrome {
            layout,
            orientation,
            metrics,
            style: NestedMenuStyle::for_metrics(metrics),
            wheel_line_px: DEFAULT_WHEEL_LINE_PX,
            pacing: ScrollPacing::Instant,
        },
        list_scroll: ControlledScroll {
            offset_px: 0.0,
            on_scroll: Box::new(on_list_scroll),
        },
        item_list_scroll: ControlledScroll {
            offset_px: 0.0,
            on_scroll: Box::new(on_item_list_scroll),
        },
    }
}

impl<RowKey, Message> NestedMenu<'_, RowKey, Message> {
    fn with_chrome(self, chrome: MenuChrome) -> Self {
        Self { chrome, ..self }
    }

    #[must_use]
    pub fn style(self, style: NestedMenuStyle) -> Self {
        let chrome = MenuChrome {
            style,
            ..self.chrome
        };
        self.with_chrome(chrome)
    }

    #[must_use]
    pub fn wheel_scrolling(self, wheel_line_px: f32, pacing: ScrollPacing) -> Self {
        let chrome = MenuChrome {
            wheel_line_px,
            pacing,
            ..self.chrome
        };
        self.with_chrome(chrome)
    }

    #[must_use]
    pub fn scrolled(self, list_offset_px: f32, item_list_offset_px: f32) -> Self {
        Self {
            list_scroll: ControlledScroll {
                offset_px: list_offset_px,
                ..self.list_scroll
            },
            item_list_scroll: ControlledScroll {
                offset_px: item_list_offset_px,
                ..self.item_list_scroll
            },
            ..self
        }
    }
}

impl<'a, RowKey: Clone + Send + Sync + 'static, Message: Clone + 'a>
    NestedMenu<'a, RowKey, Message>
{
    #[must_use]
    pub fn surfaces(self) -> Option<NestedMenuSurfaces<'a, Message>> {
        let NestedMenu {
            groups,
            list_scroll,
            item_list_scroll,
            chrome,
        } = self;
        if groups.is_empty() {
            return None;
        }
        Some(match chrome.layout {
            NestedMenuLayout::Cascading => {
                chrome.cascading_surfaces(groups, list_scroll, item_list_scroll)
            }
            NestedMenuLayout::Accordion => chrome.accordion_surfaces(groups, list_scroll),
        })
    }
}

impl MenuChrome {
    fn scrolled_list<'a, Message: Clone + 'a>(
        self,
        rows: Vec<Element<'a, Message>>,
        scroll: ControlledScroll<'a, Message>,
    ) -> Element<'a, Message> {
        let ControlledScroll {
            offset_px,
            on_scroll,
        } = scroll;
        selection_following_scroll(equal_width_column(rows), Axis::Vertical)
            .wheel_line_travel(self.wheel_line_px)
            .paced(self.pacing)
            .edge_fade(ScrollEdgeFade::within_frame(
                self.style.list_frame,
                self.style.fade_extent_px,
            ))
            .controlled(offset_px, on_scroll)
            .into()
    }

    fn satellite<'a, Message: 'a>(
        self,
        role: SatelliteRole,
        content: Element<'a, Message>,
        row: NotchRowAnchor,
        panel_top: ResolvedPanelTop,
    ) -> NestedMenuSatellite<'a, Message> {
        let frame = match role {
            SatelliteRole::ItemList => self.style.list_frame,
            SatelliteRole::Beside => self.style.beside_frame,
        };
        NestedMenuSatellite {
            role,
            element: notched_panel(content, self.orientation, row)
                .frame(frame)
                .row_height(self.metrics.row_height_px)
                .gap_to_parent(self.style.gap_to_parent_px)
                .submenu_connector(self.style.submenu_connector)
                .morph(self.style.morph)
                .publishes_top(panel_top)
                .into(),
        }
    }

    fn rows_offset(self, rows_above: usize) -> f32 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a menu shows far fewer rows than f32 counts exactly"
        )]
        let rows_above = rows_above as f32;
        self.metrics.row_height_px * rows_above
    }

    fn cascading_surfaces<'a, RowKey: Clone + Send + Sync + 'static, Message: Clone + 'a>(
        self,
        groups: Vec<NestedMenuGroup<'a, RowKey, Message>>,
        list_scroll: ControlledScroll<'a, Message>,
        item_list_scroll: ControlledScroll<'a, Message>,
    ) -> NestedMenuSurfaces<'a, Message> {
        let open_position = groups
            .iter()
            .position(|group| group.expansion_state == ExpansionState::Expanded);
        let header_corners = ListEndCorners::new(groups.len(), self.style.frame_corners());
        let mut open_items = Vec::new();
        let headers: Vec<Element<'a, Message>> = groups
            .into_iter()
            .enumerate()
            .map(|(position, group)| {
                if Some(position) == open_position {
                    open_items = group.items;
                }
                finished_row(
                    group.header,
                    self.metrics,
                    header_corners.radius_at(position),
                )
            })
            .collect();
        let list_offset_px = list_scroll.offset_px;
        let primary = self.scrolled_list(headers, list_scroll);
        let satellites = match open_position {
            Some(open_position) => self.open_group_satellites(
                open_items,
                self.rows_offset(open_position) - list_offset_px,
                item_list_scroll,
            ),
            None => Vec::new(),
        };
        NestedMenuSurfaces {
            primary,
            satellites,
        }
    }

    fn open_group_satellites<'a, RowKey: Clone + Send + Sync + 'static, Message: Clone + 'a>(
        self,
        items: Vec<NestedMenuItem<'a, RowKey, Message>>,
        open_row_top: f32,
        item_list_scroll: ControlledScroll<'a, Message>,
    ) -> Vec<NestedMenuSatellite<'a, Message>> {
        let item_corners = ListEndCorners::new(items.len(), self.style.frame_corners());
        let mut beside_card = None;
        let rows: Vec<Element<'a, Message>> = items
            .into_iter()
            .enumerate()
            .map(|(position, item)| {
                if let BesideRow::Card(card) = item.beside {
                    beside_card = Some((position, card));
                }
                anchor_marker(
                    finished_row(item.row, self.metrics, item_corners.radius_at(position)),
                    item.key,
                )
            })
            .collect();
        let item_list_offset_px = item_list_scroll.offset_px;
        let item_list = self.scrolled_list(rows, item_list_scroll);
        let list_top = ResolvedPanelTop::default();
        let item_satellite = self.satellite(
            SatelliteRole::ItemList,
            item_list,
            NotchRowAnchor::FromPrimaryTop(open_row_top),
            list_top.clone(),
        );
        let card_satellite = beside_card.map(|(position, card)| {
            self.satellite(
                SatelliteRole::Beside,
                card,
                NotchRowAnchor::WithinPanel {
                    panel_top: list_top,
                    row_offset: self.rows_offset(position) - item_list_offset_px,
                },
                ResolvedPanelTop::default(),
            )
        });
        once(item_satellite).chain(card_satellite).collect()
    }

    fn accordion_surfaces<'a, RowKey: Clone + Send + Sync + 'static, Message: Clone + 'a>(
        self,
        groups: Vec<NestedMenuGroup<'a, RowKey, Message>>,
        list_scroll: ControlledScroll<'a, Message>,
    ) -> NestedMenuSurfaces<'a, Message> {
        let sections: Vec<AccordionSectionRows> = groups
            .iter()
            .map(|group| AccordionSectionRows {
                item_count: group.items.len(),
                expansion_state: group.expansion_state,
            })
            .collect();
        let row_corners = ListEndCorners::new(
            sections.iter().map(|section| section.shown_rows()).sum(),
            self.style.frame_corners(),
        );
        let mut beside_card = None;
        let rows: Vec<Element<'a, Message>> = groups
            .into_iter()
            .enumerate()
            .flat_map(|(section, group)| {
                let expansion_state = group.expansion_state;
                let header_corners = row_corners.radius_at(rows_above_section(&sections, section));
                let divider = match section {
                    0 => SectionDivider::Absent,
                    _ => SectionDivider::Above,
                };
                let header = section_row(
                    finished_row(group.header, self.metrics, header_corners),
                    expansion_state,
                    header_corners,
                    divider,
                );
                let shown_items = match expansion_state {
                    ExpansionState::Expanded => group.items,
                    ExpansionState::Collapsed => Vec::new(),
                };
                let item_rows: Vec<Element<'a, Message>> = shown_items
                    .into_iter()
                    .enumerate()
                    .map(|(item, mut menu_item)| {
                        let rows_above = rows_above_item(&sections, section, item);
                        if let BesideRow::Card(card) = menu_item.beside {
                            beside_card = Some((rows_above, card));
                        }
                        menu_item
                            .row
                            .content
                            .leading
                            .insert(0, Space::new().width(self.style.item_indent_px).into());
                        let corners = row_corners.radius_at(rows_above);
                        section_row(
                            anchor_marker(
                                finished_row(menu_item.row, self.metrics, corners),
                                menu_item.key,
                            ),
                            expansion_state,
                            corners,
                            SectionDivider::Absent,
                        )
                    })
                    .collect();
                once(header).chain(item_rows).collect::<Vec<_>>()
            })
            .collect();
        let list_offset_px = list_scroll.offset_px;
        let primary = self.scrolled_list(rows, list_scroll);
        let satellites = beside_card
            .map(|(rows_above, card)| {
                self.satellite(
                    SatelliteRole::Beside,
                    card,
                    NotchRowAnchor::FromPrimaryTop(self.rows_offset(rows_above) - list_offset_px),
                    ResolvedPanelTop::default(),
                )
            })
            .into_iter()
            .collect();
        NestedMenuSurfaces {
            primary,
            satellites,
        }
    }
}

fn finished_row<'a, Message: Clone + 'a>(
    row: NestedMenuRow<'a, Message>,
    metrics: NestedMenuMetrics,
    frame_corners: Radius,
) -> Element<'a, Message> {
    let NestedMenuRow {
        content,
        emphasis,
        press,
        hover_reaction,
        entered,
        right_pressed,
        scrolled,
    } = row;
    let drawn = nested_menu_row(
        metrics,
        content,
        NestedMenuRowLook {
            emphasis,
            frame_corners,
        },
        press,
        hover_reaction,
    );
    let gestured = mouse_area(drawn);
    let gestured = match entered {
        RowGesture::Publishes(message) => gestured.on_enter(message),
        RowGesture::Inert => gestured,
    };
    let gestured = match right_pressed {
        RowGesture::Publishes(message) => gestured.on_right_press(message),
        RowGesture::Inert => gestured,
    };
    match scrolled {
        RowGesture::Publishes(adjust) => gestured.on_scroll(adjust).into(),
        RowGesture::Inert => gestured.into(),
    }
}

fn section_row<'a, Message: 'a>(
    row: Element<'a, Message>,
    expansion_state: ExpansionState,
    row_corners: Radius,
    divider: SectionDivider,
) -> Element<'a, Message> {
    let backed = container(row)
        .width(Length::Fill)
        .style(move |theme: &Theme| container::Style {
            background: match expansion_state {
                ExpansionState::Expanded => Some(background_weak_color(theme).into()),
                ExpansionState::Collapsed => None,
            },
            border: Border {
                radius: row_corners,
                ..Border::default()
            },
            ..container::Style::default()
        });
    match divider {
        SectionDivider::Above => stack![
            backed,
            container(
                rule::horizontal(OUTLINE_WIDTH_PX).style(|theme: &Theme| rule::Style {
                    color: background_strong_color(theme),
                    radius: Radius::default(),
                    fill_mode: rule::FillMode::Full,
                    snap: true,
                })
            )
            .align_top(Length::Fill)
        ]
        .into(),
        SectionDivider::Absent => backed.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionDivider {
    Above,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AccordionSectionRows {
    item_count: usize,
    expansion_state: ExpansionState,
}

impl AccordionSectionRows {
    const fn shown_rows(self) -> usize {
        match self.expansion_state {
            ExpansionState::Expanded => self.item_count + 1,
            ExpansionState::Collapsed => 1,
        }
    }
}

fn rows_above_section(sections: &[AccordionSectionRows], section: usize) -> usize {
    sections[..section]
        .iter()
        .map(|earlier_section| earlier_section.shown_rows())
        .sum()
}

fn rows_above_item(sections: &[AccordionSectionRows], section: usize, item: usize) -> usize {
    rows_above_section(sections, section) + 1 + item
}

#[cfg(test)]
mod tests {
    use super::{AccordionSectionRows, ExpansionState, rows_above_item};

    const OPEN_THREE: AccordionSectionRows = AccordionSectionRows {
        item_count: 3,
        expansion_state: ExpansionState::Expanded,
    };
    const CLOSED_FOUR: AccordionSectionRows = AccordionSectionRows {
        item_count: 4,
        expansion_state: ExpansionState::Collapsed,
    };

    #[test]
    fn a_closed_section_counts_only_its_header() {
        assert_eq!(CLOSED_FOUR.shown_rows(), 1);
        assert_eq!(OPEN_THREE.shown_rows(), 4);
    }

    #[test]
    fn items_follow_their_header() {
        let sections = [CLOSED_FOUR, OPEN_THREE];
        assert_eq!(rows_above_item(&sections, 1, 0), 2);
        assert_eq!(rows_above_item(&sections, 1, 2), 4);
    }
}
