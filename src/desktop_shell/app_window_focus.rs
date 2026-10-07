use crate::data::source::niri::WindowOwner;

#[derive(Debug, Clone)]
pub struct AppWindowFocus<M> {
    pub owner: WindowOwner,
    pub unmatched: M,
}
