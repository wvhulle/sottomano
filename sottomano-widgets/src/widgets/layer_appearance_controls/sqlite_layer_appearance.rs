use corredo::theming::{FrameStyle, OutlineStyle, OutlineTracing, ShadowCasting, ShadowStyle};
use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, flag_set_when, in_state_store, real, with_state_store,
};

use crate::{
    desktop_panels::region_reporter::BackdropBlur,
    widgets::shell_layer_appearance::{LayerAppearance, ShellLayer},
};

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
            AppearanceValue::Blur(backdrop_blur) => {
                flag_set_when(backdrop_blur, BackdropBlur::Blurred)
            }
            AppearanceValue::Shadow(shadow_casting) => {
                flag_set_when(shadow_casting, ShadowCasting::Cast)
            }
            AppearanceValue::Outline(outline_tracing) => {
                flag_set_when(outline_tracing, OutlineTracing::Drawn)
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

pub async fn save(
    layer: ShellLayer,
    setting: AppearanceSetting,
    value: AppearanceValue,
) -> Result<(), StoreError> {
    in_state_store(async |connection| write_setting(connection, layer, setting, value.into()).await)
        .await
}

pub async fn save_appearance(
    layer: ShellLayer,
    appearance: LayerAppearance,
) -> Result<(), StoreError> {
    in_state_store(async |connection| write_appearance(connection, layer, appearance).await).await
}

pub async fn load(layer: ShellLayer) -> LayerAppearance {
    let defaults = layer.default_appearance();
    with_state_store("layer appearance: load failed", async |connection| {
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
                real(defaults.frame.opacity),
                real(defaults.frame.corner_radius_px),
                turso::Value::from(AppearanceValue::Blur(defaults.backdrop_blur)),
                turso::Value::from(AppearanceValue::Shadow(defaults.frame.shadow.casting)),
                real(defaults.frame.shadow.softness_px),
                real(defaults.frame.shadow.opacity),
                turso::Value::from(AppearanceValue::Outline(defaults.frame.outline.tracing)),
                real(defaults.frame.outline.opacity),
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
                real(appearance.frame.opacity),
                real(appearance.frame.corner_radius_px),
                turso::Value::from(AppearanceValue::Blur(appearance.backdrop_blur)),
                turso::Value::from(AppearanceValue::Shadow(appearance.frame.shadow.casting)),
                real(appearance.frame.shadow.softness_px),
                real(appearance.frame.shadow.opacity),
                turso::Value::from(AppearanceValue::Outline(appearance.frame.outline.tracing)),
                real(appearance.frame.outline.opacity),
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
    let columns = Columns::from(&rows);
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
        frame: FrameStyle {
            opacity: real(AppearanceSetting::Opacity)?,
            corner_radius_px: real(AppearanceSetting::CornerRadius)?,
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
            ..defaults.frame
        },
        backdrop_blur: match stored_integer(AppearanceSetting::Blurred)? {
            0 => BackdropBlur::Clear,
            _ => BackdropBlur::Blurred,
        },
    })
}

#[cfg(test)]
mod tests {
    use corredo::theming::{FrameStyle, OutlineTracing, ShadowCasting};
    use sottomano_desktop_sources::sqlite_state_store::{
        flag_set_when, migrated_in_memory_store, real,
    };

    use super::{AppearanceSetting, read_layer_appearance, write_appearance, write_setting};
    use crate::{
        desktop_panels::region_reporter::BackdropBlur,
        widgets::shell_layer_appearance::{LayerAppearance, LookComparison, ShellLayer},
    };

    #[tokio::test]
    async fn every_setting_round_trips_within_its_own_layer() {
        let connection = migrated_in_memory_store().await;
        for (setting, value) in [
            (AppearanceSetting::Opacity, real(0.4)),
            (AppearanceSetting::CornerRadius, real(9.0)),
            (
                AppearanceSetting::Blurred,
                flag_set_when(BackdropBlur::Blurred, BackdropBlur::Blurred),
            ),
            (
                AppearanceSetting::DropShadow,
                flag_set_when(ShadowCasting::Omitted, ShadowCasting::Cast),
            ),
            (AppearanceSetting::ShadowSoftness, real(33.0)),
            (AppearanceSetting::ShadowOpacity, real(0.5)),
            (
                AppearanceSetting::Outline,
                flag_set_when(OutlineTracing::Omitted, OutlineTracing::Drawn),
            ),
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
        assert!((sidebar.frame.opacity - 0.4).abs() < f32::EPSILON);
        assert!((sidebar.frame.corner_radius_px - 9.0).abs() < f32::EPSILON);
        assert_eq!(sidebar.backdrop_blur, BackdropBlur::Blurred);
        assert_eq!(sidebar.frame.shadow.casting, ShadowCasting::Omitted);
        assert!((sidebar.frame.shadow.softness_px - 33.0).abs() < f32::EPSILON);
        assert!((sidebar.frame.shadow.opacity - 0.5).abs() < f32::EPSILON);
        assert_eq!(sidebar.frame.outline.tracing, OutlineTracing::Omitted);
        assert!((sidebar.frame.outline.opacity - 0.75).abs() < f32::EPSILON);

        let panels = read_layer_appearance(&connection, ShellLayer::Panels, defaults)
            .await
            .unwrap();
        assert_eq!(panels, defaults);
    }

    #[tokio::test]
    async fn a_whole_appearance_round_trips_in_one_write() {
        let connection = migrated_in_memory_store().await;
        let sidebar = ShellLayer::Sidebar.default_appearance();
        let written = LayerAppearance {
            frame: FrameStyle {
                opacity: 0.25,
                ..sidebar.frame
            },
            backdrop_blur: BackdropBlur::Clear,
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
