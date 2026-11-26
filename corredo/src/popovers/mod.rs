mod anchored_popup;
pub use anchored_popup::{
    AnchoredPopup,
    anchored_popup_builder::anchored_popup,
    popup_rendering_options::{
        OutsidePress, OutsidePressCapture, PopupArrival, PopupFraming, PopupPointerInput,
        PopupRegions, PopupRendering,
    },
};
mod popup_trigger;
pub use popup_trigger::{PopupTrigger, PopupVisibility, popup_trigger};
mod popup_cascade;
pub use popup_cascade::{CascadeDirection, CascadeOrientation, PopupGrowth};
mod popup_connector;
pub use popup_connector::{ConnectedSurface, ConnectorTriangle};
mod rectangle_morph;
pub use rectangle_morph::{RectangleMorph, interpolated_rectangle};
mod nested_menu_state;
pub use nested_menu_state::{
    NestedMenuAction, NestedMenuLayout, NestedMenuOpenTrigger, NestedMenuPresentation,
    NestedMenuState, OpenGroupLimit, SubmenuConnector,
};
mod menu_row;
pub use menu_row::{MenuRow, MenuRowContent, MenuRowEmphasis, MenuRowMetrics};
mod cascade_row;
pub use cascade_row::{CascadeRow, cascade_row};
mod notched_panel;
pub use notched_panel::{
    NotchRowAnchor, NotchedPanel, NotchedPanelRegions, SatelliteRole, notched_panel,
};
mod nested_menu;
pub use nested_menu::{
    CaretPointing, MenuEntry, MenuItem, MenuSubmenu, NestedMenu, NestedMenuStyle,
    NestedMenuSurfaces, nested_menu, submenu_header_emphasis,
};
mod anchor_marker;
pub use anchor_marker::anchor_marker;
mod dropped_anchor_watch;
pub use dropped_anchor_watch::dropped_anchor_watch;
