use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, flag, in_table, real, with_table,
};

use crate::{
    desktop_panels::region_reporter::BackdropBlur,
    widgets::shell_layer_appearance::{
        LayerAppearance, OutlineStyle, OutlineTracing, ShadowCasting, ShadowStyle, ShellLayer,
    },
};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS layer_appearance (
    layer TEXT PRIMARY KEY,
    opacity REAL NOT NULL,
    corner_radius_px REAL NOT NULL,
    blurred INTEGER NOT NULL,
    drop_shadow INTEGER NOT NULL,
    shadow_softness_px REAL NOT NULL,
    shadow_opacity REAL NOT NULL,
    outline INTEGER NOT NULL,
    outline_opacity REAL NOT NULL
)";

#[derive(Debug, Clone, Copy)]
pub enum AppearanceSetting {
    Opacity,
    CornerRadius,
    Blurred,
    DropShadow,
    ShadowSoftness,
    ShadowOpacity,
    Outline,
    OutlineOpacity,
}

impl AppearanceSetting {
    fn column(self) -> &'static str {
        match self {
            Self::Opacity => "opacity",
            Self::CornerRadius => "corner_radius_px",
            Self::Blurred => "blurred",
            Self::DropShadow => "drop_shadow",
            Self::ShadowSoftness => "shadow_softness_px",
            Self::ShadowOpacity => "shadow_opacity",
            Self::Outline => "outline",
            Self::OutlineOpacity => "outline_opacity",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AppearanceValue {
    Real(f32),
    Blur(BackdropBlur),
    Shadow(ShadowCasting),
    Outline(OutlineTracing),
}

impl From<AppearanceValue> for turso::Value {
    fn from(value: AppearanceValue) -> Self {
        match value {
            AppearanceValue::Real(number) => real(number),
            AppearanceValue::Blur(backdrop_blur) => flag(backdrop_blur == BackdropBlur::Blurred),
            AppearanceValue::Shadow(shadow_casting) => flag(shadow_casting == ShadowCasting::Cast),
            AppearanceValue::Outline(outline_tracing) => {
                flag(outline_tracing == OutlineTracing::Drawn)
            }
        }
    }
}

impl ShellLayer {
    fn key(self) -> &'static str {
        match self {
            Self::Panels => "panels",
            Self::Sidebar => "sidebar",
            Self::Overlays => "overlays",
            Self::Menus => "menus",
        }
    }
}

async fn with_table_for_appearance<T>(
    label: &str,
    work: impl AsyncFnOnce(&turso::Connection) -> Result<T, StoreError>,
) -> Option<T> {
    with_table(label, &[CREATE_TABLE], work).await
}

pub async fn save(
    layer: ShellLayer,
    setting: AppearanceSetting,
    value: AppearanceValue,
) -> Result<(), StoreError> {
    in_table(&[CREATE_TABLE], async |connection| {
        write_setting(connection, layer, setting, value.into()).await
    })
    .await
}

pub async fn save_appearance(
    layer: ShellLayer,
    appearance: LayerAppearance,
) -> Result<(), StoreError> {
    in_table(&[CREATE_TABLE], async |connection| {
        write_appearance(connection, layer, appearance).await
    })
    .await
}

pub async fn load(layer: ShellLayer) -> LayerAppearance {
    let defaults = layer.default_appearance();
    with_table_for_appearance("layer appearance: load failed", async |connection| {
        read_layer_appearance(connection, layer, defaults).await
    })
    .await
    .unwrap_or(defaults)
}

async fn insert_default_row(
    connection: &turso::Connection,
    layer: ShellLayer,
) -> Result<(), StoreError> {
    let defaults = layer.default_appearance();
    connection
        .execute(
            "INSERT INTO layer_appearance (layer, opacity, corner_radius_px, blurred, drop_shadow,
                 shadow_softness_px, shadow_opacity, outline, outline_opacity)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(layer) DO NOTHING",
            (
                layer.key(),
                real(defaults.opacity),
                real(defaults.corner_radius_px),
                turso::Value::from(AppearanceValue::Blur(defaults.backdrop_blur)),
                turso::Value::from(AppearanceValue::Shadow(defaults.shadow.casting)),
                real(defaults.shadow.softness_px),
                real(defaults.shadow.opacity),
                turso::Value::from(AppearanceValue::Outline(defaults.outline.tracing)),
                real(defaults.outline.opacity),
            ),
        )
        .await?;
    Ok(())
}

async fn write_setting(
    connection: &turso::Connection,
    layer: ShellLayer,
    setting: AppearanceSetting,
    value: turso::Value,
) -> Result<(), StoreError> {
    insert_default_row(connection, layer).await?;
    let column = setting.column();
    connection
        .execute(
            format!("UPDATE layer_appearance SET {column} = ?2 WHERE layer = ?1"),
            (layer.key(), value),
        )
        .await?;
    Ok(())
}

async fn write_appearance(
    connection: &turso::Connection,
    layer: ShellLayer,
    appearance: LayerAppearance,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO layer_appearance (layer, opacity, corner_radius_px, blurred, drop_shadow,
                 shadow_softness_px, shadow_opacity, outline, outline_opacity)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(layer) DO UPDATE SET opacity = ?2, corner_radius_px = ?3, blurred = ?4,
                 drop_shadow = ?5, shadow_softness_px = ?6, shadow_opacity = ?7, outline = ?8,
                 outline_opacity = ?9",
            (
                layer.key(),
                real(appearance.opacity),
                real(appearance.corner_radius_px),
                turso::Value::from(AppearanceValue::Blur(appearance.backdrop_blur)),
                turso::Value::from(AppearanceValue::Shadow(appearance.shadow.casting)),
                real(appearance.shadow.softness_px),
                real(appearance.shadow.opacity),
                turso::Value::from(AppearanceValue::Outline(appearance.outline.tracing)),
                real(appearance.outline.opacity),
            ),
        )
        .await?;
    Ok(())
}

async fn read_layer_appearance(
    connection: &turso::Connection,
    layer: ShellLayer,
    defaults: LayerAppearance,
) -> Result<LayerAppearance, StoreError> {
    let mut rows = connection
        .query(
            "SELECT opacity, corner_radius_px, blurred, drop_shadow, shadow_softness_px,
                    shadow_opacity, outline, outline_opacity
             FROM layer_appearance WHERE layer = ?1",
            (layer.key(),),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(defaults);
    };
    let real = |setting: AppearanceSetting| -> Result<f32, StoreError> {
        Ok(columns.real(&row, setting.column())? as f32)
    };
    let stored_integer = |setting: AppearanceSetting| -> Result<i64, StoreError> {
        columns.integer(&row, setting.column())
    };
    Ok(LayerAppearance {
        background: defaults.background,
        opacity: real(AppearanceSetting::Opacity)?,
        corner_radius_px: real(AppearanceSetting::CornerRadius)?,
        backdrop_blur: match stored_integer(AppearanceSetting::Blurred)? {
            0 => BackdropBlur::Clear,
            _ => BackdropBlur::Blurred,
        },
        shadow: ShadowStyle {
            casting: match stored_integer(AppearanceSetting::DropShadow)? {
                0 => ShadowCasting::Omitted,
                _ => ShadowCasting::Cast,
            },
            softness_px: real(AppearanceSetting::ShadowSoftness)?,
            opacity: real(AppearanceSetting::ShadowOpacity)?,
        },
        outline: OutlineStyle {
            tracing: match stored_integer(AppearanceSetting::Outline)? {
                0 => OutlineTracing::Omitted,
                _ => OutlineTracing::Drawn,
            },
            opacity: real(AppearanceSetting::OutlineOpacity)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::sqlite_state_store::{flag, in_memory_connection, real};

    use super::{
        AppearanceSetting, CREATE_TABLE, read_layer_appearance, write_appearance, write_setting,
    };
    use crate::{
        desktop_panels::region_reporter::BackdropBlur,
        widgets::shell_layer_appearance::{
            LayerAppearance, LookComparison, OutlineTracing, ShadowCasting, ShellLayer,
        },
    };

    #[tokio::test]
    async fn every_setting_round_trips_within_its_own_layer() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        for (setting, value) in [
            (AppearanceSetting::Opacity, real(0.4)),
            (AppearanceSetting::CornerRadius, real(9.0)),
            (AppearanceSetting::Blurred, flag(true)),
            (AppearanceSetting::DropShadow, flag(false)),
            (AppearanceSetting::ShadowSoftness, real(33.0)),
            (AppearanceSetting::ShadowOpacity, real(0.5)),
            (AppearanceSetting::Outline, flag(false)),
            (AppearanceSetting::OutlineOpacity, real(0.75)),
        ] {
            write_setting(&connection, ShellLayer::Sidebar, setting, value)
                .await
                .unwrap();
        }

        let defaults = LayerAppearance::default();
        let sidebar = read_layer_appearance(&connection, ShellLayer::Sidebar, defaults)
            .await
            .unwrap();
        assert!((sidebar.opacity - 0.4).abs() < f32::EPSILON);
        assert!((sidebar.corner_radius_px - 9.0).abs() < f32::EPSILON);
        assert_eq!(sidebar.backdrop_blur, BackdropBlur::Blurred);
        assert_eq!(sidebar.shadow.casting, ShadowCasting::Omitted);
        assert!((sidebar.shadow.softness_px - 33.0).abs() < f32::EPSILON);
        assert!((sidebar.shadow.opacity - 0.5).abs() < f32::EPSILON);
        assert_eq!(sidebar.outline.tracing, OutlineTracing::Omitted);
        assert!((sidebar.outline.opacity - 0.75).abs() < f32::EPSILON);

        let panels = read_layer_appearance(&connection, ShellLayer::Panels, defaults)
            .await
            .unwrap();
        assert_eq!(panels, defaults);
    }

    #[tokio::test]
    async fn a_whole_appearance_round_trips_in_one_write() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        let written = LayerAppearance {
            opacity: 0.25,
            backdrop_blur: BackdropBlur::Clear,
            ..ShellLayer::Sidebar.default_appearance()
        };
        write_appearance(&connection, ShellLayer::Sidebar, written)
            .await
            .unwrap();
        let read = read_layer_appearance(
            &connection,
            ShellLayer::Sidebar,
            ShellLayer::Sidebar.default_appearance(),
        )
        .await
        .unwrap();
        assert_eq!(read.look_comparison(&written), LookComparison::Matching);
    }
}
