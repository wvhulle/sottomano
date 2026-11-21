use std::rc::Rc;

use iced::{Element, widget::container};

use super::{ActionReport, CaretGlyph, MenuChrome, MenuEntry};
use crate::{
    gauges::activity_line,
    orientation::Axis,
    popovers::{
        NestedMenuAction, NestedMenuPresentation, NestedMenuState, NotchRowAnchor, NotchedPanel,
        SatelliteRole, notched_panel,
    },
    selection_scroll::{ScrollEdgeFade, selection_following_scroll},
    theming::{background_weak_color, style_metrics::OUTLINE_WIDTH_PX},
    transitions::ExpansionState,
    width_layout::equal_width_column,
};

pub(super) struct MenuBuilder<'view, SubmenuKey, Message> {
    pub(super) state: NestedMenuState<SubmenuKey>,
    pub(super) presentation: NestedMenuPresentation,
    pub(super) on_action: ActionReport<'view, SubmenuKey, Message>,
    pub(super) caret: CaretGlyph<'view, Message>,
    pub(super) chrome: MenuChrome,
}

impl<'view, SubmenuKey: Clone + PartialEq + 'view, Message: Clone + 'view>
    MenuBuilder<'view, SubmenuKey, Message>
{
    pub(super) fn submenu_expansion(&self, key: &SubmenuKey) -> ExpansionState {
        if self.state.open_submenus.contains(key) {
            ExpansionState::Expanded
        } else {
            ExpansionState::Collapsed
        }
    }

    pub(super) fn divider_height_px(&self) -> f32 {
        self.chrome.metrics.item_spacing_px + OUTLINE_WIDTH_PX
    }

    pub(super) fn entry_height_px(&self, entry: &MenuEntry<'view, SubmenuKey, Message>) -> f32 {
        match entry {
            MenuEntry::Divider => self.divider_height_px(),
            MenuEntry::Item(_) | MenuEntry::Submenu(_) => self.chrome.metrics.row_height_px,
        }
    }

    pub(super) fn divider(&self) -> Element<'view, Message> {
        container(activity_line(
            Axis::Horizontal,
            background_weak_color,
            1.0,
            OUTLINE_WIDTH_PX,
        ))
        .center_y(self.divider_height_px())
        .into()
    }

    pub(super) fn scrolled_list(
        &self,
        rows: Vec<Element<'view, Message>>,
        depth: usize,
    ) -> Element<'view, Message> {
        let on_action = Rc::clone(&self.on_action);
        selection_following_scroll(equal_width_column(rows), Axis::Vertical)
            .wheel_line_travel(self.chrome.wheel_line_px)
            .paced(self.chrome.pacing)
            .edge_fade(ScrollEdgeFade::within_frame(
                self.chrome.style.list_frame,
                self.chrome.style.fade_extent_px,
            ))
            .controlled(self.state.scrolled_px(depth), move |scrolled_px| {
                on_action(NestedMenuAction::Scrolled { depth, scrolled_px })
            })
            .into()
    }

    pub(super) fn panel(
        &self,
        role: SatelliteRole,
        content: Element<'view, Message>,
        row: NotchRowAnchor,
    ) -> NotchedPanel<'view, Message> {
        let frame = match role {
            SatelliteRole::ItemList => self.chrome.style.list_frame,
            SatelliteRole::Beside => self.chrome.style.beside_frame,
        };
        notched_panel(content, self.chrome.orientation, row)
            .role(role)
            .frame(frame)
            .row_height(self.chrome.metrics.row_height_px)
            .gap_to_parent(self.chrome.style.gap_to_parent_px)
            .submenu_connector(self.chrome.style.submenu_connector)
            .morph(self.chrome.style.morph)
    }
}
