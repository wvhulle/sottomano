use std::{path::Path, sync::Arc};

use image::RgbaImage;

use crate::desktop_windows_apps::{
    application_menu_nodes::SYMBOLIC_ICON_SUFFIX,
    icon_whitespace_trim::{IconFile, IconTrim, IconTrimError, read_icon_file},
};

#[derive(Debug, Clone, PartialEq)]
pub enum IconSource {
    Pixmap(Arc<RgbaImage>),
    Themed(IconFile),
    ThemedSymbolic(Arc<[u8]>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SvgInk {
    Monochrome,
    Coloured,
}

impl IconSource {
    pub(crate) fn pixmap(&self) -> Option<&Arc<RgbaImage>> {
        match self {
            Self::Pixmap(pixmap) | Self::Themed(IconFile::Raster(pixmap)) => Some(pixmap),
            Self::Themed(IconFile::Svg(_)) | Self::ThemedSymbolic(_) => None,
        }
    }

    pub fn design_side_px(&self) -> Result<f32, usvg::Error> {
        match self {
            Self::Pixmap(pixels) | Self::Themed(IconFile::Raster(pixels)) => {
                Ok(pixels.width().max(pixels.height()) as f32)
            }
            Self::Themed(IconFile::Svg(svg_bytes)) | Self::ThemedSymbolic(svg_bytes) => {
                let svg_size = usvg::Tree::from_data(svg_bytes, &usvg::Options::default())?.size();
                Ok(svg_size.width().max(svg_size.height()))
            }
        }
    }

    pub(crate) fn logged_theme_icon(theme_file: &Path, icon_trim: IconTrim) -> Option<Self> {
        theme_icon_source(theme_file, icon_trim)
            .inspect_err(|load_failure| tracing::warn!("theme icon: {load_failure}"))
            .ok()
    }

    fn classified_theme_icon(theme_file: &Path, icon_file: IconFile) -> Self {
        let is_symbolic_file_name = theme_file
            .file_stem()
            .and_then(|file_stem| file_stem.to_str())
            .is_some_and(|file_stem| file_stem.ends_with(SYMBOLIC_ICON_SUFFIX));
        match icon_file {
            IconFile::Svg(svg_bytes)
                if is_symbolic_file_name || svg_ink(&svg_bytes) == SvgInk::Monochrome =>
            {
                Self::ThemedSymbolic(svg_bytes)
            }
            full_colour_icon => Self::Themed(full_colour_icon),
        }
    }
}

pub fn theme_icon_source(
    theme_file: &Path,
    icon_trim: IconTrim,
) -> Result<IconSource, IconTrimError> {
    read_icon_file(theme_file, icon_trim)
        .map(|icon_file| IconSource::classified_theme_icon(theme_file, icon_file))
}

fn svg_ink(svg_bytes: &[u8]) -> SvgInk {
    let Ok(svg_tree) = usvg::Tree::from_data(svg_bytes, &usvg::Options::default()) else {
        return SvgInk::Coloured;
    };
    let mut paints = Vec::new();
    match collect_group_paints(svg_tree.root(), &mut paints) {
        SvgInk::Coloured => SvgInk::Coloured,
        SvgInk::Monochrome => match paints.split_first() {
            Some((first_colour, other_colours))
                if other_colours.iter().all(|colour| colour == first_colour) =>
            {
                SvgInk::Monochrome
            }
            Some(_) | None => SvgInk::Coloured,
        },
    }
}

fn collect_group_paints(group: &usvg::Group, paints: &mut Vec<usvg::Color>) -> SvgInk {
    for node in group.children() {
        let node_ink = match node {
            usvg::Node::Group(child_group) => collect_group_paints(child_group, paints),
            usvg::Node::Text(text) => collect_group_paints(text.flattened(), paints),
            usvg::Node::Image(_) => SvgInk::Coloured,
            usvg::Node::Path(path) => {
                let fill_paint = path.fill().map(usvg::Fill::paint);
                let stroke_paint = path.stroke().map(usvg::Stroke::paint);
                [fill_paint, stroke_paint]
                    .into_iter()
                    .flatten()
                    .map(|paint| match paint {
                        usvg::Paint::Color(colour) => {
                            paints.push(*colour);
                            SvgInk::Monochrome
                        }
                        usvg::Paint::LinearGradient(_)
                        | usvg::Paint::RadialGradient(_)
                        | usvg::Paint::Pattern(_) => SvgInk::Coloured,
                    })
                    .fold(SvgInk::Monochrome, |path_ink, paint_ink| {
                        match (path_ink, paint_ink) {
                            (SvgInk::Monochrome, SvgInk::Monochrome) => SvgInk::Monochrome,
                            (SvgInk::Coloured, _) | (_, SvgInk::Coloured) => SvgInk::Coloured,
                        }
                    })
            }
        };
        if node_ink == SvgInk::Coloured {
            return SvgInk::Coloured;
        }
    }
    SvgInk::Monochrome
}

#[cfg(test)]
mod tests {
    use super::*;

    const KDE_STYLESHEET_ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><defs><style type="text/css" id="current-color-scheme">.ColorScheme-Text { color:#dedede; }</style></defs><path style="fill:currentColor" class="ColorScheme-Text" d="M2 2h12v12H2z"/><path style="fill:currentColor" class="ColorScheme-Text" d="M4 4h2v2H4z"/></svg>"#;
    const TWO_TONE_ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="#dedede" d="M2 2h12v12H2z"/><path fill="#3daee9" d="M4 4h2v2H4z"/></svg>"##;

    fn placeholder_svg_bytes() -> Arc<[u8]> {
        Arc::from(&b"<svg/>"[..])
    }

    #[test]
    fn symbolic_svg_is_tinted_by_the_panel() {
        let theme_file = Path::new("/absent/icons/page.codeberg.wvhulle.Bindrune-symbolic.svg");
        assert_eq!(
            IconSource::classified_theme_icon(theme_file, IconFile::Svg(placeholder_svg_bytes())),
            IconSource::ThemedSymbolic(placeholder_svg_bytes())
        );
    }

    #[test]
    fn full_colour_svg_keeps_its_colours() {
        let theme_file = Path::new("/absent/icons/page.codeberg.wvhulle.Bindrune.svg");
        assert_eq!(
            IconSource::classified_theme_icon(theme_file, IconFile::Svg(placeholder_svg_bytes())),
            IconSource::Themed(IconFile::Svg(placeholder_svg_bytes()))
        );
    }

    #[test]
    fn symbolic_png_is_not_loaded_as_svg() {
        let theme_file = Path::new("/absent/icons/network-wireless-symbolic.png");
        let decoded_pixels = Arc::new(RgbaImage::new(4, 4));
        assert_eq!(
            IconSource::classified_theme_icon(theme_file, IconFile::Raster(decoded_pixels.clone())),
            IconSource::Themed(IconFile::Raster(decoded_pixels))
        );
    }

    #[test]
    fn kde_stylesheet_icon_in_one_colour_is_tinted_by_the_panel() {
        let stylesheet_bytes: Arc<[u8]> = Arc::from(KDE_STYLESHEET_ICON.as_bytes());
        assert_eq!(
            IconSource::classified_theme_icon(
                Path::new("/absent/icons/edit-select.svg"),
                IconFile::Svg(stylesheet_bytes.clone())
            ),
            IconSource::ThemedSymbolic(stylesheet_bytes)
        );
    }

    #[test]
    fn two_tone_icon_keeps_its_colours() {
        let two_tone_bytes: Arc<[u8]> = Arc::from(TWO_TONE_ICON.as_bytes());
        assert_eq!(
            IconSource::classified_theme_icon(
                Path::new("/absent/icons/document-sign.svg"),
                IconFile::Svg(two_tone_bytes.clone())
            ),
            IconSource::Themed(IconFile::Svg(two_tone_bytes))
        );
    }
}
