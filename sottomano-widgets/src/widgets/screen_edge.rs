use corredo::orientation::Edge;
use sottomano_toml_config::screen_edges::Position;

#[must_use]
pub fn screen_edge_of(position: Position) -> Edge {
    match position {
        Position::Top => Edge::Top,
        Position::Bottom => Edge::Bottom,
        Position::Left => Edge::Left,
    }
}
