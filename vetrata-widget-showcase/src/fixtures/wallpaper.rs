use uom::si::{angle::degree, f64::Angle};
use vetrata_config::SecretPaths;
use vetrata_sources::{
    SourceDelivery,
    wallpaper::{Position, Wallpaper, WikipediaArticle},
};
use vetrata_widgets::widget_groups::wallpaper_group::{Lookup, WallpaperGroup, WallpaperMessage};

use crate::showcased_group::ShowcasedGroup;

pub fn group() -> ShowcasedGroup {
    let mut group = WallpaperGroup::new(&SecretPaths::default());
    let _ = group.update(WallpaperMessage::Lookup(Box::new(SourceDelivery::Reading(
        Ok(Lookup::of(&milos())),
    ))));
    ShowcasedGroup::Wallpaper(Box::new(group))
}

fn milos() -> Wallpaper {
    Wallpaper {
        file_name: "pollonia-milos-unsplash.jpg".to_owned(),
        caption: Some("Chapel above the harbour of Pollonia".to_owned()),
        photographer: Some("Anastase Maragos".to_owned()),
        photo_url: "https://unsplash.com/photos/pollonia-milos".to_owned(),
        place: Some("Pollonia, Milos".to_owned()),
        position: Some(Position {
            latitude: Angle::new::<degree>(36.7613),
            longitude: Angle::new::<degree>(24.5226),
        }),
        article: Some(WikipediaArticle {
            title: "Pollonia, Milos".to_owned(),
            url: "https://en.wikipedia.org/wiki/Pollonia,_Milos".to_owned(),
            summary: Some(
                "Pollonia or Apollonia is a seaside village in the north-eastern part of the \
                 island of Milos, Greece. It is the second harbour of Milos island after \
                 Adamantas and it has taken its name from the temple of Apollo that existed to \
                 the east of the settlement."
                    .to_owned(),
            ),
        }),
    }
}
