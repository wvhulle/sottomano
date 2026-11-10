use std::path::PathBuf;

use sottomano_desktop_sinks::compositor_window_requests::compositor_rule_files::imported_legacy_niri_styling;
use sottomano_desktop_sources::{
    compositor_window_ipc::compositor_styling_model::{
        AppWindowLayout, CompositorStyling, ShellBlurDepth,
    },
    sqlite_state_store::{
        Columns, StoreError, in_state_store, real, upsert_column, with_state_store,
    },
};

const WINDOWS_BEHIND: &str = "windows_behind";
const WALLPAPER_ONLY: &str = "wallpaper_only";

fn stored_depth(blur_depth: ShellBlurDepth) -> &'static str {
    match blur_depth {
        ShellBlurDepth::WindowsBehind => WINDOWS_BEHIND,
        ShellBlurDepth::WallpaperOnly => WALLPAPER_ONLY,
    }
}

enum StoredStyling {
    Empty,
    Partial(CompositorStyling),
}

#[allow(clippy::cast_possible_truncation)]
fn stored_pixels(pixels: f64) -> f32 {
    pixels as f32
}

async fn read_layout(
    connection: &turso::Connection,
) -> Result<Option<AppWindowLayout>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT corner_radius_px, gaps_px FROM app_window_layout WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    Ok(Some(AppWindowLayout {
        window_corner_radius_px: stored_pixels(columns.real(&row, "corner_radius_px")?),
        gaps_px: stored_pixels(columns.real(&row, "gaps_px")?),
    }))
}

async fn read_blur_depth(
    connection: &turso::Connection,
) -> Result<Option<ShellBlurDepth>, StoreError> {
    let mut rows = connection
        .query("SELECT depth FROM shell_blur_depth WHERE id = 0", ())
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    Ok(Some(match columns.text(&row, "depth")?.as_str() {
        WALLPAPER_ONLY => ShellBlurDepth::WallpaperOnly,
        _ => ShellBlurDepth::WindowsBehind,
    }))
}

async fn read_styling(connection: &turso::Connection) -> Result<StoredStyling, StoreError> {
    let layout = read_layout(connection).await?;
    let blur_depth = read_blur_depth(connection).await?;
    Ok(match (layout, blur_depth) {
        (None, None) => StoredStyling::Empty,
        (layout, blur_depth) => StoredStyling::Partial(CompositorStyling {
            blur_depth: blur_depth.unwrap_or_default(),
            layout: layout.unwrap_or_default(),
        }),
    })
}

async fn write_layout(
    connection: &turso::Connection,
    layout: AppWindowLayout,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO app_window_layout (id, corner_radius_px, gaps_px) VALUES (0, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET corner_radius_px = ?1, gaps_px = ?2",
            (real(layout.window_corner_radius_px), real(layout.gaps_px)),
        )
        .await?;
    Ok(())
}

async fn write_blur_depth(
    connection: &turso::Connection,
    blur_depth: ShellBlurDepth,
) -> Result<(), StoreError> {
    upsert_column(
        connection,
        "shell_blur_depth",
        "depth",
        turso::Value::Text(stored_depth(blur_depth).to_owned()),
    )
    .await
}

async fn stored_or_imported(
    connection: &turso::Connection,
    config_home: PathBuf,
) -> Result<CompositorStyling, StoreError> {
    if let StoredStyling::Partial(styling) = read_styling(connection).await? {
        return Ok(styling);
    }
    let Some(imported) = imported_legacy_niri_styling(&config_home).await else {
        return Ok(CompositorStyling::default());
    };
    write_layout(connection, imported.layout).await?;
    write_blur_depth(connection, imported.blur_depth).await?;
    Ok(imported)
}

pub async fn load(config_home: PathBuf) -> CompositorStyling {
    with_state_store("compositor styling: load failed", async |connection| {
        stored_or_imported(connection, config_home).await
    })
    .await
    .unwrap_or_default()
}

pub async fn save_layout(layout: AppWindowLayout) -> Result<(), StoreError> {
    in_state_store(async |connection| write_layout(connection, layout).await).await
}

pub async fn save_blur_depth(blur_depth: ShellBlurDepth) -> Result<(), StoreError> {
    in_state_store(async |connection| write_blur_depth(connection, blur_depth).await).await
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use sottomano_desktop_sources::{
        compositor_window_ipc::compositor_styling_model::{
            AppWindowLayout, CompositorStyling, ShellBlurDepth,
        },
        sqlite_state_store::migrated_in_memory_store,
    };

    use super::{stored_or_imported, write_blur_depth, write_layout};

    fn styled() -> CompositorStyling {
        CompositorStyling {
            blur_depth: ShellBlurDepth::WallpaperOnly,
            layout: AppWindowLayout {
                window_corner_radius_px: 7.0,
                gaps_px: 5.0,
            },
        }
    }

    #[tokio::test]
    async fn an_empty_store_without_niri_rules_reads_the_defaults() {
        let connection = migrated_in_memory_store().await;
        let config_home = env::temp_dir().join("sottomano-styling-no-niri-rules");
        assert_eq!(
            stored_or_imported(&connection, config_home).await.unwrap(),
            CompositorStyling::default()
        );
    }

    #[tokio::test]
    async fn a_stored_styling_reads_back() {
        let connection = migrated_in_memory_store().await;
        write_layout(&connection, styled().layout).await.unwrap();
        write_blur_depth(&connection, styled().blur_depth)
            .await
            .unwrap();
        let config_home = env::temp_dir().join("sottomano-styling-stored");
        assert_eq!(
            stored_or_imported(&connection, config_home).await.unwrap(),
            styled()
        );
    }

    #[tokio::test]
    async fn a_lone_blur_depth_keeps_the_default_layout() {
        let connection = migrated_in_memory_store().await;
        write_blur_depth(&connection, ShellBlurDepth::WallpaperOnly)
            .await
            .unwrap();
        let config_home = env::temp_dir().join("sottomano-styling-lone-depth");
        let stored = stored_or_imported(&connection, config_home).await.unwrap();
        assert_eq!(stored.blur_depth, ShellBlurDepth::WallpaperOnly);
        assert_eq!(stored.layout, AppWindowLayout::default());
    }

    #[tokio::test]
    async fn an_empty_store_imports_the_niri_rules_once() {
        let connection = migrated_in_memory_store().await;
        let config_home =
            env::temp_dir().join(format!("sottomano-styling-import-{}", process::id()));
        let rules_directory = config_home.join("sottomano");
        fs::create_dir_all(&rules_directory).unwrap();
        fs::write(
            rules_directory.join("niri.kdl"),
            "layer-rule {\n    match namespace=\"^sottomano-panel$\"\n    background-effect {\n        \
             blur true\n        xray true\n    }\n}\nlayout {\n    gaps 5.0\n}\nwindow-rule {\n    \
             geometry-corner-radius 7.0\n    clip-to-geometry true\n}\n",
        )
        .unwrap();
        assert_eq!(
            stored_or_imported(&connection, config_home.clone())
                .await
                .unwrap(),
            styled()
        );
        fs::remove_dir_all(&config_home).unwrap();
        assert_eq!(
            stored_or_imported(&connection, config_home).await.unwrap(),
            styled()
        );
    }
}
