#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupItemGesture<M> {
    Inert,
    Performs {
        description: &'static str,
        message: M,
    },
}

impl<M> GroupItemGesture<M> {
    #[must_use]
    pub const fn description(&self) -> Option<&'static str> {
        match self {
            GroupItemGesture::Inert => None,
            GroupItemGesture::Performs { description, .. } => Some(*description),
        }
    }

    #[must_use]
    pub fn map<Outer>(self, lift: fn(M) -> Outer) -> GroupItemGesture<Outer> {
        match self {
            GroupItemGesture::Inert => GroupItemGesture::Inert,
            GroupItemGesture::Performs {
                description,
                message,
            } => GroupItemGesture::Performs {
                description,
                message: lift(message),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupItemGestures<M> {
    pub left: GroupItemGesture<M>,
    pub right: GroupItemGesture<M>,
}

impl<M> GroupItemGestures<M> {
    #[must_use]
    pub const fn inert() -> Self {
        Self {
            left: GroupItemGesture::Inert,
            right: GroupItemGesture::Inert,
        }
    }

    #[must_use]
    pub fn map<Outer>(self, lift: fn(M) -> Outer) -> GroupItemGestures<Outer> {
        GroupItemGestures {
            left: self.left.map(lift),
            right: self.right.map(lift),
        }
    }
}
