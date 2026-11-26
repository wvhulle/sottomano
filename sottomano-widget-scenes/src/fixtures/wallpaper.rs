use sottomano_desktop_sources::{
    SnapshotDelivery,
    wallpaper_place_article::unsplash_photo_article::{
        ArticleSummary, PhotoCaption, PhotoCredit, PhotoPlaceName, PhotoPosition, PlaceArticle,
        Position, Wallpaper, WikipediaArticle,
    },
};
use sottomano_toml_config::SecretPaths;
use sottomano_widgets::{
    clock::Clock,
    panel_menus::display_menu::wallpaper_group::{
        WallpaperGroup, WallpaperMessage, wallpaper_lookup::Lookup,
    },
};
use uom::si::{angle::degree, f64::Angle};

use crate::showcased_group::ShowcasedGroup;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = WallpaperGroup::new(&SecretPaths::default());
    let _ = group.update(
        WallpaperMessage::Lookup(Box::new(SnapshotDelivery::live_push(Ok(Lookup::from(
            &milos(),
        ))))),
        clock,
    );
    ShowcasedGroup::Wallpaper(Box::new(group))
}

fn milos() -> Wallpaper {
    Wallpaper {
        file_name: "pollonia-milos-unsplash.jpg".to_owned(),
        caption: PhotoCaption::Described("Chapel above the harbour of Pollonia".to_owned()),
        photographer: PhotoCredit::Photographer("Anastase Maragos".to_owned()),
        photo_url: "https://unsplash.com/photos/pollonia-milos".to_owned(),
        place: PhotoPlaceName::Named("Pollonia, Milos".to_owned()),
        position: PhotoPosition::Located(Position {
            latitude: Angle::new::<degree>(36.7613),
            longitude: Angle::new::<degree>(24.5226),
        }),
        article: PlaceArticle::Found(WikipediaArticle {
            title: "Pollonia, Milos".to_owned(),
            url: "https://en.wikipedia.org/wiki/Pollonia,_Milos".to_owned(),
            summary: ArticleSummary::Extract(
                "Pollonia or Apollonia is a seaside village in the north-eastern part of the \
                 island of Milos, Greece. It is the second harbour of Milos island after \
                 Adamantas and it has taken its name from the temple of Apollo that existed to \
                 the east of the settlement."
                    .to_owned(),
            ),
        }),
    }
}
