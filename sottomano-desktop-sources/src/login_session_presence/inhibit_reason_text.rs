#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InhibitReason {
    Stated(String),
    Unstated,
}

impl From<String> for InhibitReason {
    fn from(reason_text: String) -> Self {
        if reason_text.is_empty() {
            InhibitReason::Unstated
        } else {
            InhibitReason::Stated(reason_text)
        }
    }
}
