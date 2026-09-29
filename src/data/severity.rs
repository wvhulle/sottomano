#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    #[default]
    Normal,
    Warning,
    Critical,
}
