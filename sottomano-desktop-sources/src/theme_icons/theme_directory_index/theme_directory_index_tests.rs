use std::process;

use super::{index_theme_metadata::DirectoryDeclaration, *};

const EXAMPLE_THEME_INDEX: &str = r"[Icon Theme]
Name=Example-Dark
Inherits=example-base,hicolor
Directories=actions/16,actions/16@2x,apps/scalable,broken

[actions/16]
Size=16
Type=Fixed

[actions/16@2x]
Size=16
Scale=2
Type=Fixed

[apps/scalable]
Size=16
MinSize=16
MaxSize=512
Type=Scalable

[broken]
Size=large
";

#[test]
fn theme_metadata_keeps_unscaled_sized_directories_and_inheritance() {
    let metadata =
        parsed_theme_metadata(Path::new("Example-Dark/index.theme"), EXAMPLE_THEME_INDEX)
            .expect("example index parses");
    assert_eq!(metadata.inherited_themes, ["example-base", "hicolor"]);
    assert_eq!(
        metadata.directory_declarations,
        [
            DirectoryDeclaration {
                relative_path: "actions/16".to_owned(),
                sizing: DirectorySizing::Fixed { side_px: 16 },
            },
            DirectoryDeclaration {
                relative_path: "apps/scalable".to_owned(),
                sizing: DirectorySizing::Scalable {
                    min_px: 16,
                    max_px: 512
                },
            },
        ]
    );
}

#[tokio::test]
async fn an_installed_theme_is_indexed_with_its_icon_files() {
    let theme_root = env::temp_dir().join(format!("sottomano-theme-index-{}", process::id()));
    let action_directory = theme_root.join("Example-Dark/actions/16");
    fs::create_dir_all(&action_directory)
        .await
        .expect("scratch theme directory is creatable");
    fs::write(
        theme_root.join("Example-Dark/index.theme"),
        EXAMPLE_THEME_INDEX,
    )
    .await
    .expect("scratch index is writable");
    fs::write(action_directory.join("edit-select.svg"), "<svg/>")
        .await
        .expect("scratch icon is writable");
    let theme_index = indexed_icon_theme(
        "Example-Dark".to_owned(),
        IconSearchRoots {
            theme_roots: vec![theme_root.clone()],
            pixmap_directories: Vec::new(),
        },
    )
    .await
    .expect("scratch theme indexes");
    fs::remove_dir_all(&theme_root)
        .await
        .expect("scratch theme is removable");
    let indexed_theme_names: Vec<&str> = theme_index
        .searched_themes
        .iter()
        .map(|indexed_theme| indexed_theme.theme_name.as_str())
        .collect();
    assert_eq!(indexed_theme_names, ["Example-Dark"]);
    assert!(
        theme_index.searched_themes[0].sized_directories[0]
            .icon_files
            .contains_key("edit-select")
    );
}
