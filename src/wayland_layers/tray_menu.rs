pub mod menu_content;
pub mod menu_placement;
pub mod menu_surfaces;

use iced::Rectangle;

use crate::wayland_layers::surface::Position;
pub use crate::wayland_layers::tray_menu::menu_surfaces::TrayMenuHost;

#[derive(Debug, Clone)]
pub struct MenuAnchor {
    pub anchor: Rectangle,
    pub output: String,
    pub float_offset: i32,
    pub position: Position,
}
