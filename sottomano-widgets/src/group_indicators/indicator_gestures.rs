use crate::widget_groups::widget_group_interaction::WidgetGroupInteraction;

#[derive(Debug, Clone)]
pub enum IndicatorGesture {
    Inert,
    Performs {
        description: &'static str,
        message: WidgetGroupInteraction,
    },
}

impl IndicatorGesture {
    #[must_use]
    pub const fn description(&self) -> Option<&'static str> {
        match self {
            IndicatorGesture::Inert => None,
            IndicatorGesture::Performs { description, .. } => Some(*description),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndicatorGestures {
    pub left: IndicatorGesture,
    pub right: IndicatorGesture,
}

impl IndicatorGestures {
    #[must_use]
    pub const fn inert() -> Self {
        Self {
            left: IndicatorGesture::Inert,
            right: IndicatorGesture::Inert,
        }
    }
}
