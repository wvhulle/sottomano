use std::collections::{BTreeMap, HashMap};

use iced::{Rectangle, Task, window::Id};
use iced_exwlshell::reexport::{BlurOption, BlurRegion};

use crate::{app::Message, state::PanelLayout, wayland_layers::surface::Surface};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    TopStart,
    TopCenter,
    TopEnd,
    BottomStart,
    BottomMiddle,
    BottomEnd,
    Sidebar,
}

#[derive(Default)]
pub struct PanelPills {
    rects: HashMap<(Id, Zone), Vec<Rectangle>>,
}

impl PanelPills {
    pub fn pill_rects(
        &mut self,
        strip: Id,
        zone: Zone,
        rects: &[Rectangle],
        layout: &PanelLayout,
    ) -> Task<Message> {
        self.rects.insert((strip, zone), rects.to_vec());
        self.blur_of(strip, layout)
    }

    pub fn refresh(&self, layout: &PanelLayout, bars: &BTreeMap<Id, Surface>) -> Task<Message> {
        Task::batch(
            bars.iter()
                .filter(|(_, surface)| !surface.position.is_vertical())
                .map(|(&strip, _)| self.blur_of(strip, layout))
                .collect::<Vec<_>>(),
        )
    }

    pub fn forget(&mut self, window: Id) {
        self.rects.retain(|(strip, _), _| *strip != window);
    }

    fn blur_of(&self, strip: Id, layout: &PanelLayout) -> Task<Message> {
        let option = if layout.disconnected_groups {
            BlurOption::Region(
                self.rects
                    .iter()
                    .filter(|((owner, _), _)| *owner == strip)
                    .flat_map(|(_, rects)| rects)
                    .flat_map(|rect| rounded(*rect, layout.corner_radius.top_left))
                    .collect(),
            )
        } else {
            BlurOption::None
        };
        Task::done(Message::BlurOptionChange { id: strip, option })
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn rounded(rect: Rectangle, radius: f32) -> Vec<BlurRegion> {
    let x = rect.x.round() as i32;
    let y = rect.y.round() as i32;
    let width = rect.width.ceil() as i32;
    let height = rect.height.ceil() as i32;
    let radius = (radius.round() as i32).clamp(0, width.min(height) / 2);
    let row = |top: i32, inset: i32| BlurRegion {
        x: x + inset,
        y: top,
        width: width - 2 * inset,
        height: 1,
    };
    let corner_rows = (0..radius).flat_map(|step| {
        let from_edge = (radius - step) as f32 - 0.5;
        let reach = (radius as f32)
            .mul_add(radius as f32, -from_edge * from_edge)
            .sqrt();
        let inset = (radius as f32 - reach).round() as i32;
        [row(y + step, inset), row(y + height - 1 - step, inset)]
    });
    corner_rows
        .chain([BlurRegion {
            x,
            y: y + radius,
            width,
            height: height - 2 * radius,
        }])
        .filter(|row| row.width > 0 && row.height > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use iced::Rectangle;

    use super::rounded;

    fn pill() -> Rectangle {
        Rectangle {
            x: 10.0,
            y: 0.0,
            width: 60.0,
            height: 24.0,
        }
    }

    #[test]
    fn a_square_pill_is_one_rectangle() {
        let region = rounded(pill(), 0.0);
        assert_eq!(region.len(), 1);
        assert_eq!(
            (region[0].x, region[0].width, region[0].height),
            (10, 60, 24)
        );
    }

    #[test]
    fn rounded_corners_step_inwards_towards_the_edge() {
        let region = rounded(pill(), 8.0);
        let top_row = region.iter().find(|row| row.y == 0).expect("top row");
        let middle = region.iter().find(|row| row.height > 1).expect("middle");
        assert!(top_row.x > 10 && top_row.width < 60);
        assert_eq!((middle.y, middle.height, middle.width), (8, 8, 60));
    }

    #[test]
    fn the_radius_never_exceeds_half_the_pill() {
        let region = rounded(pill(), 100.0);
        assert!(region.iter().all(|row| row.width > 0 && row.height > 0));
    }
}
