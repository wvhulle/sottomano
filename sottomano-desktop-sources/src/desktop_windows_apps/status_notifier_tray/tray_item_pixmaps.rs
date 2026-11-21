use std::{path::PathBuf, sync::Arc};

use image::{
    RgbaImage,
    imageops::{self, FilterType},
};
use system_tray::item::IconPixmap;

use crate::{
    desktop_windows_apps::status_notifier_tray::TrayItemIcon,
    theme_icons::{icon_file_loading::trim_rgba, icon_requests::ItemThemePath},
};

pub(super) const TRAY_PIXMAP_SIDE_LIMIT: u32 = 128;

fn pixmap_area(pixmap: &IconPixmap) -> i64 {
    i64::from(pixmap.width) * i64::from(pixmap.height)
}

pub(super) fn best_pixmap(pixmaps: &[IconPixmap]) -> Option<RgbaImage> {
    let valid_pixmaps = pixmaps.iter().filter(|pixmap| {
        pixmap.width > 0
            && pixmap.height > 0
            && i64::try_from(pixmap.pixels.len()).expect("a pixel buffer is shorter than i64::MAX")
                == pixmap_area(pixmap) * 4
    });
    let best = valid_pixmaps
        .clone()
        .filter(|pixmap| {
            i64::from(pixmap.width.min(pixmap.height)) >= i64::from(TRAY_PIXMAP_SIDE_LIMIT)
        })
        .min_by_key(|pixmap| pixmap_area(pixmap))
        .or_else(|| valid_pixmaps.max_by_key(|pixmap| pixmap_area(pixmap)))?;
    let (width, height) = (
        u32::try_from(best.width).expect("pixmap width was checked positive"),
        u32::try_from(best.height).expect("pixmap height was checked positive"),
    );
    let mut bytes = best.pixels.clone();
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.rotate_left(1);
    }
    RgbaImage::from_raw(width, height, bytes)
        .map(trim_rgba)
        .map(capped_to_side_limit)
}

pub(crate) fn capped_to_side_limit(image: RgbaImage) -> RgbaImage {
    let longest_side = image.width().max(image.height());
    if longest_side <= TRAY_PIXMAP_SIDE_LIMIT {
        return image;
    }
    let scaled_side = |side: u32| {
        u32::try_from(u64::from(side) * u64::from(TRAY_PIXMAP_SIDE_LIMIT) / u64::from(longest_side))
            .expect("a side scaled down to the limit fits u32")
            .max(1)
    };
    imageops::resize(
        &image,
        scaled_side(image.width()),
        scaled_side(image.height()),
        FilterType::Triangle,
    )
}

pub(super) fn tray_item_icon(
    icon_name: Option<&str>,
    icon_pixmap: Option<&[IconPixmap]>,
    icon_theme_path: Option<&str>,
) -> TrayItemIcon {
    if let Some(pixmap) = icon_pixmap.and_then(best_pixmap) {
        return TrayItemIcon::Pixmap(Arc::new(pixmap));
    }
    let Some(icon_name) = icon_name.filter(|icon_name| !icon_name.is_empty()) else {
        return TrayItemIcon::NoIcon;
    };
    TrayItemIcon::Named {
        icon_name: icon_name.to_owned(),
        item_theme_path: icon_theme_path
            .filter(|theme_path| !theme_path.is_empty())
            .map_or(ItemThemePath::Unset, |theme_path| {
                ItemThemePath::Directory(PathBuf::from(theme_path))
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque_pixmap(side: i32) -> IconPixmap {
        let side_length = usize::try_from(side).unwrap();
        IconPixmap {
            width: side,
            height: side,
            pixels: vec![0xff; side_length * side_length * 4],
        }
    }

    #[test]
    fn an_oversized_pixmap_is_scaled_down_to_the_side_limit() {
        let scaled = best_pixmap(&[opaque_pixmap(512)]).unwrap();
        assert_eq!(
            scaled.dimensions(),
            (TRAY_PIXMAP_SIDE_LIMIT, TRAY_PIXMAP_SIDE_LIMIT)
        );
    }

    #[test]
    fn the_smallest_pixmap_reaching_the_side_limit_is_chosen() {
        let chosen =
            best_pixmap(&[opaque_pixmap(512), opaque_pixmap(128), opaque_pixmap(64)]).unwrap();
        assert_eq!(chosen.dimensions(), (128, 128));
    }

    #[test]
    fn pixmaps_below_the_side_limit_keep_their_largest_size() {
        let chosen = best_pixmap(&[opaque_pixmap(22), opaque_pixmap(48)]).unwrap();
        assert_eq!(chosen.dimensions(), (48, 48));
    }
}
