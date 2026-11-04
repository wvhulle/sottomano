use std::{mem, sync::Arc};

use iced::{Task, font};
use sottomano_config::{Config, LoadError, theme::Theme};
use sottomano_sources::blocking_background_work::blocking;
use sottomano_widgets::widgets::{configured_theme::configured_theme, text::FontFaces};

use crate::{desktop_shell::DesktopShell, shell_message::Message};

impl DesktopShell {
    pub(super) fn reload_config(&self) -> Task<Message> {
        let file = self.config_file.clone();
        Task::future(blocking("config reload", move || Some(file.load()))).and_then(|loaded| {
            Task::done(Message::ConfigLoaded(
                loaded.map(Box::new).map_err(Arc::new),
            ))
        })
    }

    pub(super) fn adopt_config(&mut self, config: Config) -> Task<Message> {
        self.typing_under_cards
            .adopt_config(&config.overlay_surfaces);
        let previous = mem::replace(&mut self.config, config);
        self.widget_groups.adopt_config(&previous, &self.config);
        self.theme = configured_theme(&self.config.theme.palette);
        Task::batch([
            self.notifications
                .withdraw_config_error(&self.config, self.clock.instant),
            self.adopt_fonts(&previous.theme),
            self.adopt_panel_geometry(&previous),
        ])
    }

    pub(super) fn adopt_fonts(&mut self, previous: &Theme) -> Task<Message> {
        let theme = &self.config.theme;
        if theme.font.has_same_face_as(&previous.font)
            && theme.monospace_font.family == previous.monospace_font.family
        {
            return Task::none();
        }
        FontFaces::point_generic_families_at(theme);
        font::load(self.config.theme.font.data.to_vec()).map(|loaded| {
            if let Err(e) = loaded {
                tracing::warn!("config reload: font not loaded: {e:?}");
            }
            Message::Redraw
        })
    }

    pub(super) fn adopt_panel_geometry(&mut self, previous: &Config) -> Task<Message> {
        let unchanged = self.config.desktop_panels == previous.desktop_panels
            && self.config.theme.spacing == previous.theme.spacing
            && (self.config.theme.font.text_size_px - previous.theme.font.text_size_px).abs()
                < f32::EPSILON
            && self.config.widget_group_layout == previous.widget_group_layout
            && self.config.group_items == previous.group_items
            && self.config.group_menus == previous.group_menus;
        if unchanged {
            return Task::none();
        }
        Task::batch([self.resize_backdrops(), self.reserve()])
    }

    pub(super) fn reject_config(&mut self, error: &LoadError) -> Task<Message> {
        let path = self.config_file.path();
        tracing::warn!("invalid configuration {}: {error}", path.display());
        self.notifications.show_config_error(
            path,
            error,
            &self.widget_groups.toast_context(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                self.clock,
            ),
        )
    }
}
