use crate::widget_groups::widget_group_interaction::WidgetGroupInteraction;

#[derive(Debug, Clone)]
pub enum GroupItemGesture {
    Inert,
    Performs {
        description: &'static str,
        message: WidgetGroupInteraction,
    },
}

impl GroupItemGesture {
    #[must_use]
    pub const fn description(&self) -> Option<&'static str> {
        match self {
            GroupItemGesture::Inert => None,
            GroupItemGesture::Performs { description, .. } => Some(*description),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GroupItemGestures {
    pub left: GroupItemGesture,
    pub right: GroupItemGesture,
}

impl GroupItemGestures {
    #[must_use]
    pub const fn inert() -> Self {
        Self {
            left: GroupItemGesture::Inert,
            right: GroupItemGesture::Inert,
        }
    }
}
