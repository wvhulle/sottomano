use std::collections::HashMap;

use iced::{Rectangle, Task, window::Id};
use iced_exwlshell::reexport::{BlurOption, BlurRegion};

use crate::{app::Message, state::PanelLayout, wayland_layers::surface::input_regions};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    TopBar,
    BottomBar,
    TopStart,
    TopCenter,
    TopEnd,
    BottomStart,
    BottomMiddle,
    BottomEnd,
    Sidebar,
}

impl Zone {
    fn takes_input(self) -> bool {
        matches!(self, Self::TopBar | Self::BottomBar | Self::Sidebar)
    }

    fn is_blurred(self, layout: &PanelLayout) -> bool {
        match self {
            Self::Sidebar => true,
            Self::TopBar | Self::BottomBar => !layout.disconnected_groups,
            Self::TopStart
            | Self::TopCenter
            | Self::TopEnd
            | Self::BottomStart
            | Self::BottomMiddle
            | Self::BottomEnd => layout.disconnected_groups,
        }
    }
}

#[derive(Default)]
pub struct PanelRegions {
    rects: HashMap<(Id, Zone), Vec<Rectangle>>,
}

impl PanelRegions {
    pub fn bounds(
        &mut self,
        panel: Id,
        zone: Zone,
        rects: Vec<Rectangle>,
        layout: &PanelLayout,
    ) -> Task<Message> {
        self.rects.insert((panel, zone), rects);
        let input = zone.takes_input().then(|| self.input_of(panel));
        Task::batch(
            [Some(self.blur_of(panel, layout)), input]
                .into_iter()
                .flatten(),
        )
    }

    pub fn refresh(&self, layout: &PanelLayout, panels: impl Iterator<Item = Id>) -> Task<Message> {
        Task::batch(
            panels
                .map(|panel| self.blur_of(panel, layout))
                .collect::<Vec<_>>(),
        )
    }

    pub fn width_of(&self, panel: Id, zone: Zone) -> f32 {
        self.rects
            .get(&(panel, zone))
            .into_iter()
            .flatten()
            .map(|rect| rect.width)
            .fold(0.0, f32::max)
    }

    pub fn forget(&mut self, window: Id) {
        self.rects.retain(|(panel, _), _| *panel != window);
    }

    fn zones_of(&self, panel: Id) -> impl Iterator<Item = (Zone, &Vec<Rectangle>)> {
        self.rects
            .iter()
            .filter(move |((owner, _), _)| *owner == panel)
            .map(|((_, zone), rects)| (*zone, rects))
    }

    fn input_of(&self, panel: Id) -> Task<Message> {
        let rects = self
            .zones_of(panel)
            .filter(|(zone, _)| zone.takes_input())
            .flat_map(|(_, rects)| rects.iter().copied())
            .collect();
        Task::done(input_regions(panel, rects))
    }

    fn blur_of(&self, panel: Id, layout: &PanelLayout) -> Task<Message> {
        let region = self
            .zones_of(panel)
            .filter(|(zone, _)| zone.is_blurred(layout))
            .flat_map(|(_, rects)| rects)
            .flat_map(|rect| rounded(*rect, layout.corner_radius.top_left))
            .collect::<Vec<BlurRegion>>();
        tracing::warn!(
            target: "vetrata::blurdbg",
            ?panel,
            zones = ?self.zones_of(panel).map(|(zone, rects)| (zone, rects.len(), zone.is_blurred(layout))).collect::<Vec<_>>(),
            region_rows = region.len(),
            "blur region sent"
        );
        Task::done(Message::BlurOptionChange {
            id: panel,
            option: BlurOption::Region(region),
        })
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
