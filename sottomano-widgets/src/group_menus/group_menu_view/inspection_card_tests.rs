use iced::{Element, Rectangle, Settings, Size, widget::text};
use iced_test::Simulator;
use sottomano_toml_config::Config;

use super::group_menu_layout_tests::MenuTestBench;
use crate::{
    group_menus::menu_rows::menu_row::menu_metrics,
    hover_cards::{
        MouseGesture, PopupMessage, PressAction,
        hover_card_body::{LineOverflow, Section, SectionWidget},
    },
};

fn inspection_card_view(sections: Vec<Section>) -> Option<Element<'static, PopupMessage>> {
    let bench = MenuTestBench {
        sections,
        ..MenuTestBench::default()
    };
    bench.inspection_card().view(&bench.config)
}

#[test]
fn an_actions_only_card_is_shown_in_a_menu() {
    let actions_only = Section::press_actions(vec![PressAction::new(
        MouseGesture::LeftClick,
        "switch something",
    )]);
    assert!(inspection_card_view(actions_only.into_iter().collect()).is_some());
}

#[test]
fn a_card_without_sections_stays_hidden() {
    assert!(inspection_card_view(Vec::new()).is_none());
}

#[test]
fn a_card_taller_than_the_screen_keeps_its_rows_apart() {
    let tall_sections = (0..40)
        .map(|position| {
            Section::new(
                format!("Deadline {position}"),
                SectionWidget::paragraph(format!("task {position}")),
            )
        })
        .collect();
    let card = inspection_card_view(tall_sections).expect("a card with sections is shown");
    let mut simulator = Simulator::with_size(Settings::default(), Size::new(600.0, 300.0), card);
    let title_tops: Vec<f32> = (0..40)
        .map(|position| {
            simulator
                .find(format!("Deadline {position}"))
                .expect("the title is laid out")
                .bounds()
                .y
        })
        .collect();
    let line_height = menu_metrics(&Config::default()).row_height_px / 2.0;
    assert!(
        title_tops
            .windows(2)
            .all(|neighbours| neighbours[1] - neighbours[0] >= line_height)
    );
}

const UNBROKEN_TABLE_ROW: &str = "Monday 23 November 09:30 Design review in the north meeting room";

fn long_line_bounds(line_overflow: LineOverflow) -> Rectangle {
    let wide_section = Section::new(
        "Agenda",
        SectionWidget::drawn(|_| text(UNBROKEN_TABLE_ROW).into()),
    )
    .line_overflow(line_overflow);
    let card = inspection_card_view(vec![wide_section]).expect("a card with sections is shown");
    let mut simulator = Simulator::with_size(Settings::default(), Size::new(220.0, 400.0), card);
    simulator
        .find(UNBROKEN_TABLE_ROW)
        .expect("the long line is laid out")
        .bounds()
}

#[test]
fn a_sideways_scrolling_section_keeps_its_long_line_unwrapped() {
    let wrapped = long_line_bounds(LineOverflow::Wrap);
    let unwrapped = long_line_bounds(LineOverflow::ScrollSideways);
    assert!(wrapped.width <= 220.0, "wrapped {wrapped:?}");
    assert!(unwrapped.width > 220.0, "unwrapped {unwrapped:?}");
    assert!(
        unwrapped.height < wrapped.height,
        "wrapped {wrapped:?}, unwrapped {unwrapped:?}"
    );
}
