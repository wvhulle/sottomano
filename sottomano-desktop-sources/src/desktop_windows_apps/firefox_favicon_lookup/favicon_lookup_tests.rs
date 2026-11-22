use super::{FaviconKind, classify};

#[test]
fn sniffs_svg_past_bom_and_whitespace() {
    for blob in [
        b"<svg xmlns=...".as_slice(),
        b"<?xml version=\"1.0\"?><svg/>",
        b"<!-- comment --><svg/>",
        b"\n<svg/>",
        &[0xEF, 0xBB, 0xBF, b'<', b's', b'v', b'g'],
        &[0xEF, 0xBB, 0xBF, b'\n', b'<'],
        &[0x1f, 0x8b, 0x08],
    ] {
        assert_eq!(classify(blob), FaviconKind::Svg);
    }
}

#[test]
fn sniffs_raster() {
    assert_eq!(classify(&[0x89, 0x50, 0x4E, 0x47]), FaviconKind::Raster);
    assert_eq!(
        classify(&[0x00, 0x00, 0x01, 0x00, 0x10]),
        FaviconKind::Raster
    );
}
