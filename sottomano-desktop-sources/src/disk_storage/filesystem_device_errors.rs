#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceErrors {
    pub counted_on: String,
    pub counts: Vec<ErrorCount>,
    pub reset_command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCount {
    pub kind: &'static str,
    pub count: u64,
}

impl ErrorCount {
    #[must_use]
    pub fn new(kind: &'static str, count: u64) -> Self {
        ErrorCount { kind, count }
    }
}

impl DeviceErrors {
    #[must_use]
    pub fn total(&self) -> u64 {
        self.counts.iter().map(|error| error.count).sum()
    }

    pub fn nonzero_counts(&self) -> impl Iterator<Item = ErrorCount> + '_ {
        self.counts.iter().copied().filter(|error| error.count > 0)
    }
}
