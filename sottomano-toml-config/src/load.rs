use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

use chrono::Locale;
use serde::{Deserialize, Serialize};
use toml::{de, ser};

use super::{
    calendar::{self, Calendar, CalendarChoice},
    desktop_panels::DesktopPanelsConfig,
    group_items::GroupItemsConfig,
    group_menus::GroupMenusConfig,
    hover_cards::HoverCardsConfig,
    launched_programs::LaunchedPrograms,
    notification_toasts::NotificationToastsConfig,
    overlay_surfaces::OverlaySurfacesConfig,
    spacing::Spacing,
    theme::{
        Theme, ThemeChoice,
        font::{self, FontError, FontRole},
    },
    widget_group_layout::WidgetGroupLayoutConfig,
    widget_groups::WidgetGroupsConfig,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub theme: Theme,
    pub desktop_panels: DesktopPanelsConfig,
    pub group_menus: GroupMenusConfig,
    pub widget_group_layout: WidgetGroupLayoutConfig,
    pub group_items: GroupItemsConfig,
    pub hover_cards: HoverCardsConfig,
    pub overlay_surfaces: OverlaySurfacesConfig,
    pub widget_groups: WidgetGroupsConfig,
    pub notification_toasts: NotificationToastsConfig,
    pub calendar: Calendar,
    pub launched_programs: LaunchedPrograms,
}

impl Default for Config {
    fn default() -> Self {
        UserConfig::default()
            .resolve()
            .expect("the default configuration resolves")
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct UserConfig {
    theme: ThemeChoice,
    desktop_panels: DesktopPanelsConfig<Spacing>,
    group_menus: GroupMenusConfig,
    widget_group_layout: WidgetGroupLayoutConfig,
    group_items: GroupItemsConfig,
    hover_cards: HoverCardsConfig,
    overlay_surfaces: OverlaySurfacesConfig,
    widget_groups: WidgetGroupsConfig,
    notification_toasts: NotificationToastsConfig,
    calendar: CalendarChoice,
    launched_programs: LaunchedPrograms,
}

impl UserConfig {
    fn resolve(self) -> Result<Config, LoadError> {
        let locale = match self.calendar.locale {
            None => calendar::locale_from_environment(),
            Some(configured) => configured
                .parse::<Locale>()
                .map_err(|_| LoadError::UnknownLocale(configured))?,
        };
        let font = font::resolve(&self.theme.font, FontRole::Text).map_err(LoadError::Font)?;
        let em_px = font.text_size_px;
        Ok(Config {
            theme: Theme {
                font,
                monospace_font: font::resolve(&self.theme.font, FontRole::Monospace)
                    .map_err(LoadError::Font)?,
                palette: self.theme.palette,
                spacing: self.theme.spacing.resolve(em_px),
            },
            desktop_panels: self.desktop_panels.resolve(em_px),
            group_menus: self.group_menus,
            widget_group_layout: self.widget_group_layout,
            group_items: self.group_items,
            hover_cards: self.hover_cards,
            overlay_surfaces: self.overlay_surfaces,
            widget_groups: self.widget_groups,
            notification_toasts: self.notification_toasts,
            calendar: Calendar {
                locale,
                other_time_zones: self.calendar.other_time_zones,
                working_hours: self.calendar.working_hours,
            },
            launched_programs: self.launched_programs,
        })
    }
}

#[derive(Debug)]
pub enum LoadError {
    Unreadable(io::Error),
    Invalid { error: de::Error, text: String },
    UnknownLocale(String),
    Font(FontError),
}

pub struct OffendingLine<'a> {
    pub number: usize,
    pub before: &'a str,
    pub offending: &'a str,
    pub after: &'a str,
}

impl LoadError {
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Invalid { error, .. } => error.message().to_owned(),
            _ => self.to_string(),
        }
    }

    #[must_use]
    pub fn offending_line(&self) -> Option<OffendingLine<'_>> {
        let Self::Invalid { error, text } = self else {
            return None;
        };
        let span = error.span()?;
        let preceding = text.get(..span.start)?;
        let line_start = preceding.rfind('\n').map_or(0, |newline| newline + 1);
        let line_end = text
            .get(span.start..)?
            .find('\n')
            .map_or(text.len(), |newline| span.start + newline);
        let offending_end = span.end.clamp(span.start, line_end);
        Some(OffendingLine {
            number: preceding.matches('\n').count() + 1,
            before: text.get(line_start..span.start)?,
            offending: text.get(span.start..offending_end)?,
            after: text.get(offending_end..line_end)?,
        })
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(e) => write!(f, "cannot be read: {e}"),
            Self::Invalid { error, .. } => write!(f, "{error}"),
            Self::UnknownLocale(locale) => write!(f, "unknown locale {locale:?}"),
            Self::Font(e) => write!(f, "{e}"),
        }
    }
}

#[derive(Debug, Clone, Hash)]
pub struct ConfigFile {
    pub(super) path: PathBuf,
}

impl ConfigFile {
    #[must_use]
    pub fn in_config_home(config_home: &Path) -> Self {
        Self {
            path: config_home.join("sottomano.toml"),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Config, LoadError> {
        self.read()?.resolve()
    }

    fn read(&self) -> Result<UserConfig, LoadError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => toml::from_str(&text).map_err(|error| LoadError::Invalid { error, text }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(UserConfig::default()),
            Err(e) => Err(LoadError::Unreadable(e)),
        }
    }
}

pub fn default_config_toml() -> Result<String, ser::Error> {
    toml::to_string(&UserConfig::default())
}

pub fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use chrono_tz::Asia;

    use super::{ConfigFile, FontError, LoadError, Spacing, UserConfig, default_config_toml};

    fn file_containing(test: &str, toml: &str) -> ConfigFile {
        let directory = env::temp_dir().join(format!("sottomano-{test}-{}", process::id()));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("sottomano.toml"), toml).unwrap();
        ConfigFile::in_config_home(&directory)
    }

    #[test]
    fn keys_from_before_the_rename_are_rejected() {
        let removed_tables = [
            ("chrome", "[chrome]\nouter_height_px = 28.0\n"),
            ("panel_bar", "[panel_bar]\nslider_width = \"60px\"\n"),
            ("tooltips", "[tooltips]\nvalue_width = \"620px\"\n"),
            ("layout", "[layout.panel]\nheight = \"24px\"\n"),
            (
                "window_list",
                "[widget_groups.window_list]\nprogress = true\n",
            ),
            (
                "content_inset",
                "[overlay_surfaces]\ncontent_inset = \"10px\"\n",
            ),
            ("row_gap", "[overlay_surfaces]\nrow_gap = \"4px\"\n"),
            ("column_gap", "[overlay_surfaces]\ncolumn_gap = \"14px\"\n"),
            (
                "section_gap",
                "[overlay_surfaces]\nsection_gap = \"16px\"\n",
            ),
            (
                "satellite_gap",
                "[overlay_surfaces]\nsatellite_gap = \"14px\"\n",
            ),
            ("item_gap", "[group_items]\nitem_gap = \"4px\"\n"),
            ("slider_width", "[group_items]\nslider_width = \"60px\"\n"),
            (
                "slider_stroke_width",
                "[group_items]\nslider_stroke_width = \"2px\"\n",
            ),
            (
                "frame_outline_width",
                "[desktop_panels]\nframe_outline_width = \"1px\"\n",
            ),
            (
                "group_edge_padding",
                "[desktop_panels]\ngroup_edge_padding = \"5px\"\n",
            ),
            (
                "joined_group_gap",
                "[desktop_panels]\njoined_group_gap = \"11px\"\n",
            ),
            (
                "framed_group_gap",
                "[desktop_panels]\nframed_group_gap = \"4px\"\n",
            ),
            (
                "start_distance",
                "[desktop_panels.slot_drag]\nstart_distance = \"6px\"\n",
            ),
            (
                "edge_indicator_thickness",
                "[widget_group_layout]\nedge_indicator_thickness = \"2.4px\"\n",
            ),
            (
                "value_column_width",
                "[hover_cards]\nvalue_column_width = \"620px\"\n",
            ),
            (
                "edge_gap",
                "[notification_toasts.placement]\nedge_gap = \"24px\"\n",
            ),
            (
                "window_right_offset",
                "[notification_toasts.placement]\nwindow_right_offset = \"12px\"\n",
            ),
            (
                "window_top_offset",
                "[notification_toasts.placement]\nwindow_top_offset = \"48px\"\n",
            ),
            ("width", "[notification_toasts.card]\nwidth = \"440px\"\n"),
            (
                "icon_size",
                "[notification_toasts.card]\nicon_size = \"44px\"\n",
            ),
            (
                "app_badge_size",
                "[notification_toasts.card]\napp_badge_size = \"20px\"\n",
            ),
            (
                "image_preview_max_height",
                "[notification_toasts.card]\nimage_preview_max_height = \"180px\"\n",
            ),
            (
                "small_text_size",
                "[theme.font]\nsmall_text_size = \"12px\"\n",
            ),
            ("icon_size", "[theme.font]\nicon_size = \"15.5px\"\n"),
        ];
        for (table_name, toml) in removed_tables {
            let file = file_containing("old-keys", toml);
            let error = file.load().unwrap_err();
            fs::remove_dir_all(file.path().parent().unwrap()).unwrap();
            assert!(matches!(error, LoadError::Invalid { .. }));
            assert!(error.to_string().contains(table_name));
        }
    }

    #[test]
    fn spacing_in_em_resolves_against_the_text_size() {
        let file = file_containing("em", "[theme.spacing]\nsurface_inset = \"0.5em\"\n");
        let read = file.read().unwrap();
        fs::remove_dir_all(file.path().parent().unwrap()).unwrap();
        let spacing = read.theme.spacing.resolve(14.0);
        assert!((spacing.surface_inset - 7.0).abs() < f32::EPSILON);
        assert!((spacing.item_gap - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_bare_number_is_rejected_as_spacing() {
        let file = file_containing("bare", "[desktop_panels]\nheight = 24\n");
        let read = file.read();
        fs::remove_dir_all(file.path().parent().unwrap()).unwrap();
        assert!(matches!(read, Err(LoadError::Invalid { .. })));
    }

    #[test]
    fn the_base_text_size_cannot_be_relative() {
        let file = file_containing("base", "[theme.font]\ntext_size = \"1em\"\n");
        let read = file.read().unwrap();
        fs::remove_dir_all(file.path().parent().unwrap()).unwrap();
        assert!(matches!(
            read.theme.font.text_size_px(),
            Err(FontError::RelativeTextSize(Spacing::Em(_)))
        ));
    }

    #[test]
    fn other_time_zones_parse_as_iana_names() {
        let zones = |toml: &str| file_containing("zone", &format!("[calendar]\n{toml}\n")).read();
        let configured = zones("other_time_zones = [\"Asia/Tokyo\"]");
        let emptied = zones("other_time_zones = []");
        let unknown = zones("other_time_zones = [\"Mars/Olympus\"]");
        let file = file_containing("zone", "");
        fs::remove_dir_all(file.path().parent().unwrap()).unwrap();
        assert_eq!(configured.unwrap().calendar.other_time_zones, [Asia::Tokyo]);
        assert!(emptied.unwrap().calendar.other_time_zones.is_empty());
        assert!(matches!(unknown, Err(LoadError::Invalid { .. })));
    }

    #[test]
    fn an_unknown_locale_is_rejected_with_its_name() {
        let file = file_containing("locale", "[calendar]\nlocale = \"xx_nowhere\"\n");
        let error = file.load().unwrap_err();
        fs::remove_dir_all(file.path().parent().unwrap()).unwrap();
        assert!(matches!(error, LoadError::UnknownLocale(ref name) if name == "xx_nowhere"));
    }

    #[test]
    fn the_printed_default_config_reads_back_unchanged() {
        let printed = default_config_toml().unwrap();
        let read_back: UserConfig = toml::from_str(&printed).unwrap();
        assert_eq!(toml::to_string(&read_back).unwrap(), printed);
    }
}
