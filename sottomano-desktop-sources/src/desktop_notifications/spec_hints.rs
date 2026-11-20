use std::{collections::HashMap, path::PathBuf, str::FromStr, sync::Arc};

use image::RgbaImage;
use url::Url;
use zbus::zvariant::OwnedValue;

use crate::{
    desktop_notifications::{
        spec_categories::NotificationCategory, spec_image_data::image_data_pixels,
    },
    non_empty_text::non_empty,
    system_protocol_clients::dbus_desktop_buses::prop,
};

const FILE_URI_PREFIX: &str = "file:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconReference {
    FileUri(PathBuf),
    ThemeName(String),
}

#[derive(Debug, thiserror::Error)]
pub enum IconReferenceError {
    #[error("malformed file uri: {0}")]
    MalformedFileUri(#[from] url::ParseError),
    #[error("file uri {0} does not name a local path")]
    NotLocalPath(Url),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeFactoPath {
    Absolute(PathBuf),
    NotAPath,
}

fn de_facto_absolute_path(reference_text: &str) -> DeFactoPath {
    if reference_text.starts_with('/') {
        DeFactoPath::Absolute(PathBuf::from(reference_text))
    } else {
        DeFactoPath::NotAPath
    }
}

impl FromStr for IconReference {
    type Err = IconReferenceError;

    fn from_str(reference_text: &str) -> Result<Self, Self::Err> {
        if let DeFactoPath::Absolute(icon_path) = de_facto_absolute_path(reference_text) {
            return Ok(IconReference::FileUri(icon_path));
        }
        if !reference_text.starts_with(FILE_URI_PREFIX) {
            return Ok(IconReference::ThemeName(reference_text.to_owned()));
        }
        let file_uri = Url::parse(reference_text)?;
        match file_uri.to_file_path() {
            Ok(icon_path) => Ok(IconReference::FileUri(icon_path)),
            Err(()) => Err(IconReferenceError::NotLocalPath(file_uri)),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum SpecImage {
    ImageData(Arc<RgbaImage>),
    ImagePath(IconReference),
    AppIcon(IconReference),
    LegacyIconData(Arc<RgbaImage>),
    #[default]
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SpecAppIcon {
    Given(IconReference),
    #[default]
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SoundRequest {
    Suppressed,
    File(PathBuf),
    ThemeName(String),
    #[default]
    Unspecified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Persistence {
    #[default]
    Retained,
    Transient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HintFlag {
    Set,
    Unset,
}

pub(super) fn hint_str(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    non_empty(prop::<String>(hints, key)?)
}

pub(super) fn hint_flag(hints: &HashMap<String, OwnedValue>, key: &str) -> HintFlag {
    match prop::<bool>(hints, key) {
        Some(true) => HintFlag::Set,
        Some(false) | None => HintFlag::Unset,
    }
}

fn icon_reference_hint(reference_text: &str, hint_name: &str) -> Option<IconReference> {
    reference_text
        .parse()
        .inspect_err(|reference_error| {
            tracing::warn!(
                hint = hint_name,
                error = %reference_error,
                "notifications: icon reference is unusable"
            );
        })
        .ok()
}

fn pixels_hint(hints: &HashMap<String, OwnedValue>, keys: &[&str]) -> Option<Arc<RgbaImage>> {
    keys.iter()
        .find_map(|key| hints.get(*key))
        .and_then(|value| image_data_pixels(value))
        .map(Arc::new)
}

#[must_use]
pub fn spec_app_icon(app_icon_text: &str) -> SpecAppIcon {
    match non_empty(app_icon_text.to_owned())
        .and_then(|reference_text| icon_reference_hint(&reference_text, "app_icon"))
    {
        Some(icon_reference) => SpecAppIcon::Given(icon_reference),
        None => SpecAppIcon::Absent,
    }
}

#[must_use]
pub fn spec_image(hints: &HashMap<String, OwnedValue>, app_icon: &SpecAppIcon) -> SpecImage {
    if let Some(pixels) = pixels_hint(hints, &["image-data", "image_data"]) {
        return SpecImage::ImageData(pixels);
    }
    let image_path = hint_str(hints, "image-path")
        .or_else(|| hint_str(hints, "image_path"))
        .and_then(|reference_text| icon_reference_hint(&reference_text, "image-path"));
    if let Some(icon_reference) = image_path {
        return SpecImage::ImagePath(icon_reference);
    }
    if let SpecAppIcon::Given(icon_reference) = app_icon {
        return SpecImage::AppIcon(icon_reference.clone());
    }
    match pixels_hint(hints, &["icon_data"]) {
        Some(pixels) => SpecImage::LegacyIconData(pixels),
        None => SpecImage::Absent,
    }
}

#[must_use]
pub fn sound_request(hints: &HashMap<String, OwnedValue>) -> SoundRequest {
    if hint_flag(hints, "suppress-sound") == HintFlag::Set {
        return SoundRequest::Suppressed;
    }
    if let Some(sound_file) = hint_str(hints, "sound-file") {
        return SoundRequest::File(PathBuf::from(sound_file));
    }
    match hint_str(hints, "sound-name") {
        Some(sound_name) => SoundRequest::ThemeName(sound_name),
        None => SoundRequest::Unspecified,
    }
}

#[must_use]
pub fn notification_category(hints: &HashMap<String, OwnedValue>) -> NotificationCategory {
    hint_str(hints, "category").map_or(NotificationCategory::Unspecified, |category_name| {
        NotificationCategory::from(category_name.as_str())
    })
}

#[must_use]
pub fn persistence(hints: &HashMap<String, OwnedValue>) -> Persistence {
    match hint_flag(hints, "transient") {
        HintFlag::Set => Persistence::Transient,
        HintFlag::Unset => Persistence::Retained,
    }
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::{Str, Value};

    use super::*;

    fn text(value: &str) -> OwnedValue {
        OwnedValue::from(Str::from(value.to_owned()))
    }

    fn flag(value: bool) -> OwnedValue {
        OwnedValue::from(value)
    }

    fn one_pixel_image_data() -> OwnedValue {
        OwnedValue::try_from(Value::from((
            1_i32,
            1_i32,
            3_i32,
            false,
            8_i32,
            3_i32,
            vec![9_u8, 8, 7],
        )))
        .unwrap()
    }

    #[test]
    fn a_file_uri_names_a_local_path() {
        assert_eq!(
            "file:///tmp/screen%20shot.png"
                .parse::<IconReference>()
                .unwrap(),
            IconReference::FileUri(PathBuf::from("/tmp/screen shot.png"))
        );
    }

    #[test]
    fn a_bare_absolute_path_is_tolerated_as_a_file() {
        assert_eq!(
            "/tmp/icon.png".parse::<IconReference>().unwrap(),
            IconReference::FileUri(PathBuf::from("/tmp/icon.png"))
        );
    }

    #[test]
    fn anything_else_is_a_theme_name() {
        assert_eq!(
            "drive-harddisk-usb-symbolic"
                .parse::<IconReference>()
                .unwrap(),
            IconReference::ThemeName("drive-harddisk-usb-symbolic".to_owned())
        );
    }

    #[test]
    fn a_remote_file_uri_is_rejected() {
        assert!(matches!(
            "file://elsewhere/tmp/icon.png".parse::<IconReference>(),
            Err(IconReferenceError::NotLocalPath(_))
        ));
    }

    #[test]
    fn image_data_outranks_image_path() {
        let hints = HashMap::from([
            ("image-data".to_owned(), one_pixel_image_data()),
            ("image-path".to_owned(), text("camera-photo")),
        ]);
        assert!(matches!(
            spec_image(&hints, &SpecAppIcon::Absent),
            SpecImage::ImageData(_)
        ));
    }

    #[test]
    fn image_path_outranks_app_icon() {
        let hints = HashMap::from([("image-path".to_owned(), text("camera-photo"))]);
        let app_icon = spec_app_icon("firefox");
        assert!(matches!(
            spec_image(&hints, &app_icon),
            SpecImage::ImagePath(IconReference::ThemeName(icon_name)) if icon_name == "camera-photo"
        ));
    }

    #[test]
    fn app_icon_outranks_legacy_icon_data() {
        let hints = HashMap::from([("icon_data".to_owned(), one_pixel_image_data())]);
        let app_icon = spec_app_icon("firefox");
        assert!(matches!(
            spec_image(&hints, &app_icon),
            SpecImage::AppIcon(_)
        ));
        assert!(matches!(
            spec_image(&hints, &SpecAppIcon::Absent),
            SpecImage::LegacyIconData(_)
        ));
    }

    #[test]
    fn suppress_sound_outranks_named_sounds() {
        let hints = HashMap::from([
            ("suppress-sound".to_owned(), flag(true)),
            ("sound-name".to_owned(), text("bell")),
        ]);
        assert_eq!(sound_request(&hints), SoundRequest::Suppressed);
    }

    #[test]
    fn sound_file_outranks_sound_name() {
        let hints = HashMap::from([
            ("sound-file".to_owned(), text("/tmp/ding.oga")),
            ("sound-name".to_owned(), text("bell")),
        ]);
        assert_eq!(
            sound_request(&hints),
            SoundRequest::File(PathBuf::from("/tmp/ding.oga"))
        );
    }

    #[test]
    fn sound_name_is_a_theme_sound() {
        let hints = HashMap::from([("sound-name".to_owned(), text("bell"))]);
        assert_eq!(
            sound_request(&hints),
            SoundRequest::ThemeName("bell".to_owned())
        );
    }

    #[test]
    fn the_transient_hint_bypasses_persistence() {
        let hints = HashMap::from([("transient".to_owned(), flag(true))]);
        assert_eq!(persistence(&hints), Persistence::Transient);
        assert_eq!(persistence(&HashMap::new()), Persistence::Retained);
    }
}
