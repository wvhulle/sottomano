use sottomano_widget_scenes::{
    scenes::{group_menu_scenes, indicator_group_scenes},
    scripted_session::ScriptedSession,
    showcase_program::booting_clock,
};

use crate::snapshot_check::assert_matches_snapshot;

#[test]
fn audio_face_rests_on_panel() {
    let mut session =
        ScriptedSession::start(&indicator_group_scenes::AUDIO_VOLUME, booting_clock());
    session.rest();
    assert_matches_snapshot(&mut session, "panel_faces/audio_resting");
}

#[test]
fn pinned_compute_face_rests_on_panel() {
    let mut session =
        ScriptedSession::start(&group_menu_scenes::COMPUTE_CPU_PINNED, booting_clock());
    session.rest();
    assert_matches_snapshot(&mut session, "panel_faces/compute_pinned_resting");
}
