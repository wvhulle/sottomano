use std::path::{Path, PathBuf};

use crate::{
    blocking_background_work::{Panicked, blocking_poll},
    desktop_notifications::spec_hints::{IconReference, SpecImage},
    theme_icons::icon_file_loading::is_svg_file,
};

const PICTURE_SIDE_THRESHOLD_PX: u32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageRole {
    Picture,
    #[default]
    Icon,
}

#[derive(Debug, thiserror::Error)]
enum ImageHeaderError {
    #[error("cannot read the image header: {0}")]
    Header(#[from] image::ImageError),
    #[error(transparent)]
    Panicked(#[from] Panicked),
}

#[must_use]
pub fn role_by_side_length(width_px: u32, height_px: u32) -> ImageRole {
    if width_px.max(height_px) > PICTURE_SIDE_THRESHOLD_PX {
        ImageRole::Picture
    } else {
        ImageRole::Icon
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageSource<'image> {
    File(&'image Path),
    NotAFile,
}

impl<'image> From<&'image SpecImage> for ImageSource<'image> {
    fn from(spec_image: &'image SpecImage) -> Self {
        match spec_image {
            SpecImage::ImagePath(IconReference::FileUri(image_path))
            | SpecImage::AppIcon(IconReference::FileUri(image_path)) => {
                ImageSource::File(image_path)
            }
            SpecImage::ImagePath(IconReference::ThemeName(_))
            | SpecImage::AppIcon(IconReference::ThemeName(_))
            | SpecImage::ImageData(_)
            | SpecImage::LegacyIconData(_)
            | SpecImage::Absent => ImageSource::NotAFile,
        }
    }
}

async fn raster_file_role(image_path: PathBuf) -> ImageRole {
    let header_path = image_path.clone();
    let dimensions = blocking_poll(move || {
        image::image_dimensions(&header_path).map_err(ImageHeaderError::from)
    })
    .await;
    match dimensions {
        Ok((width_px, height_px)) => role_by_side_length(width_px, height_px),
        Err(header_error) => {
            tracing::warn!(
                path = %image_path.display(),
                error = %header_error,
                "notifications: image file is shown as an icon"
            );
            ImageRole::Icon
        }
    }
}

pub async fn image_role(spec_image: &SpecImage) -> ImageRole {
    let ImageSource::File(image_path) = ImageSource::from(spec_image) else {
        return ImageRole::Icon;
    };
    if is_svg_file(image_path) {
        return ImageRole::Icon;
    }
    raster_file_role(image_path.to_path_buf()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_sized_rasters_stay_icons() {
        assert_eq!(role_by_side_length(48, 48), ImageRole::Icon);
        assert_eq!(role_by_side_length(128, 128), ImageRole::Icon);
    }

    #[test]
    fn larger_rasters_are_pictures() {
        assert_eq!(role_by_side_length(1920, 1080), ImageRole::Picture);
        assert_eq!(role_by_side_length(64, 400), ImageRole::Picture);
    }

    #[tokio::test]
    async fn theme_names_are_icons() {
        let spec_image = SpecImage::ImagePath(IconReference::ThemeName("camera-photo".to_owned()));
        assert_eq!(image_role(&spec_image).await, ImageRole::Icon);
    }

    #[tokio::test]
    async fn svg_files_are_icons() {
        let spec_image = SpecImage::ImagePath(IconReference::FileUri(PathBuf::from(
            "/nonexistent/shot.svg",
        )));
        assert_eq!(image_role(&spec_image).await, ImageRole::Icon);
    }
}
