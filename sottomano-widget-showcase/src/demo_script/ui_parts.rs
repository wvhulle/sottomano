use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UiPart {
    GroupMenus,
    HoverCards,
    PanelFaces,
    PanelPins,
    Sliders,
    AppLauncher,
    Workspaces,
    WindowList,
    Toasts,
    FrostedBackdrop,
}

impl UiPart {
    pub const ALL: [UiPart; 10] = [
        UiPart::GroupMenus,
        UiPart::HoverCards,
        UiPart::PanelFaces,
        UiPart::PanelPins,
        UiPart::Sliders,
        UiPart::AppLauncher,
        UiPart::Workspaces,
        UiPart::WindowList,
        UiPart::Toasts,
        UiPart::FrostedBackdrop,
    ];

    const fn selector_name(self) -> &'static str {
        match self {
            UiPart::GroupMenus => "group-menus",
            UiPart::HoverCards => "hover-cards",
            UiPart::PanelFaces => "panel-faces",
            UiPart::PanelPins => "panel-pins",
            UiPart::Sliders => "sliders",
            UiPart::AppLauncher => "app-launcher",
            UiPart::Workspaces => "workspaces",
            UiPart::WindowList => "window-list",
            UiPart::Toasts => "toasts",
            UiPart::FrostedBackdrop => "frosted-backdrop",
        }
    }

    #[must_use]
    pub fn named(selector: &str) -> Option<UiPart> {
        UiPart::ALL
            .into_iter()
            .find(|ui_part| ui_part.selector_name() == selector)
    }
}

impl Display for UiPart {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.selector_name())
    }
}
