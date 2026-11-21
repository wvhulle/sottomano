use reqwest::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackTitle {
    Titled(String),
    Untitled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackAlbum {
    OnAlbum(String),
    NoAlbum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackSource {
    Located(Url),
    Unlocated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverArtUrl {
    Advertised(String),
    NotAdvertised,
}
