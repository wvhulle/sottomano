#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionPlan {
    Named(String),
    Unnamed,
}
