use sottomano_widget_scenes::{
    scenes::menu_tour_scenes,
    script_steps::{Step, indicator_row},
    scripted_session::ScriptedSession,
    showcase_program::booting_clock,
};
use sottomano_widgets::{
    group_indicators::indicator_group_identity::IndicatorGroupFamily,
    group_menus::group_menu_state::GroupMenuTopic, hover_cards::HoverTarget,
};

use crate::snapshot_check::assert_matches_snapshot;

#[test]
fn weather_forecast_card_shows_on_hover() {
    let mut session = ScriptedSession::start(&menu_tour_scenes::WEATHER_IN_AGENDA, booting_clock());
    session.perform(&Step::Press(HoverTarget::GroupMenu(GroupMenuTopic::Agenda)));
    session.perform(&Step::PressText("Weather"));
    session.perform(&Step::Hover(indicator_row(
        IndicatorGroupFamily::Weather,
        "Temperature forecast",
    )));
    assert_matches_snapshot(&mut session, "hover_cards/weather_forecast");
}
