use crate::data::source::journal::KernelEntry;

#[derive(Debug, Clone)]
pub struct KernelErrors {
    pub entries: Vec<KernelEntry>,
}

impl KernelErrors {
    pub fn from_entries(entries: Vec<KernelEntry>) -> Option<Self> {
        (!entries.is_empty()).then_some(Self { entries })
    }

    pub fn follow_log_args() -> Vec<String> {
        ["-k", "-b", "-f", "-p", "warning"]
            .map(String::from)
            .to_vec()
    }
}
