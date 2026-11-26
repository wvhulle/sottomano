pub mod anchored_popup;
pub mod core_grid;
pub mod hover_area;
pub mod hover_card_body;
pub mod hover_card_content;
pub mod hover_card_surfaces;
pub mod hover_card_transition;
pub mod laid_out_anchors;
use std::borrow::Cow;

use corredo::transitions::ExpansionState;
use iced::{Rectangle, window::Id};
use sottomano_toml_config::screen_edges::Position;

pub use crate::hover_cards::{
    hover_area::hoverable,
    hover_card_surfaces::HoverCardHost,
    laid_out_anchors::{AnchorVisibility, WidgetAnchor},
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::indicator_group_identity::{IndicatorGroupIdentity, IndicatorGroupTitle},
    group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic, PanelPin},
    output_windows::{OutputWindowsInteraction, WindowTarget},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndicatorKey {
    pub group: IndicatorGroupIdentity,
    pub name: Cow<'static, str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HoverTarget {
    Indicator(IndicatorKey),
    PinnedOnPanel(Box<PanelPin>),
    GroupHeadline(Box<IndicatorGroupIdentity>),
    GroupMenu(GroupMenuTopic),
    WindowTopMenu(i32),
    Window(WindowTarget),
    Workspace(u64),
    WorkspaceOrder,
    WorkspaceFolding,
}

impl From<IndicatorKey> for HoverTarget {
    fn from(indicator_key: IndicatorKey) -> Self {
        HoverTarget::Indicator(indicator_key)
    }
}

impl HoverTarget {
    #[must_use]
    pub fn indicator_key(&self) -> Option<&IndicatorKey> {
        match self {
            HoverTarget::Indicator(indicator_key) => Some(indicator_key),
            HoverTarget::PinnedOnPanel(panel_pin) => match &panel_pin.entry {
                GroupMenuPin::Item(indicator_key) => Some(indicator_key),
                GroupMenuPin::Group(_) => None,
            },
            HoverTarget::GroupHeadline(_)
            | HoverTarget::GroupMenu(_)
            | HoverTarget::WindowTopMenu(_)
            | HoverTarget::Window(_)
            | HoverTarget::Workspace(_)
            | HoverTarget::WorkspaceOrder
            | HoverTarget::WorkspaceFolding => None,
        }
    }

    #[must_use]
    pub fn headline_group(&self) -> Option<&IndicatorGroupIdentity> {
        match self {
            HoverTarget::GroupHeadline(group_identity) => Some(group_identity),
            HoverTarget::PinnedOnPanel(panel_pin) => match &panel_pin.entry {
                GroupMenuPin::Group(group_identity) => Some(group_identity),
                GroupMenuPin::Item(_) => None,
            },
            HoverTarget::Indicator(_)
            | HoverTarget::GroupMenu(_)
            | HoverTarget::WindowTopMenu(_)
            | HoverTarget::Window(_)
            | HoverTarget::Workspace(_)
            | HoverTarget::WorkspaceOrder
            | HoverTarget::WorkspaceFolding => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MouseGesture {
    LeftClick,
    MiddleClick,
    RightClick,
    Scroll,
}

impl MouseGesture {
    #[must_use]
    pub fn glyph(self) -> Phosphor {
        match self {
            MouseGesture::LeftClick => Phosphor::MouseLeftClick(Weight::Regular),
            MouseGesture::MiddleClick => Phosphor::MouseMiddleClick(Weight::Regular),
            MouseGesture::RightClick => Phosphor::MouseRightClick(Weight::Regular),
            MouseGesture::Scroll => Phosphor::MouseScroll(Weight::Regular),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressAction {
    pub gesture: MouseGesture,
    pub does: Cow<'static, str>,
}

impl PressAction {
    pub fn new(gesture: MouseGesture, does: impl Into<Cow<'static, str>>) -> Self {
        Self {
            gesture,
            does: does.into(),
        }
    }

    #[must_use]
    pub fn group_handle(expansion_state: ExpansionState) -> Self {
        PressAction::new(
            MouseGesture::LeftClick,
            match expansion_state {
                ExpansionState::Expanded => "collapse the group",
                ExpansionState::Collapsed => "expand the group",
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CardHeader {
    Unheaded,
    Pinned {
        group_title: IndicatorGroupTitle,
        pin: Box<PanelPin>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PanelPinOrigin {
    #[default]
    Unpinned,
    Pinned(Box<PanelPin>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenuHandleIndicator {
    pub pointing: PointerPresence,
    pub menu_expansion: ExpansionState,
}

impl MenuHandleIndicator {
    #[must_use]
    pub fn settled_reveal(self) -> f32 {
        match self.menu_expansion {
            ExpansionState::Expanded => 1.0,
            ExpansionState::Collapsed => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerPresence {
    PointedAt,
    #[default]
    NotPointedAt,
}

#[derive(Debug, Clone)]
pub enum AnchorEvent {
    LaidOut(WidgetAnchor),
    Entered {
        target: HoverTarget,
    },
    HandlePointed {
        target: HoverTarget,
        pointing: PointerPresence,
    },
    HandlePressed {
        target: HoverTarget,
    },
}

#[derive(Debug, Clone)]
pub enum HoverCardMessage {
    Anchor(AnchorEvent),
    OnSurface {
        surface: Id,
        position: Position,
        event: Box<AnchorEvent>,
    },
    ToastCardsLaidOut {
        output: String,
        toast_cards: Vec<Rectangle>,
    },
    Exit {
        target: HoverTarget,
    },
    EnterHoverCard,
    ExitHoverCard,
    DismissHeldOpen,
    SwitchIntentElapsed,
    ShownWidgetDropped,
    ConfirmShownWidgetDropped,
}

#[derive(Debug, Clone)]
pub enum PopupMessage {
    HoverCard(HoverCardMessage),
    CopyText(String),
    OpenUrl(String),
    FocusWindow(u64),
    RefreshHoveredIndicator,
    OutputWindows(OutputWindowsInteraction),
    ReleaseFromPanel(PanelPin),
}
