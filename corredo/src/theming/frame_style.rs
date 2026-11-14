use iced::{
    Border, Color, Padding, Rectangle, Shadow, Theme, Vector, border::Radius, widget::container,
};

use crate::theming::{
    background_color, background_strong_color, background_weak_color,
    style_metrics::OUTLINE_WIDTH_PX, text_color,
};

const SHADOW_DROP_PER_SOFTNESS: f32 = 0.25;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FrameBackground {
    #[default]
    Base,
    Weak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowCasting {
    Cast,
    Omitted,
}

impl ShadowCasting {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            ShadowCasting::Cast => ShadowCasting::Omitted,
            ShadowCasting::Omitted => ShadowCasting::Cast,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowStyle {
    pub casting: ShadowCasting,
    pub softness_px: f32,
    pub opacity: f32,
}

impl Default for ShadowStyle {
    fn default() -> Self {
        Self {
            casting: ShadowCasting::Cast,
            softness_px: 20.0,
            opacity: 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineTracing {
    Drawn,
    Omitted,
}

impl OutlineTracing {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            OutlineTracing::Drawn => OutlineTracing::Omitted,
            OutlineTracing::Omitted => OutlineTracing::Drawn,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineStyle {
    pub tracing: OutlineTracing,
    pub opacity: f32,
}

impl Default for OutlineStyle {
    fn default() -> Self {
        Self {
            tracing: OutlineTracing::Drawn,
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameStyle {
    pub background: FrameBackground,
    pub opacity: f32,
    pub corner_radius_px: f32,
    pub shadow: ShadowStyle,
    pub outline: OutlineStyle,
    pub outline_width_px: f32,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            background: FrameBackground::Base,
            opacity: 0.8,
            corner_radius_px: 8.0,
            shadow: ShadowStyle::default(),
            outline: OutlineStyle::default(),
            outline_width_px: OUTLINE_WIDTH_PX,
        }
    }
}

impl FrameStyle {
    #[must_use]
    pub fn with_corner_radius(mut self, corner_radius_px: f32) -> Self {
        self.corner_radius_px = corner_radius_px;
        self
    }

    #[must_use]
    pub fn corner_radius(self) -> Radius {
        Radius::from(self.corner_radius_px)
    }

    #[must_use]
    pub fn drawn_corners(self, outlined_sides: OutlinedSides) -> Radius {
        outlined_sides.square_unoutlined_corners(self.corner_radius())
    }

    #[must_use]
    pub fn container_style(self, theme: &Theme) -> container::Style {
        container::Style::default()
            .background(self.fill_color(theme))
            .border(Border {
                radius: self.corner_radius(),
                ..self.outline_border(theme)
            })
            .shadow(self.drop_shadow())
            .color(text_color(theme))
    }

    #[must_use]
    pub fn fill_color(self, theme: &Theme) -> Color {
        let background = match self.background {
            FrameBackground::Base => background_color(theme),
            FrameBackground::Weak => background_weak_color(theme),
        };
        background.scale_alpha(self.opacity)
    }

    #[must_use]
    pub fn outline_border(self, theme: &Theme) -> Border {
        match self.outline.tracing {
            OutlineTracing::Drawn => Border {
                color: background_strong_color(theme).scale_alpha(self.outline.opacity),
                width: self.outline_width_px,
                radius: Radius::default(),
            },
            OutlineTracing::Omitted => Border::default(),
        }
    }

    #[must_use]
    pub fn outline_width(self) -> f32 {
        match self.outline.tracing {
            OutlineTracing::Drawn => self.outline_width_px,
            OutlineTracing::Omitted => 0.0,
        }
    }

    #[must_use]
    pub fn drop_shadow(self) -> Shadow {
        match self.shadow.casting {
            ShadowCasting::Cast => Shadow {
                color: Color::BLACK.scale_alpha(self.shadow.opacity),
                offset: Vector::ZERO,
                blur_radius: self.shadow.softness_px,
            },
            ShadowCasting::Omitted => Shadow::default(),
        }
    }

    #[must_use]
    pub fn lifted_drop_shadow(self) -> Shadow {
        let centered_shadow = self.drop_shadow();
        Shadow {
            offset: Vector::new(0.0, centered_shadow.blur_radius * SHADOW_DROP_PER_SOFTNESS),
            ..centered_shadow
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct OutlinedSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl OutlinedSides {
    pub const ALL: Self = Self {
        top: true,
        right: true,
        bottom: true,
        left: true,
    };

    #[must_use]
    pub fn all_when_floating(self, float_offset_px: f32) -> Self {
        if float_offset_px > 0.0 {
            Self::ALL
        } else {
            self
        }
    }

    fn square_unoutlined_corners(self, radius: Radius) -> Radius {
        let rounded_between = |first: bool, second: bool, corner: f32| {
            if first && second { corner } else { 0.0 }
        };
        Radius {
            top_left: rounded_between(self.top, self.left, radius.top_left),
            top_right: rounded_between(self.top, self.right, radius.top_right),
            bottom_right: rounded_between(self.bottom, self.right, radius.bottom_right),
            bottom_left: rounded_between(self.bottom, self.left, radius.bottom_left),
        }
    }

    #[must_use]
    pub fn push_unoutlined_past(self, bounds: Rectangle, outline_width: f32) -> Rectangle {
        let beyond_antialiasing = |outlined: bool| if outlined { 0.0 } else { 2.0 * outline_width };
        bounds.expand(Padding {
            top: beyond_antialiasing(self.top),
            right: beyond_antialiasing(self.right),
            bottom: beyond_antialiasing(self.bottom),
            left: beyond_antialiasing(self.left),
        })
    }
}
