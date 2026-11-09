use corredo::anchored_popup::{AnchoredPopup, PopupArrival, PopupRegions, PopupSide};
pub use corredo::anchored_popup::{OutsidePress, PopupFraming, PopupRendering};
use iced::{
    Element, Point, Rectangle, Size, advanced::widget::Operation, border::Radius, time::Duration,
};
use sottomano_toml_config::{Config, screen_edges::Position};

use crate::{
    desktop_panels::region_reporter::{
        BackdropBlur, PointerInput, RegionRoles, report_input_across_except,
    },
    hover_cards::hover_card_transition::ShownTarget,
    widgets::{shell_layer_appearance::LayerAppearance, typing_under_cards::CardBackgroundFade},
};

#[derive(Debug, Clone, Copy)]
pub struct PopupAppearances {
    pub overlay: LayerAppearance,
    pub menu: LayerAppearance,
    pub background_fade: CardBackgroundFade,
}

pub struct PressCatcher<Message> {
    pub outside_press: OutsidePress<Message>,
    pub catch_area_holes: Vec<Rectangle>,
}

impl<Message> PressCatcher<Message> {
    #[must_use]
    pub fn without_holes(outside_press: OutsidePress<Message>) -> Self {
        Self {
            outside_press,
            catch_area_holes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PopupMorph {
    pub duration: Duration,
    pub shown: ShownTarget,
}

fn popup_side_of(position: Position) -> PopupSide {
    match position {
        Position::Top => PopupSide::Below,
        Position::Bottom => PopupSide::Above,
        Position::Left => PopupSide::Right,
    }
}

fn report_shell_regions(
    regions: &PopupRegions,
    backdrop_blur: BackdropBlur,
    catch_area_holes: &[Rectangle],
    operation: &mut dyn Operation,
) {
    let roles = RegionRoles {
        pointer_input: if regions.takes_input {
            PointerInput::Accepted
        } else {
            PointerInput::PassedThrough
        },
        backdrop_blur,
        corner_radius: Radius::default(),
    };
    if regions.captures_outside_presses {
        let bounds = regions.bounds;
        report_input_across_except(
            Rectangle::new(
                Point::ORIGIN,
                Size::new(
                    2.0f32.mul_add(bounds.x, bounds.width),
                    2.0f32.mul_add(bounds.y, bounds.height),
                ),
            ),
            catch_area_holes,
            operation,
        );
    }
    roles.report(&regions.surface_rows, regions.bounds, operation);
}

#[must_use]
pub fn anchored_popup<'a, Message: Clone + 'a>(
    renderings: Vec<PopupRendering<'a, Message>>,
    anchor: Rectangle,
    position: Position,
    morph: PopupMorph,
    press_catcher: PressCatcher<Message>,
    config: &Config,
    appearances: PopupAppearances,
) -> Element<'a, Message> {
    let PressCatcher {
        outside_press,
        catch_area_holes,
    } = press_catcher;
    let shows_menu = renderings.iter().any(|rendering| {
        rendering.arrival == PopupArrival::Incoming
            && matches!(rendering.framing, PopupFraming::LeadingSurface(_))
    });
    let appearance = if shows_menu {
        appearances.menu
    } else {
        appearances.overlay
    };
    let background_fade = appearances.background_fade;
    let appearance_over = move |popup: Rectangle| {
        appearance.with_background_faded(background_fade.fade_level_over(popup))
    };
    let shown = match morph.shown {
        ShownTarget::Nothing => None,
        target @ ShownTarget::Target(_) => Some(target),
    };
    AnchoredPopup::with_renderings(renderings, anchor)
        .side(popup_side_of(position))
        .gap(config.theme.spacing.group_gap)
        .docked_at_inset(config.desktop_panels.height)
        .leading_alignment(
            config.theme.spacing.surface_inset + config.theme.font.icon_size_px / 2.0,
        )
        .morph_duration(morph.duration)
        .frame_over(move |popup| appearance_over(popup).frame_style())
        .outside_press(outside_press)
        .report_regions(move |regions, operation| {
            let backdrop_blur = appearance_over(regions.popup_bounds).backdrop_blur;
            report_shell_regions(&regions, backdrop_blur, &catch_area_holes, operation);
        })
        .identified_by(shown)
        .into()
}

#[cfg(test)]
mod tests {
    use iced::{
        Element, Length, Point, Rectangle, Settings, Size,
        mouse::{Button, Event as MouseEvent},
        time::{Duration, Instant},
        widget::{Space, mouse_area, stack, text},
        window,
    };
    use iced_test::Simulator;
    use sottomano_toml_config::{Config, screen_edges::Position};

    use super::{
        CardBackgroundFade, OutsidePress, PopupAppearances, PopupFraming, PopupMorph,
        PopupRendering, PressCatcher, anchored_popup,
    };
    use crate::{
        clock::Clock,
        group_menus::cascading_layout::{
            cascade_row::{CascadeDirection, CascadeOrientation, cascade_row},
            cascade_satellite_panels::CascadeSatellitePanels,
            notched_panel::PanelGrowth,
        },
        hover_cards::{HoverTarget, hover_card_transition::ShownTarget},
        widgets::shell_layer_appearance::ShellLayer,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct PressedOutside;

    const CANVAS_SIZE: Size = Size::new(1200.0, 800.0);
    const ORIENTATION: CascadeOrientation = CascadeOrientation {
        direction: CascadeDirection::Rightward,
        growth: PanelGrowth::Downward,
    };

    fn redraw_at(now: Instant) -> iced::Event {
        iced::Event::Window(window::Event::RedrawRequested(now))
    }

    fn settle_morph(simulator: &mut Simulator<'_, PressedOutside>) {
        let started = Clock::real().instant;
        let _settled = simulator.simulate([
            redraw_at(started),
            redraw_at(started + Duration::from_secs(1)),
        ]);
    }

    fn messages_after_pressing(
        popup: Element<'_, PressedOutside>,
        pressed_at: impl FnOnce(&mut Simulator<'_, PressedOutside>) -> Point,
    ) -> Vec<PressedOutside> {
        let mut simulator = Simulator::with_size(Settings::default(), CANVAS_SIZE, popup);
        settle_morph(&mut simulator);
        let pointer = pressed_at(&mut simulator);
        simulator.point_at(pointer);
        let _pressed =
            simulator.simulate([iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left))]);
        simulator.into_messages().collect()
    }

    fn center_of_text(simulator: &mut Simulator<'_, PressedOutside>, shown: &str) -> Point {
        simulator
            .find(shown)
            .expect("the text is on screen")
            .visible_bounds()
            .expect("the text is visible")
            .center()
    }

    fn workspace_morph() -> PopupMorph {
        PopupMorph {
            duration: Duration::from_millis(10),
            shown: ShownTarget::Target(HoverTarget::Workspace(7)),
        }
    }

    const HANDLE_BELOW_POPUP: Rectangle =
        Rectangle::new(Point::new(1000.0, 700.0), Size::new(40.0, 40.0));

    fn cascading_popup(config: &Config) -> Element<'_, PressedOutside> {
        let appearance = ShellLayer::Overlays.default_appearance();
        let satellite_panels =
            CascadeSatellitePanels::new(&appearance, &appearance, config, ORIENTATION);
        let satellite = satellite_panels.beside_row(config, text("satellite").into(), 0.0);
        anchored_popup(
            vec![
                PopupRendering::incoming(cascade_row(
                    text("primary").into(),
                    [satellite],
                    ORIENTATION,
                ))
                .framing(PopupFraming::LeadingSurface(ORIENTATION)),
            ],
            Rectangle::new(Point::new(20.0, 0.0), Size::new(20.0, 20.0)),
            Position::Top,
            workspace_morph(),
            PressCatcher::without_holes(OutsidePress::Dismisses {
                dismissal: PressedOutside,
                reachable_below: vec![HANDLE_BELOW_POPUP],
            }),
            config,
            PopupAppearances {
                overlay: appearance,
                menu: appearance,
                background_fade: CardBackgroundFade::Resting,
            },
        )
    }

    fn messages_from_pointing_at_layers_below(
        pointed_at: impl FnOnce(&mut Simulator<'_, PressedOutside>) -> Point,
    ) -> Vec<PressedOutside> {
        let config = Config::default();
        let panel_below = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
            .on_enter(PressedOutside);
        let layers: Element<'_, PressedOutside> =
            stack![panel_below, cascading_popup(&config)].into();
        let mut simulator =
            Simulator::with_size(Settings::default(), Size::new(600.0, 400.0), layers);
        settle_morph(&mut simulator);
        let pointer = pointed_at(&mut simulator);
        simulator.point_at(pointer);
        let _moved = simulator.simulate([iced::Event::Mouse(MouseEvent::CursorMoved {
            position: pointer,
        })]);
        simulator.into_messages().collect()
    }

    #[test]
    fn a_pointer_on_the_card_does_not_reach_the_panel_below() {
        let messages = messages_from_pointing_at_layers_below(|simulator| {
            center_of_text(simulator, "primary")
        });
        assert_eq!(messages, []);
    }

    #[test]
    fn a_pointer_beside_the_card_reaches_the_panel_below() {
        let messages = messages_from_pointing_at_layers_below(|_| Point::new(590.0, 390.0));
        assert_eq!(messages, [PressedOutside]);
    }

    #[test]
    fn a_press_on_the_primary_surface_keeps_the_popup() {
        let config = Config::default();
        let messages = messages_after_pressing(cascading_popup(&config), |simulator| {
            center_of_text(simulator, "primary")
        });
        assert_eq!(messages, []);
    }

    #[test]
    fn a_press_on_a_satellite_keeps_the_popup() {
        let config = Config::default();
        let messages = messages_after_pressing(cascading_popup(&config), |simulator| {
            center_of_text(simulator, "satellite")
        });
        assert_eq!(messages, []);
    }

    #[test]
    fn a_press_outside_every_surface_dismisses_the_popup() {
        let config = Config::default();
        let messages = messages_after_pressing(cascading_popup(&config), |_| {
            Point::new(CANVAS_SIZE.width - 1.0, CANVAS_SIZE.height - 1.0)
        });
        assert_eq!(messages, [PressedOutside]);
    }

    #[test]
    fn a_press_outside_before_the_first_frame_keeps_the_popup() {
        let config = Config::default();
        let mut simulator =
            Simulator::with_size(Settings::default(), CANVAS_SIZE, cascading_popup(&config));
        simulator.point_at(Point::new(
            CANVAS_SIZE.width - 1.0,
            CANVAS_SIZE.height - 1.0,
        ));
        let _pressed =
            simulator.simulate([iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left))]);
        assert_eq!(simulator.into_messages().collect::<Vec<_>>(), []);
    }

    #[test]
    fn the_morph_clock_starts_at_the_first_frame() {
        let config = Config::default();
        let mut simulator =
            Simulator::with_size(Settings::default(), CANVAS_SIZE, cascading_popup(&config));
        let first_frame = Clock::real().instant + Duration::from_secs(60);
        let _first_frame = simulator.simulate([redraw_at(first_frame)]);
        simulator.point_at(Point::new(
            CANVAS_SIZE.width - 1.0,
            CANVAS_SIZE.height - 1.0,
        ));
        let _pressed =
            simulator.simulate([iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left))]);
        assert_eq!(simulator.into_messages().collect::<Vec<_>>(), []);
    }

    #[test]
    fn a_press_on_a_reachable_handle_leaves_it_to_the_handle() {
        let config = Config::default();
        let messages =
            messages_after_pressing(cascading_popup(&config), |_| HANDLE_BELOW_POPUP.center());
        assert_eq!(messages, []);
    }

    #[test]
    fn a_press_in_the_gap_before_a_satellite_keeps_the_popup() {
        let config = Config::default();
        let messages = messages_after_pressing(cascading_popup(&config), |simulator| {
            let satellite_text = simulator
                .find("satellite")
                .expect("the text is on screen")
                .visible_bounds()
                .expect("the text is visible");
            Point::new(satellite_text.x - 1.0, satellite_text.center_y())
        });
        assert_eq!(messages, []);
    }

    #[test]
    fn a_press_on_a_surface_with_nothing_shown_dismisses_the_menu_elsewhere() {
        let config = Config::default();
        let popup = anchored_popup(
            Vec::new(),
            Rectangle::default(),
            Position::Top,
            PopupMorph {
                duration: Duration::from_millis(10),
                shown: ShownTarget::Nothing,
            },
            PressCatcher::without_holes(OutsidePress::Dismisses {
                dismissal: PressedOutside,
                reachable_below: Vec::new(),
            }),
            &config,
            PopupAppearances {
                overlay: ShellLayer::Overlays.default_appearance(),
                menu: ShellLayer::Overlays.default_appearance(),
                background_fade: CardBackgroundFade::Resting,
            },
        );
        let messages = messages_after_pressing(popup, |_| Point::new(500.0, 500.0));
        assert_eq!(messages, [PressedOutside]);
    }

    #[test]
    fn a_hovered_popup_ignores_a_press_outside() {
        let config = Config::default();
        let appearance = ShellLayer::Overlays.default_appearance();
        let popup = anchored_popup(
            vec![PopupRendering::incoming(text("card").into())],
            Rectangle::new(Point::new(20.0, 0.0), Size::new(20.0, 20.0)),
            Position::Top,
            workspace_morph(),
            PressCatcher::without_holes(OutsidePress::PassesThrough),
            &config,
            PopupAppearances {
                overlay: appearance,
                menu: appearance,
                background_fade: CardBackgroundFade::Resting,
            },
        );
        let messages = messages_after_pressing(popup, |_| {
            Point::new(CANVAS_SIZE.width - 1.0, CANVAS_SIZE.height - 1.0)
        });
        assert_eq!(messages, []);
    }
}
