const SEPARATORS: [&str; 3] = [" — ", " · ", " - "];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitAppTitle {
    InContext { subject: String, context: String },
    SubjectOnly(String),
}

impl SplitAppTitle {
    pub(crate) fn before_brand(title: &str, brands: &[&str]) -> Option<Self> {
        let unbranded = title_without_brand(title, brands).filter(|page| !page.is_empty())?;
        let Some(context_separator) = last_separator(unbranded) else {
            return Some(SplitAppTitle::SubjectOnly(unbranded.to_owned()));
        };
        Some(SplitAppTitle::InContext {
            subject: unbranded[..context_separator.at].to_owned(),
            context: unbranded[context_separator.at + context_separator.length..].to_owned(),
        })
    }

    #[must_use]
    pub fn subject(&self) -> &str {
        match self {
            SplitAppTitle::InContext { subject, .. } | SplitAppTitle::SubjectOnly(subject) => {
                subject
            }
        }
    }

    #[must_use]
    pub fn context(&self) -> Option<&str> {
        match self {
            SplitAppTitle::InContext { context, .. } => Some(context),
            SplitAppTitle::SubjectOnly(_) => None,
        }
    }
}

pub(crate) fn title_without_brand<'a>(title: &'a str, brands: &[&str]) -> Option<&'a str> {
    brands.iter().find_map(|brand| {
        SEPARATORS
            .iter()
            .find_map(|separator| title.strip_suffix(&format!("{separator}{brand}")))
    })
}

struct SeparatorMatch {
    at: usize,
    length: usize,
}

fn last_separator(text: &str) -> Option<SeparatorMatch> {
    SEPARATORS
        .iter()
        .filter_map(|separator| {
            Some(SeparatorMatch {
                at: text.rfind(separator)?,
                length: separator.len(),
            })
        })
        .max_by_key(|found| found.at)
}

#[cfg(test)]
pub(crate) fn in_context(subject: &str, context: &str) -> SplitAppTitle {
    SplitAppTitle::InContext {
        subject: subject.to_owned(),
        context: context.to_owned(),
    }
}
