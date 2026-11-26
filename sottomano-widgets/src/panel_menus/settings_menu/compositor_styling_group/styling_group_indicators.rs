use sottomano_compositor_model::{
    compositor_backend::CompositorFeatureSupport,
    compositor_styling_model::{ShellBlurDepth, StylingControl},
};
use sottomano_compositors::compositor_connection::CompositorKind;
use sottomano_toml_config::{Config, severity::Severity};

use super::{CompositorStylingGroup, CompositorStylingMessage};
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::{LabeledValue, ValueCell},
};

impl From<ShellBlurDepth> for SwitchPosition {
    fn from(blur_depth: ShellBlurDepth) -> Self {
        match blur_depth {
            ShellBlurDepth::WallpaperOnly => SwitchPosition::On,
            ShellBlurDepth::WindowsBehind => SwitchPosition::Off,
        }
    }
}

fn xray_toggle(blur_depth: ShellBlurDepth) -> IconToggle {
    IconToggle::new(
        "X-ray",
        Phosphor::Eye(Weight::Regular),
        blur_depth,
        SwitchPresses {
            turning_on: "blur only the wallpaper, hiding windows behind the shell",
            turning_off: "blur the windows behind the shell too",
        },
    )
}

fn compositor_face(compositor: Option<CompositorKind>) -> IconGlyph {
    match compositor.map(CompositorKind::name) {
        Some("niri") => SimpleIcon::niri.into(),
        Some("sway") => SimpleIcon::Sway.into(),
        Some("Hyprland") => SimpleIcon::Hyprland.into(),
        _ => Phosphor::AppWindow(Weight::Regular).into(),
    }
}

impl CompositorStylingGroup {
    fn with_unwritable_rules_warning<'view>(
        &'view self,
        styling_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        if self.rules_file.unwritable().is_none() {
            return styling_indicator;
        }
        styling_indicator.with_hover_card(Box::new(move |_| {
            self.rules_file
                .unwritable()
                .map(|unwritable| {
                    Section::new(
                        "The compositor rules are not written",
                        dictionary_table([LabeledValue::new(
                            ValueCell::Path(unwritable.path.to_path_buf()),
                            unwritable.kind.to_string(),
                        )
                        .level(Severity::Critical)]),
                    )
                })
                .into_iter()
                .collect()
        }))
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorGroupContent<'view> {
        let styling_indicators = [
            (
                StylingControl::WindowCornerRadius,
                GroupIndicator::slider(
                    &self.corner_radius,
                    config,
                    CompositorStylingMessage::CornerRadius,
                ),
            ),
            (
                StylingControl::WindowGaps,
                GroupIndicator::slider(&self.gaps, config, CompositorStylingMessage::Gaps),
            ),
            (
                StylingControl::ShellBlurDepth,
                GroupIndicator::toggle(
                    xray_toggle(self.blur_depth),
                    CompositorStylingMessage::ToggleXray,
                ),
            ),
            (
                StylingControl::SwitchHeatBorders,
                GroupIndicator::toggle(
                    self.switch_heat.toggle(),
                    CompositorStylingMessage::ToggleSwitchHeat,
                ),
            ),
        ];
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::CompositorStyling,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::CompositorStyling.title(),
                compositor_face(self.rules_file.compositor()),
                IndicatorState::Idle,
            ),
            &self.expansion,
            CompositorStylingMessage::Expansion,
            IndicatorListing::Listed(
                styling_indicators
                    .into_iter()
                    .filter(|(control, _)| {
                        self.control_support(*control) == CompositorFeatureSupport::Available
                    })
                    .map(|(_, styling_indicator)| {
                        self.with_unwritable_rules_warning(styling_indicator)
                    })
                    .collect(),
            ),
        )
    }
}
