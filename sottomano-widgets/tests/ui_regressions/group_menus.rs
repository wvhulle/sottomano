use sottomano_widget_scenes::{
    scenes::group_menu_scenes, script_steps::Step, scripted_session::ScriptedSession,
    showcase_program::booting_clock,
};
use sottomano_widgets::{group_menus::group_menu_state::GroupMenuTopic, hover_cards::HoverTarget};

use crate::snapshot_check::assert_matches_snapshot;

#[test]
fn hardware_menu_opens_compute_group() {
    let mut session = ScriptedSession::start(
        &group_menu_scenes::GROUP_MENU_ACCORDION_SEVERAL,
        booting_clock(),
    );
    session.perform(&Step::Press(HoverTarget::GroupMenu(
        GroupMenuTopic::Hardware,
    )));
    assert_matches_snapshot(&mut session, "group_menus/hardware_open");
    session.perform(&Step::PressText("Compute"));
    assert_matches_snapshot(&mut session, "group_menus/hardware_compute_expanded");
}
