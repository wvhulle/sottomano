use super::split_app_title::SplitAppTitle;

pub const ZED_APP_IDS: [&str; 2] = ["dev.zed.Zed", "dev.zed.Zed-Preview"];

const PROJECT_SEPARATOR: &str = " — ";

#[must_use]
pub fn split_zed_title(title: &str) -> Option<SplitAppTitle> {
    let (project, subject) = title.split_once(PROJECT_SEPARATOR)?;
    Some(SplitAppTitle::InContext {
        subject: subject.to_owned(),
        context: project.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::{super::split_app_title::in_context, *};

    #[test]
    fn zed_titles_put_the_project_first() {
        assert_eq!(
            split_zed_title("sottomano — CONTRIBUTING.md"),
            Some(in_context("CONTRIBUTING.md", "sottomano"))
        );
        assert_eq!(split_zed_title("sottomano"), None);
    }
}
