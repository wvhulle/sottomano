use super::split_app_title::SplitAppTitle;

pub const VISUAL_STUDIO_CODE_APP_IDS: [&str; 4] = [
    "code",
    "code-url-handler",
    "codium",
    "com.visualstudio.code",
];

const VISUAL_STUDIO_CODE_BRANDS: [&str; 2] = ["Visual Studio Code", "VSCodium"];

#[must_use]
pub fn split_visual_studio_code_title(title: &str) -> Option<SplitAppTitle> {
    SplitAppTitle::before_brand(title, &VISUAL_STUDIO_CODE_BRANDS)
}

#[cfg(test)]
mod tests {
    use super::{super::split_app_title::in_context, *};

    #[test]
    fn a_file_in_a_workspace_names_its_project() {
        assert_eq!(
            split_visual_studio_code_title("README.md - sottomano - Visual Studio Code"),
            Some(in_context("README.md", "sottomano"))
        );
    }

    #[test]
    fn a_file_outside_a_workspace_has_no_project() {
        assert_eq!(
            split_visual_studio_code_title("● settings.json - Visual Studio Code"),
            Some(SplitAppTitle::SubjectOnly("● settings.json".to_owned()))
        );
    }
}
