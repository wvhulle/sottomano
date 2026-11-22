use std::path::{Path, PathBuf};

use iced::{
    Color, Element,
    widget::{Space, container},
};
use sottomano_desktop_sources::{
    desktop_notifications::{
        notification_data::{DesktopEntryHint, NotificationData},
        notification_image_policy::ImageRole,
        spec_hints::{IconReference, SpecAppIcon, SpecImage},
    },
    theme_icons::icon_requests::{
        IconRequest, app_icon_request, notification_file_icon_request,
        notification_image_icon_request,
    },
};

use crate::{
    glyph_icons::icon_glyph::IconGlyph,
    icon_catalog::{drawable_icon_catalog::IconCatalog, icon_handle::IconHandle},
    toast_messages::NotificationMessage,
};

const TOAST_ICON_SIZE_PX: f32 = 44.0;
const TOAST_APP_BADGE_SIZE_PX: f32 = 20.0;

#[derive(Debug, Clone)]
pub enum CardIcon {
    Handle(IconHandle),
    IconGlyph(IconGlyph),
    Themed(IconRequest),
}

impl CardIcon {
    #[must_use]
    pub fn icon_request(&self) -> Option<&IconRequest> {
        match self {
            CardIcon::Themed(request) => Some(request),
            CardIcon::Handle(_) | CardIcon::IconGlyph(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CardImage {
    Preview { icon: CardIcon, file: PathBuf },
    Icon(CardIcon),
    Imageless,
}

#[derive(Debug, Clone)]
pub enum CardBadge {
    Badged(CardIcon),
    Unbadged,
}

impl CardImage {
    fn referenced(icon_reference: &IconReference, image_role: ImageRole) -> Self {
        match icon_reference {
            IconReference::ThemeName(icon_name) => {
                Self::Icon(CardIcon::Themed(notification_image_icon_request(icon_name)))
            }
            IconReference::FileUri(image_path) => match image_role {
                ImageRole::Picture => Self::Preview {
                    icon: CardIcon::Handle(IconHandle::at(image_path)),
                    file: image_path.clone(),
                },
                ImageRole::Icon => {
                    Self::Icon(CardIcon::Themed(notification_file_icon_request(image_path)))
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardIcons {
    pub image: CardImage,
    pub badge: CardBadge,
}

impl CardIcons {
    pub fn previewed_file(&self) -> Option<&Path> {
        match &self.image {
            CardImage::Preview { file, .. } => Some(file),
            CardImage::Icon(_) | CardImage::Imageless => None,
        }
    }
}

impl From<IconGlyph> for CardIcons {
    fn from(glyph: IconGlyph) -> Self {
        Self {
            image: CardImage::Icon(CardIcon::IconGlyph(glyph)),
            badge: CardBadge::Unbadged,
        }
    }
}

#[must_use]
pub fn resolve_icons(data: &NotificationData) -> CardIcons {
    let image = match &data.image {
        SpecImage::ImageData(pixels) | SpecImage::LegacyIconData(pixels) => {
            CardImage::Icon(CardIcon::Handle(IconHandle::from(pixels.as_ref())))
        }
        SpecImage::ImagePath(icon_reference) | SpecImage::AppIcon(icon_reference) => {
            CardImage::referenced(icon_reference, data.image_role)
        }
        SpecImage::Absent => CardImage::Imageless,
    };
    let badge_app_icon = match &data.image {
        SpecImage::AppIcon(_) => None,
        SpecImage::ImageData(_)
        | SpecImage::ImagePath(_)
        | SpecImage::LegacyIconData(_)
        | SpecImage::Absent => match &data.app_icon {
            SpecAppIcon::Given(IconReference::ThemeName(icon_name)) => Some(icon_name.as_str()),
            SpecAppIcon::Given(IconReference::FileUri(_)) | SpecAppIcon::Absent => None,
        },
    };
    let desktop_entry = match &data.desktop_entry {
        DesktopEntryHint::Entry(entry) => Some(entry.as_str()),
        DesktopEntryHint::Unhinted => None,
    };
    let badge = match badge_app_icon.or(desktop_entry) {
        Some(app_id) => CardBadge::Badged(CardIcon::Themed(app_icon_request(app_id))),
        None => CardBadge::Unbadged,
    };
    CardIcons { image, badge }
}

pub(super) fn icon_slot<'view>(
    icons: &CardIcons,
    icon_catalog: &IconCatalog,
    reveal: f32,
    text: Color,
) -> Option<Element<'view, NotificationMessage>> {
    let CardImage::Icon(image) = &icons.image else {
        return match &icons.badge {
            CardBadge::Badged(badge) => Some(icon_widget(
                badge,
                icon_catalog,
                TOAST_ICON_SIZE_PX,
                reveal,
                text,
            )),
            CardBadge::Unbadged => None,
        };
    };
    let main = icon_widget(image, icon_catalog, TOAST_ICON_SIZE_PX, reveal, text);
    let CardBadge::Badged(badge) = &icons.badge else {
        return Some(main);
    };
    let overlay = container(icon_widget(
        badge,
        icon_catalog,
        TOAST_APP_BADGE_SIZE_PX,
        reveal,
        text,
    ))
    .width(TOAST_ICON_SIZE_PX)
    .height(TOAST_ICON_SIZE_PX);
    Some(iced::widget::stack![main, overlay].clip(true).into())
}

pub(super) fn preview_widget<'view>(
    icon: &CardIcon,
    icon_catalog: &IconCatalog,
    max_height: f32,
    reveal: f32,
    text: Color,
) -> Element<'view, NotificationMessage> {
    let preview: Element<'view, NotificationMessage> = match icon {
        CardIcon::Handle(handle) => handle.scaled_down(reveal),
        CardIcon::IconGlyph(glyph) => glyph.view(max_height, move |_| text.scale_alpha(reveal)),
        CardIcon::Themed(request) => match icon_catalog.drawable_icon(request) {
            Some(drawable_icon) => drawable_icon.scaled_down(reveal),
            None => Space::new().into(),
        },
    };
    container(preview).max_height(max_height).into()
}

fn icon_widget<'view>(
    icon: &CardIcon,
    icon_catalog: &IconCatalog,
    size: f32,
    reveal: f32,
    text: Color,
) -> Element<'view, NotificationMessage> {
    match icon {
        CardIcon::IconGlyph(glyph) => glyph.view(size, move |_| text.scale_alpha(reveal)),
        CardIcon::Handle(handle) => handle.view(size, reveal),
        CardIcon::Themed(request) => match icon_catalog.drawable_icon(request) {
            Some(drawable_icon) => drawable_icon.view(size, reveal),
            None => Space::new().width(size).height(size).into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn notification_with(image: SpecImage, image_role: ImageRole) -> NotificationData {
        NotificationData {
            image,
            image_role,
            ..NotificationData::default()
        }
    }

    fn screenshot_file() -> IconReference {
        IconReference::FileUri(PathBuf::from("/home/u/Pictures/Screenshot.png"))
    }

    #[test]
    fn a_picture_file_is_previewed() {
        let card_icons = resolve_icons(&notification_with(
            SpecImage::ImagePath(screenshot_file()),
            ImageRole::Picture,
        ));
        assert_eq!(
            card_icons.previewed_file(),
            Some(Path::new("/home/u/Pictures/Screenshot.png"))
        );
    }

    #[test]
    fn an_icon_file_goes_through_the_catalog_without_file_actions() {
        let card_icons = resolve_icons(&notification_with(
            SpecImage::ImagePath(screenshot_file()),
            ImageRole::Icon,
        ));
        assert_eq!(card_icons.previewed_file(), None);
        assert!(matches!(
            card_icons.image,
            CardImage::Icon(CardIcon::Themed(IconRequest::IconFile { .. }))
        ));
    }

    #[test]
    fn a_theme_name_is_a_named_catalog_icon() {
        let card_icons = resolve_icons(&notification_with(
            SpecImage::ImagePath(IconReference::ThemeName(
                "drive-harddisk-usb-symbolic".to_owned(),
            )),
            ImageRole::Icon,
        ));
        assert!(matches!(
            card_icons.image,
            CardImage::Icon(CardIcon::Themed(IconRequest::NamedIcon { .. }))
        ));
    }

    #[test]
    fn the_app_icon_is_not_repeated_as_a_badge() {
        let app_icon = IconReference::ThemeName("firefox".to_owned());
        let card_icons = resolve_icons(&NotificationData {
            image: SpecImage::AppIcon(app_icon.clone()),
            app_icon: SpecAppIcon::Given(app_icon),
            ..NotificationData::default()
        });
        assert!(matches!(card_icons.badge, CardBadge::Unbadged));
    }
}
