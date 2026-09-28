use std::{fs, path::Path};

const INTRINSIC_HEIGHT_EXCEEDING_ANY_ICON_SIZE: f32 = 1000.0;

const PHOSPHOR_CLOUD_RAIN_HEIGHT_FRACTION: f32 = 232.0 / 256.0;

pub fn fit_to_phosphor_cloud_rain(source_dir: &Path, fitted_dir: &Path) {
    fs::create_dir_all(fitted_dir).unwrap();
    fs::read_dir(source_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .for_each(|path| {
            fs::write(
                fitted_dir.join(path.file_name().unwrap()),
                fitted_drawing(&fs::read(&path).unwrap()),
            )
            .unwrap();
        });
}

fn fitted_drawing(svg: &[u8]) -> String {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).unwrap();
    let drawing = tree.root().abs_bounding_box();
    let normalized = tree.to_string(&usvg::WriteOptions::default());
    let (_, body) = normalized.split_once('>').unwrap();
    let canvas_height = drawing.height() / PHOSPHOR_CLOUD_RAIN_HEIGHT_FRACTION;
    let canvas_top = drawing.y() - (canvas_height - drawing.height()) / 2.0;
    let x = drawing.x();
    let width = drawing.width();
    let intrinsic_height = INTRINSIC_HEIGHT_EXCEEDING_ANY_ICON_SIZE;
    let intrinsic_width = intrinsic_height * width / canvas_height;
    format!(
        r#"<svg width="{intrinsic_width}" height="{intrinsic_height}" viewBox="{x} {canvas_top} {width} {canvas_height}" xmlns="http://www.w3.org/2000/svg">{body}"#
    )
}
