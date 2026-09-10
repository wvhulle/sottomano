use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
};

use iced::{
    Element, Subscription, Task,
    widget::{Row, button},
};

use crate::{
    config::{Config, SecretPaths},
    data::source::{
        command,
        wallpaper::{self, Position, Wallpaper},
    },
    message_subscriptions::poll_every,
    wayland_layers::{
        surface::Position as SurfacePosition,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        flat_button,
        font_awesome::Solid,
        last_poll::LastPoll,
        menu_group::{MenuGroup, MenuMessage},
        reading_row::{Reading, ReadingRow},
        simple_icons::SimpleIcon,
        source_reading::source_reading,
        tooltip_body::{Tooltip, TooltipBody, row, section},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Lookup(Wallpaper);

#[derive(Debug, Clone)]
pub enum Failure {
    Query(command::Error),
    Resolve(wallpaper::Error),
}

impl Display for Failure {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Failure::Query(error) => write!(f, "awww: {error}"),
            Failure::Resolve(error) => write!(f, "{error}"),
        }
    }
}

impl BarWidget for Lookup {
    fn icon(&self) -> Glyph {
        Solid::MountainSun.into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![row("File", self.0.file_name.clone())])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    name: String,
    position: Option<Position>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Article {
    title: String,
    url: String,
    summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Photo {
    file_name: String,
    photographer: Option<String>,
    caption: Option<String>,
    url: String,
}

impl BarWidget for Place {
    fn icon(&self) -> Glyph {
        Solid::LocationDot.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.name.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![row("Where", self.name.clone())];
        if let Some(position) = self.position {
            rows.push(row(
                "Coordinates",
                format!("{:.5}, {:.5}", position.latitude, position.longitude),
            ));
        }
        TooltipBody::rows(rows)
    }
}

impl BarWidget for Article {
    fn icon(&self) -> Glyph {
        SimpleIcon::Wikipedia.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.title.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        let mut sections = vec![section(
            "Wikipedia",
            vec![
                row("Article", self.title.clone()),
                row("Link", self.url.clone()),
            ],
        )];
        if let Some(summary) = &self.summary {
            sections.push(section("Summary", vec![row(" ", summary.clone())]));
        }
        TooltipBody::Sections(sections)
    }
}

impl BarWidget for Photo {
    fn icon(&self) -> Glyph {
        SimpleIcon::Unsplash.into()
    }

    fn label(&self) -> Option<String> {
        self.photographer.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        let mut credit = vec![row("File", self.file_name.clone())];
        if let Some(photographer) = &self.photographer {
            credit.push(row("Photographer", photographer.clone()));
        }
        credit.push(row("Unsplash", self.url.clone()));
        let mut sections = vec![section("Photo", credit)];
        if let Some(caption) = &self.caption {
            sections.push(section("Caption", vec![row(" ", caption.clone())]));
        }
        TooltipBody::Sections(sections)
    }
}

impl Place {
    fn of(lookup: &Lookup) -> Option<Self> {
        Some(Self {
            name: lookup.0.place.clone()?,
            position: lookup.0.position,
        })
    }
}

impl Article {
    fn of(lookup: &Lookup) -> Option<Self> {
        let article = lookup.0.article.as_ref()?;
        Some(Self {
            title: article.title.clone(),
            url: article.url.clone(),
            summary: article.summary.clone(),
        })
    }
}

impl Photo {
    fn of(lookup: &Lookup) -> Self {
        Self {
            file_name: lookup.0.file_name.clone(),
            photographer: lookup.0.photographer.clone(),
            caption: lookup.0.caption.clone(),
            url: lookup.0.photo_url.clone(),
        }
    }
}

pub struct WallpaperGroup {
    lookup: LastPoll<Lookup>,
    unsplash_key: Option<PathBuf>,
    wallpaper_daemon: bool,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum WallpaperMessage {
    DaemonFound(bool),
    Displayed(Result<String, command::Error>),
    Resolved(Result<Wallpaper, wallpaper::Error>),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
}

impl WallpaperGroup {
    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            lookup: LastPoll::default(),
            unsplash_key: secrets.unsplash_access_key.clone(),
            wallpaper_daemon: false,
            menu: MenuGroup::default(),
        }
    }

    pub fn probe(&self) -> Task<WallpaperMessage> {
        if self.unsplash_key.is_none() {
            return Task::none();
        }
        Task::future(async { WallpaperMessage::DaemonFound(wallpaper::daemon_installed().await) })
    }

    pub fn subscription(&self) -> Subscription<WallpaperMessage> {
        if self.unsplash_key.is_none() || !self.wallpaper_daemon {
            return Subscription::none();
        }
        poll_every(
            "wallpaper-displayed",
            wallpaper::QUERY_INTERVAL,
            wallpaper::displayed_path,
        )
        .map(WallpaperMessage::Displayed)
    }

    pub fn update(&mut self, message: WallpaperMessage) -> Task<WallpaperMessage> {
        match message {
            WallpaperMessage::DaemonFound(found) => self.wallpaper_daemon = found,
            WallpaperMessage::Displayed(Ok(path)) => return self.follow(path),
            WallpaperMessage::Displayed(Err(failure)) => {
                self.lookup.receive(Err(Failure::Query(failure)));
            }
            WallpaperMessage::Resolved(resolved) => {
                self.lookup
                    .receive(resolved.map(Lookup).map_err(Failure::Resolve));
            }
            WallpaperMessage::Menu(menu) => {
                self.menu.update(menu);
            }
            WallpaperMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    fn follow(&mut self, path: String) -> Task<WallpaperMessage> {
        let (Some(id), Some(key)) = (wallpaper::photo_id(&path), self.unsplash_key.clone()) else {
            self.lookup.load(None);
            return Task::none();
        };
        Task::future(async move { wallpaper::resolve(path, id, &key).await })
            .map(WallpaperMessage::Resolved)
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let body = match target {
            HoverTarget::Wallpaper => self.lookup.display()?.tooltip(),
            HoverTarget::WallpaperPlace => Place::of(self.lookup.get()?)?.tooltip(),
            HoverTarget::WallpaperArticle => Article::of(self.lookup.get()?)?.tooltip(),
            HoverTarget::WallpaperPhoto => Photo::of(self.lookup.get()?).tooltip(),
            _ => return None,
        };
        Some(Tooltip::of(body))
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        position: SurfacePosition,
    ) -> Option<Element<'a, WallpaperMessage>> {
        let handle_reading = source_reading(config, self.lookup.display(), 1.0)?;
        let handle = button(hoverable(
            handle_reading,
            HoverTarget::Wallpaper,
            WallpaperMessage::Tooltip,
        ))
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(WallpaperMessage::Menu(MenuMessage::Toggle))
        .into();

        let place = self.lookup.get().and_then(Place::of);
        let article = self.lookup.get().and_then(Article::of);
        let photo = self.lookup.get().map(Photo::of);
        let readings = ReadingRow::new()
            .opacity(self.menu.reveal())
            .push(Reading::new(
                place.as_ref().map(|pill| pill as &dyn BarWidget),
                HoverTarget::WallpaperPlace,
            ))
            .push(Reading::new(
                article.as_ref().map(|pill| pill as &dyn BarWidget),
                HoverTarget::WallpaperArticle,
            ))
            .push(Reading::new(
                photo.as_ref().map(|pill| pill as &dyn BarWidget),
                HoverTarget::WallpaperPhoto,
            ))
            .row(config, WallpaperMessage::Tooltip)
            .unwrap_or_else(Row::new);

        Some(self.menu.view(
            config,
            position,
            handle,
            readings.into(),
            WallpaperMessage::Menu,
        ))
    }
}
