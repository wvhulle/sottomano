use core::f32::consts::{FRAC_PI_2, PI};

use iced::{Point, Rectangle, Vector, border::Radius};

use super::{ARC_SAMPLE_STEP_PX, COINCIDENT_POINT_PX, CURVE_SAMPLE_COUNT, STRAIGHT_SAMPLE_STEP_PX};

pub(super) fn toward(from: Point, to: Point, fraction: f32) -> Point {
    from + (to - from) * fraction
}

pub(super) fn dot_product(first: Vector, second: Vector) -> f32 {
    first.x.mul_add(second.x, first.y * second.y)
}

pub(super) fn unit_direction(from: Point, to: Point) -> Vector {
    let along = to - from;
    along * dot_product(along, along).sqrt().max(f32::EPSILON).recip()
}

#[allow(clippy::cast_precision_loss)]
fn curve_fractions() -> impl Iterator<Item = f32> {
    (1..=CURVE_SAMPLE_COUNT).map(|sample| sample as f32 / CURVE_SAMPLE_COUNT as f32)
}

pub(super) fn quadratic_samples(
    start: Point,
    control: Point,
    end: Point,
) -> impl Iterator<Item = Point> {
    curve_fractions().map(move |fraction| {
        toward(
            toward(start, control, fraction),
            toward(control, end, fraction),
            fraction,
        )
    })
}

pub(super) fn cubic_samples(
    start: Point,
    first_control: Point,
    second_control: Point,
    end: Point,
) -> impl Iterator<Item = Point> {
    curve_fractions().map(move |fraction| {
        toward(
            toward(
                toward(start, first_control, fraction),
                toward(first_control, second_control, fraction),
                fraction,
            ),
            toward(
                toward(first_control, second_control, fraction),
                toward(second_control, end, fraction),
                fraction,
            ),
            fraction,
        )
    })
}

struct CornerTurn {
    center: Point,
    radius: f32,
    start_angle: f32,
}

impl CornerTurn {
    fn point_at(&self, angle: f32) -> Point {
        self.center + Vector::new(angle.cos(), angle.sin()) * self.radius
    }

    fn first_point(&self) -> Point {
        self.point_at(self.start_angle)
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn samples(&self) -> impl Iterator<Item = Point> {
        let step_count = ((self.radius * FRAC_PI_2 / ARC_SAMPLE_STEP_PX).ceil() as usize).max(1);
        (0..=step_count).map(move |step| {
            self.point_at(self.start_angle + FRAC_PI_2 * step as f32 / step_count as f32)
        })
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn straight_samples(start: Point, end: Point) -> impl Iterator<Item = Point> {
    let step_count = (start.distance(end) / STRAIGHT_SAMPLE_STEP_PX).ceil() as usize;
    (1..step_count).map(move |step| toward(start, end, step as f32 / step_count as f32))
}

pub(super) struct PerimeterPosition {
    pub(super) position: Point,
    pub(super) tangent: Vector,
}

pub(super) struct RoundedPerimeter {
    points: Vec<Point>,
    distances: Vec<f32>,
    pub(super) total_length: f32,
}

impl RoundedPerimeter {
    pub(super) fn around(panel: Rectangle, corners: Radius) -> Option<Self> {
        let fitted = |radius: f32| radius.clamp(0.0, panel.width.min(panel.height) / 2.0);
        let top_right = fitted(corners.top_right);
        let bottom_right = fitted(corners.bottom_right);
        let bottom_left = fitted(corners.bottom_left);
        let top_left = fitted(corners.top_left);
        let right = panel.x + panel.width;
        let bottom = panel.y + panel.height;
        let turns = [
            CornerTurn {
                center: Point::new(right - top_right, panel.y + top_right),
                radius: top_right,
                start_angle: -FRAC_PI_2,
            },
            CornerTurn {
                center: Point::new(right - bottom_right, bottom - bottom_right),
                radius: bottom_right,
                start_angle: 0.0,
            },
            CornerTurn {
                center: Point::new(panel.x + bottom_left, bottom - bottom_left),
                radius: bottom_left,
                start_angle: FRAC_PI_2,
            },
            CornerTurn {
                center: Point::new(panel.x + top_left, panel.y + top_left),
                radius: top_left,
                start_angle: PI,
            },
        ];
        let next_turns = turns.iter().cycle().skip(1);
        let sampled = turns.iter().zip(next_turns).flat_map(|(turn, next_turn)| {
            let leaving = turn.point_at(turn.start_angle + FRAC_PI_2);
            turn.samples()
                .chain(straight_samples(leaving, next_turn.first_point()))
        });
        let mut points: Vec<Point> = Vec::new();
        for point in sampled {
            if points
                .last()
                .is_none_or(|last| last.distance(point) > COINCIDENT_POINT_PX)
            {
                points.push(point);
            }
        }
        if points.len() < 2 {
            return None;
        }
        let distances = points
            .iter()
            .scan((0.0, points[0]), |(walked, previous), point| {
                *walked += previous.distance(*point);
                *previous = *point;
                Some(*walked)
            })
            .collect::<Vec<f32>>();
        let total_length =
            distances[distances.len() - 1] + points[points.len() - 1].distance(points[0]);
        Some(Self {
            points,
            distances,
            total_length,
        })
    }

    fn point_after(&self, index: usize) -> Point {
        self.points[(index + 1) % self.points.len()]
    }

    pub(super) fn at(&self, distance: f32) -> PerimeterPosition {
        let wrapped = distance.rem_euclid(self.total_length);
        let index = self
            .distances
            .partition_point(|walked| *walked <= wrapped)
            .checked_sub(1)
            .expect("perimeter distances start at zero, at or before any wrapped distance");
        let start = self.points[index];
        let end = self.point_after(index);
        let tangent = unit_direction(start, end);
        PerimeterPosition {
            position: start + tangent * (wrapped - self.distances[index]),
            tangent,
        }
    }

    pub(super) fn nearest_distance(&self, target: Point) -> f32 {
        let (_, nearest) = self.points.iter().enumerate().fold(
            (f32::INFINITY, 0.0),
            |(best_gap, best_distance), (index, start)| {
                let end = self.point_after(index);
                let tangent = unit_direction(*start, end);
                let along = dot_product(target - *start, tangent).clamp(0.0, start.distance(end));
                let gap = target.distance(*start + tangent * along);
                if gap < best_gap {
                    (gap, self.distances[index] + along)
                } else {
                    (best_gap, best_distance)
                }
            },
        );
        nearest
    }

    pub(super) fn points_between(&self, from: f32, to: f32) -> impl Iterator<Item = Point> {
        let sweep = (to - from).rem_euclid(self.total_length);
        let first = self
            .distances
            .partition_point(|walked| *walked <= from.rem_euclid(self.total_length));
        (0..self.points.len())
            .map(move |offset| (first + offset) % self.points.len())
            .map_while(move |index| {
                let ahead = (self.distances[index] - from).rem_euclid(self.total_length);
                (ahead < sweep).then(|| self.points[index])
            })
    }
}
