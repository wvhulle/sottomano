use sottomano_sources::sqlite_state_store::{Columns, StoreError, flag, real, with_table};

use crate::widgets::shell_layer_appearance::{
    LayerAppearance, OutlineStyle, ShadowStyle, ShellLayer,
};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS layer_appearance (
    layer TEXT PRIMARY KEY,
    opacity REAL,
    corner_radius_px REAL,
    blurred INTEGER,
    drop_shadow INTEGER,
    shadow_softness_px REAL,
    shadow_opacity REAL,
    outline INTEGER,
    outline_opacity REAL
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
    Flag(bool),
}

impl From<AppearanceValue> for turso::Value {
    fn from(value: AppearanceValue) -> Self {
        match value {
            AppearanceValue::Real(number) => real(number),
            AppearanceValue::Flag(enabled) => flag(enabled),
        }
    }
}

impl ShellLayer {
    fn key(self) -> &'static str {
        match self {
            Self::Panels => "panels",
            Self::Sidebar => "sidebar",
            Self::Overlays => "overlays",
        }
    }
}

pub async fn save(layer: ShellLayer, setting: AppearanceSetting, value: AppearanceValue) -> bool {
    with_table(
        &format!(
            "layer appearance: save {} {} failed",
            layer.key(),
            setting.column()
        ),
        &[CREATE_TABLE],
        async |connection| write_setting(connection, layer, setting, value.into()).await,
    )
    .await
    .is_some()
}

pub async fn load(layer: ShellLayer) -> LayerAppearance {
    let defaults = layer.default_appearance();
    with_table(
        "layer appearance: load failed",
        &[CREATE_TABLE],
        async |connection| read_layer_appearance(connection, layer, defaults).await,
    )
    .await
    .unwrap_or(defaults)
}

async fn write_setting(
    connection: &turso::Connection,
    layer: ShellLayer,
    setting: AppearanceSetting,
    value: turso::Value,
) -> Result<(), StoreError> {
    let column = setting.column();
    connection
        .execute(
            format!(
                "INSERT INTO layer_appearance (layer, {column}) VALUES (?1, ?2)
             ON CONFLICT(layer) DO UPDATE SET {column} = ?2"
            ),
            (layer.key(), value),
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
    let real = |setting: AppearanceSetting, fallback: f32| {
        columns
            .optional_real(&row, setting.column())
            .map_or(fallback, |value| value as f32)
    };
    let flag = |setting: AppearanceSetting, fallback: bool| {
        columns
            .optional_flag(&row, setting.column())
            .unwrap_or(fallback)
    };
    Ok(LayerAppearance {
        opacity: real(AppearanceSetting::Opacity, defaults.opacity),
        corner_radius_px: real(AppearanceSetting::CornerRadius, defaults.corner_radius_px),
        blurred: flag(AppearanceSetting::Blurred, defaults.blurred),
        shadow: ShadowStyle {
            cast: flag(AppearanceSetting::DropShadow, defaults.shadow.cast),
            softness_px: real(
                AppearanceSetting::ShadowSoftness,
                defaults.shadow.softness_px,
            ),
            opacity: real(AppearanceSetting::ShadowOpacity, defaults.shadow.opacity),
        },
        outline: OutlineStyle {
            drawn: flag(AppearanceSetting::Outline, defaults.outline.drawn),
            opacity: real(AppearanceSetting::OutlineOpacity, defaults.outline.opacity),
        },
    })
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::{flag, in_memory_connection, real};

    use super::{AppearanceSetting, CREATE_TABLE, read_layer_appearance, write_setting};
    use crate::widgets::shell_layer_appearance::{LayerAppearance, ShellLayer};

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
        assert!(sidebar.blurred);
        assert!(!sidebar.shadow.cast);
        assert!((sidebar.shadow.softness_px - 33.0).abs() < f32::EPSILON);
        assert!((sidebar.shadow.opacity - 0.5).abs() < f32::EPSILON);
        assert!(!sidebar.outline.drawn);
        assert!((sidebar.outline.opacity - 0.75).abs() < f32::EPSILON);

        let panels = read_layer_appearance(&connection, ShellLayer::Panels, defaults)
            .await
            .unwrap();
        assert_eq!(panels, defaults);
    }
}
