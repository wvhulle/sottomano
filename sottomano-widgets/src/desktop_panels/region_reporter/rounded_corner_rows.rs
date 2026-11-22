use iced::{Rectangle, border::Radius};
use iced_exwlshell::reexport::BlurRegion;

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn corner_inset(radius: i32, step: i32) -> i32 {
    if step >= radius {
        return 0;
    }
    let from_edge = (radius - step) as f32 - 0.5;
    let reach = (radius as f32)
        .mul_add(radius as f32, -from_edge * from_edge)
        .sqrt();
    (radius as f32 - reach).ceil() as i32
}

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn rounded_blur_rows(rect: Rectangle, radius: Radius) -> Vec<BlurRegion> {
    let left_edge = rect.x.ceil() as i32;
    let top_edge = rect.y.ceil() as i32;
    let width = (rect.x + rect.width).floor() as i32 - left_edge;
    let height = (rect.y + rect.height).floor() as i32 - top_edge;
    if width <= 0 || height <= 0 {
        return Vec::new();
    }
    let fitted = |corner: f32| (corner.round() as i32).clamp(0, width.min(height) / 2);
    let top_left = fitted(radius.top_left);
    let top_right = fitted(radius.top_right);
    let bottom_left = fitted(radius.bottom_left);
    let bottom_right = fitted(radius.bottom_right);
    let row = |top: i32, left: i32, right: i32| BlurRegion {
        x: left_edge + left,
        y: top,
        width: width - left - right,
        height: 1,
    };
    let top_rows = top_left.max(top_right);
    let bottom_rows = bottom_left.max(bottom_right);
    let top_corner_rows = (0..top_rows).map(|step| {
        row(
            top_edge + step,
            corner_inset(top_left, step),
            corner_inset(top_right, step),
        )
    });
    let bottom_corner_rows = (0..bottom_rows).map(|step| {
        row(
            top_edge + height - 1 - step,
            corner_inset(bottom_left, step),
            corner_inset(bottom_right, step),
        )
    });
    top_corner_rows
        .chain(bottom_corner_rows)
        .chain([BlurRegion {
            x: left_edge,
            y: top_edge + top_rows,
            width,
            height: height - top_rows - bottom_rows,
        }])
        .filter(|row| row.width > 0 && row.height > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use iced::{Rectangle, border::Radius};

    use super::rounded_blur_rows;

    fn indicator_group_frame() -> Rectangle {
        Rectangle {
            x: 10.0,
            y: 0.0,
            width: 60.0,
            height: 24.0,
        }
    }

    #[test]
    fn a_frame_thinner_than_a_pixel_blurs_nothing() {
        let collapsed = Rectangle {
            x: 10.4,
            y: 0.0,
            width: 0.3,
            height: 24.0,
        };
        assert!(rounded_blur_rows(collapsed, Radius::from(8.0)).is_empty());
    }

    #[test]
    fn a_square_frame_blurs_as_one_rectangle() {
        let region = rounded_blur_rows(indicator_group_frame(), Radius::from(0.0));
        assert_eq!(region.len(), 1);
        assert_eq!(
            (region[0].x, region[0].width, region[0].height),
            (10, 60, 24)
        );
    }

    #[test]
    fn rounded_corners_step_inwards_towards_the_edge() {
        let region = rounded_blur_rows(indicator_group_frame(), Radius::from(8.0));
        let top_row = region.iter().find(|row| row.y == 0).expect("top row");
        let middle = region.iter().find(|row| row.height > 1).expect("middle");
        assert!(top_row.x > 10 && top_row.width < 60);
        assert_eq!((middle.y, middle.height, middle.width), (8, 8, 60));
    }

    #[test]
    fn the_radius_never_exceeds_half_the_frame() {
        let region = rounded_blur_rows(indicator_group_frame(), Radius::from(100.0));
        assert!(region.iter().all(|row| row.width > 0 && row.height > 0));
    }

    #[test]
    fn square_top_corners_blur_the_full_top_row() {
        let bottom_only = Radius {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 8.0,
            bottom_left: 8.0,
        };
        let region = rounded_blur_rows(indicator_group_frame(), bottom_only);
        let top_row = region.iter().find(|row| row.y == 0).expect("top row");
        let bottom_row = region.iter().find(|row| row.y == 23).expect("bottom row");
        assert_eq!((top_row.x, top_row.width), (10, 60));
        assert!(bottom_row.x > 10 && bottom_row.width < 60);
    }

    #[test]
    fn each_side_steps_in_by_its_own_corner() {
        let left_only = Radius {
            top_left: 8.0,
            top_right: 0.0,
            bottom_right: 0.0,
            bottom_left: 8.0,
        };
        let region = rounded_blur_rows(indicator_group_frame(), left_only);
        let top_row = region.iter().find(|row| row.y == 0).expect("top row");
        assert!(top_row.x > 10);
        assert_eq!(top_row.x + top_row.width, 70);
    }
}
