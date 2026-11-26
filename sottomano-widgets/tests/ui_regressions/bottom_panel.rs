use sottomano_widget_scenes::{
    scenes::desktop_session_scenes, script_steps::Step, scripted_session::ScriptedSession,
    showcase_program::booting_clock,
};
use sottomano_widgets::hover_cards::HoverTarget;

use crate::snapshot_check::assert_matches_snapshot;

#[test]
fn hovered_workspace_shows_its_card() {
    let mut session = ScriptedSession::start(&desktop_session_scenes::BOTTOM_BAR, booting_clock());
    session.perform(&Step::Hover(HoverTarget::Workspace(2)));
    assert_matches_snapshot(&mut session, "bottom_panel/workspace_hovered");
}
