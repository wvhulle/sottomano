use std::{path::Path, sync::Arc};

use image::RgbaImage;

use crate::theme_icons::icon_file_matching::SYMBOLIC_ICON_SUFFIX;

#[derive(Debug, Clone, PartialEq)]
pub enum IconDrawing {
    Raster(Arc<RgbaImage>),
    ColouredSvg(Arc<[u8]>),
    MonochromeSvg(Arc<[u8]>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgInk {
    Monochrome,
    Coloured,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum NodePaint {
    Solid(usvg::Color),
    Shaded,
}

#[must_use]
pub fn classified_svg(
    icon_path: &Path,
    svg_tree: &usvg::Tree,
    svg_bytes: Arc<[u8]>,
) -> IconDrawing {
    match svg_ink(icon_path, svg_tree) {
        SvgInk::Monochrome => IconDrawing::MonochromeSvg(svg_bytes),
        SvgInk::Coloured => IconDrawing::ColouredSvg(svg_bytes),
    }
}

#[must_use]
pub fn svg_ink(icon_path: &Path, svg_tree: &usvg::Tree) -> SvgInk {
    let symbolic_file_name = icon_path
        .file_stem()
        .and_then(|file_stem| file_stem.to_str())
        .is_some_and(|file_stem| file_stem.ends_with(SYMBOLIC_ICON_SUFFIX));
    if symbolic_file_name {
        return SvgInk::Monochrome;
    }
    let paints = group_paints(svg_tree.root());
    match paints.split_first() {
        Some((NodePaint::Solid(first_colour), other_paints))
            if other_paints
                .iter()
                .all(|other_paint| *other_paint == NodePaint::Solid(*first_colour)) =>
        {
            SvgInk::Monochrome
        }
        Some(_) | None => SvgInk::Coloured,
    }
}

fn group_paints(group: &usvg::Group) -> Vec<NodePaint> {
    group
        .children()
        .iter()
        .flat_map(|node| match node {
            usvg::Node::Group(child_group) => group_paints(child_group),
            usvg::Node::Text(text) => group_paints(text.flattened()),
            usvg::Node::Image(_) => vec![NodePaint::Shaded],
            usvg::Node::Path(path) => [
                path.fill().map(usvg::Fill::paint),
                path.stroke().map(usvg::Stroke::paint),
            ]
            .into_iter()
            .flatten()
            .map(node_paint)
            .collect(),
        })
        .collect()
}

fn node_paint(paint: &usvg::Paint) -> NodePaint {
    match paint {
        usvg::Paint::Color(colour) => NodePaint::Solid(*colour),
        usvg::Paint::LinearGradient(_)
        | usvg::Paint::RadialGradient(_)
        | usvg::Paint::Pattern(_) => NodePaint::Shaded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KDE_STYLESHEET_ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><defs><style type="text/css" id="current-color-scheme">.ColorScheme-Text { color:#dedede; }</style></defs><path style="fill:currentColor" class="ColorScheme-Text" d="M2 2h12v12H2z"/><path style="fill:currentColor" class="ColorScheme-Text" d="M4 4h2v2H4z"/></svg>"#;
    const TWO_TONE_ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="#dedede" d="M2 2h12v12H2z"/><path fill="#3daee9" d="M4 4h2v2H4z"/></svg>"##;
    const EMPTY_ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"/>"#;

    fn parsed_svg(svg_text: &str) -> usvg::Tree {
        usvg::Tree::from_data(svg_text.as_bytes(), &usvg::Options::default())
            .expect("test svg parses")
    }

    #[test]
    fn a_symbolic_file_name_is_monochrome_whatever_it_draws() {
        assert_eq!(
            svg_ink(
                Path::new("/icons/bindrune-symbolic.svg"),
                &parsed_svg(TWO_TONE_ICON)
            ),
            SvgInk::Monochrome
        );
    }

    #[test]
    fn a_kde_stylesheet_icon_in_one_colour_is_monochrome() {
        assert_eq!(
            svg_ink(
                Path::new("/icons/edit-select.svg"),
                &parsed_svg(KDE_STYLESHEET_ICON)
            ),
            SvgInk::Monochrome
        );
    }

    #[test]
    fn two_tone_and_empty_icons_keep_their_colours() {
        assert_eq!(
            svg_ink(
                Path::new("/icons/document-sign.svg"),
                &parsed_svg(TWO_TONE_ICON)
            ),
            SvgInk::Coloured
        );
        assert_eq!(
            svg_ink(Path::new("/icons/blank.svg"), &parsed_svg(EMPTY_ICON)),
            SvgInk::Coloured
        );
    }
}
