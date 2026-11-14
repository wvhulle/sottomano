use corredo::popovers::PopupGrowth;
use sottomano_toml_config::screen_edges::Position;

#[must_use]
pub fn growth_away_from(bar: Position) -> PopupGrowth {
    match bar {
        Position::Bottom => PopupGrowth::Upward,
        Position::Top | Position::Left => PopupGrowth::Downward,
    }
}
