use sottomano_widget_scenes::{
    scenes::desktop_session_scenes, script_steps::Step, scripted_session::ScriptedSession,
    showcase_program::booting_clock,
};

use crate::snapshot_check::assert_matches_snapshot;

#[test]
fn arriving_toast_settles_in_its_corner() {
    let mut session =
        ScriptedSession::start(&desktop_session_scenes::TOAST_ARRIVAL, booting_clock());
    session.rest();
    assert_matches_snapshot(&mut session, "toasts/arrival_settled");
}

#[test]
fn hovered_toast_action_highlights() {
    let mut session =
        ScriptedSession::start(&desktop_session_scenes::TOAST_ACTIONS, booting_clock());
    session.perform(&Step::HoverText("Later"));
    assert_matches_snapshot(&mut session, "toasts/action_hovered");
}
