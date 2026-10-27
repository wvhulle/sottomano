use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AlignedQuantity {
    Opacity,
    CornerRadius,
    Blur,
    ShadowCast,
    ShadowSoftness,
    ShadowOpacity,
    Outline,
    OutlineOpacity,
}

impl fmt::Display for AlignedQuantity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            AlignedQuantity::Opacity => "Opacity",
            AlignedQuantity::CornerRadius => "Corner radius",
            AlignedQuantity::Blur => "Blur",
            AlignedQuantity::ShadowCast => "Shadow",
            AlignedQuantity::ShadowSoftness => "Shadow softness",
            AlignedQuantity::ShadowOpacity => "Shadow opacity",
            AlignedQuantity::Outline => "Outline",
            AlignedQuantity::OutlineOpacity => "Outline opacity",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemAlignment {
    #[default]
    Unaligned,
    Aligned(AlignedQuantity),
}
