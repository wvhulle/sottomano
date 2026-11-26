use sottomano_compositor_model::compositor_styling_model::{
    AppWindowLayout, CompositorStyling, ShellBlurDepth,
};
use sottomano_desktop_sources::sqlite_state_store::{
    StoreError, in_state_store, real, singleton_row, upsert_column, with_state_store,
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
    singleton_row(
        connection,
        "SELECT corner_radius_px, gaps_px FROM app_window_layout WHERE id = 0",
        |columns, row| {
            Ok(AppWindowLayout {
                window_corner_radius_px: stored_pixels(columns.real(row, "corner_radius_px")?),
                gaps_px: stored_pixels(columns.real(row, "gaps_px")?),
            })
        },
    )
    .await
}

async fn read_blur_depth(
    connection: &turso::Connection,
) -> Result<Option<ShellBlurDepth>, StoreError> {
    singleton_row(
        connection,
        "SELECT depth FROM shell_blur_depth WHERE id = 0",
        |columns, row| {
            Ok(match columns.text(row, "depth")?.as_str() {
                WALLPAPER_ONLY => ShellBlurDepth::WallpaperOnly,
                _ => ShellBlurDepth::WindowsBehind,
            })
        },
    )
    .await
}

async fn read_styling(connection: &turso::Connection) -> Result<StoredStyling, StoreError> {
    let layout = read_layout(connection).await?;
    let blur_depth = read_blur_depth(connection).await?;
    if layout.is_none() && blur_depth.is_none() {
        return Ok(StoredStyling::Empty);
    }
    Ok(StoredStyling::Partial(CompositorStyling {
        blur_depth: blur_depth.unwrap_or_default(),
        layout: layout.unwrap_or_default(),
    }))
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

async fn stored_or_default(
    connection: &turso::Connection,
) -> Result<CompositorStyling, StoreError> {
    Ok(match read_styling(connection).await? {
        StoredStyling::Partial(styling) => styling,
        StoredStyling::Empty => CompositorStyling::default(),
    })
}

pub async fn load() -> CompositorStyling {
    with_state_store("compositor styling: load failed", async |connection| {
        stored_or_default(connection).await
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
    use sottomano_compositor_model::compositor_styling_model::{
        AppWindowLayout, CompositorStyling, ShellBlurDepth,
    };
    use sottomano_desktop_sources::sqlite_state_store::migrated_in_memory_store;

    use super::{stored_or_default, write_blur_depth, write_layout};

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
    async fn an_empty_store_reads_the_defaults() {
        let connection = migrated_in_memory_store().await;
        assert_eq!(
            stored_or_default(&connection).await.unwrap(),
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
        assert_eq!(stored_or_default(&connection).await.unwrap(), styled());
    }

    #[tokio::test]
    async fn a_lone_blur_depth_keeps_the_default_layout() {
        let connection = migrated_in_memory_store().await;
        write_blur_depth(&connection, ShellBlurDepth::WallpaperOnly)
            .await
            .unwrap();
        let stored = stored_or_default(&connection).await.unwrap();
        assert_eq!(stored.blur_depth, ShellBlurDepth::WallpaperOnly);
        assert_eq!(stored.layout, AppWindowLayout::default());
    }
}
