use std::{rc::Rc, time::Duration};

use iced::{Color, Element, Theme, border::Radius, widget::Space};

use crate::{
    popovers::{
        CascadeDirection, CascadeOrientation, MenuRowMetrics, NestedMenuAction, NestedMenuLayout,
        NestedMenuPresentation, NestedMenuState, PopupGrowth, SubmenuConnector, cascade_row,
    },
    selection_scroll::ScrollPacing,
    theming::FrameStyle,
};

const DEFAULT_MORPH: Duration = Duration::from_millis(160);
const DEFAULT_REVEAL_DELAY: Duration = Duration::from_millis(180);
const DEFAULT_WHEEL_LINE_PX: f32 = 40.0;

mod accordion_rows;
mod cascading_panels;
mod menu_builder;
mod menu_entries;
#[cfg(test)]
mod nested_menu_tests;
mod submenu_header;

use menu_builder::MenuBuilder;
pub use menu_entries::{MenuEntry, MenuItem, MenuSubmenu};
use submenu_header::text_caret;
pub use submenu_header::{CaretPointing, submenu_header_emphasis};

pub struct NestedMenuSurfaces<'view, Message> {
    pub primary: Element<'view, Message>,
    pub satellites: Vec<Element<'view, Message>>,
}

impl<'view, Message: 'view> NestedMenuSurfaces<'view, Message> {
    #[must_use]
    pub fn map<Mapped: 'view>(
        self,
        mapping: impl Fn(Message) -> Mapped + Clone + 'view,
    ) -> NestedMenuSurfaces<'view, Mapped> {
        NestedMenuSurfaces {
            primary: self.primary.map(mapping.clone()),
            satellites: self
                .satellites
                .into_iter()
                .map(|satellite| satellite.map(mapping.clone()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NestedMenuStyle {
    pub list_frame: FrameStyle,
    pub beside_frame: FrameStyle,
    pub morph: Duration,
    pub gap_to_parent_px: f32,
    pub submenu_connector: SubmenuConnector,
    pub fade_extent_px: f32,
    pub reveal_delay: Duration,
}

impl Default for NestedMenuStyle {
    fn default() -> Self {
        Self {
            list_frame: FrameStyle::default(),
            beside_frame: FrameStyle::default(),
            morph: DEFAULT_MORPH,
            gap_to_parent_px: 0.0,
            submenu_connector: SubmenuConnector::default(),
            fade_extent_px: MenuRowMetrics::default().row_height_px,
            reveal_delay: DEFAULT_REVEAL_DELAY,
        }
    }
}

impl NestedMenuStyle {
    pub(crate) fn frame_corners(self) -> Radius {
        Radius::from(self.list_frame.corner_radius().top_left)
    }
}

pub(crate) type ActionReport<'view, SubmenuKey, Message> =
    Rc<dyn Fn(NestedMenuAction<SubmenuKey>) -> Message + 'view>;
pub(crate) type CaretGlyph<'view, Message> =
    Box<dyn Fn(CaretPointing, fn(&Theme) -> Color) -> Element<'view, Message> + 'view>;

pub struct NestedMenu<'view, SubmenuKey, Message> {
    entries: Vec<MenuEntry<'view, SubmenuKey, Message>>,
    state: NestedMenuState<SubmenuKey>,
    presentation: NestedMenuPresentation,
    on_action: ActionReport<'view, SubmenuKey, Message>,
    caret: CaretGlyph<'view, Message>,
    chrome: MenuChrome,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MenuChrome {
    pub(crate) orientation: CascadeOrientation,
    pub(crate) metrics: MenuRowMetrics,
    pub(crate) style: NestedMenuStyle,
    pub(crate) wheel_line_px: f32,
    pub(crate) pacing: ScrollPacing,
}

pub fn nested_menu<'view, SubmenuKey: Clone, Message: 'view>(
    entries: impl IntoIterator<Item = MenuEntry<'view, SubmenuKey, Message>>,
    state: &NestedMenuState<SubmenuKey>,
    presentation: NestedMenuPresentation,
    on_action: impl Fn(NestedMenuAction<SubmenuKey>) -> Message + 'view,
) -> NestedMenu<'view, SubmenuKey, Message> {
    NestedMenu {
        entries: entries.into_iter().collect(),
        state: state.clone(),
        presentation,
        on_action: Rc::new(on_action),
        caret: Box::new(text_caret),
        chrome: MenuChrome {
            orientation: CascadeOrientation {
                direction: CascadeDirection::Rightward,
                growth: PopupGrowth::Downward,
            },
            metrics: MenuRowMetrics::default(),
            style: NestedMenuStyle::default(),
            wheel_line_px: DEFAULT_WHEEL_LINE_PX,
            pacing: ScrollPacing::Instant,
        },
    }
}

impl<'view, SubmenuKey, Message> NestedMenu<'view, SubmenuKey, Message> {
    fn with_chrome(self, chrome: MenuChrome) -> Self {
        Self { chrome, ..self }
    }

    #[must_use]
    pub fn metrics(self, metrics: MenuRowMetrics) -> Self {
        let chrome = MenuChrome {
            metrics,
            ..self.chrome
        };
        self.with_chrome(chrome)
    }

    #[must_use]
    pub fn orientation(self, orientation: CascadeOrientation) -> Self {
        let chrome = MenuChrome {
            orientation,
            ..self.chrome
        };
        self.with_chrome(chrome)
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
    pub fn caret(
        self,
        caret: impl Fn(CaretPointing, fn(&Theme) -> Color) -> Element<'view, Message> + 'view,
    ) -> Self {
        Self {
            caret: Box::new(caret),
            ..self
        }
    }
}

impl<'view, SubmenuKey: Clone + PartialEq + 'view, Message: Clone + 'view>
    NestedMenu<'view, SubmenuKey, Message>
{
    #[must_use]
    pub fn surfaces(self) -> Option<NestedMenuSurfaces<'view, Message>> {
        let NestedMenu {
            entries,
            state,
            presentation,
            on_action,
            caret,
            chrome,
        } = self;
        if entries.is_empty() {
            return None;
        }
        let builder = MenuBuilder {
            state,
            presentation,
            on_action,
            caret,
            chrome,
        };
        Some(match presentation.layout {
            NestedMenuLayout::Cascading => builder.cascading_surfaces(entries),
            NestedMenuLayout::Accordion => builder.accordion_surfaces(entries),
        })
    }
}

impl<'view, SubmenuKey: Clone + PartialEq + 'view, Message: Clone + 'view>
    From<NestedMenu<'view, SubmenuKey, Message>> for Element<'view, Message>
{
    fn from(menu: NestedMenu<'view, SubmenuKey, Message>) -> Self {
        let orientation = menu.chrome.orientation;
        match menu.surfaces() {
            Some(NestedMenuSurfaces {
                primary,
                satellites,
            }) => cascade_row(primary, satellites, orientation),
            None => Space::new().into(),
        }
    }
}
