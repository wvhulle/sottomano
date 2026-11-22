use super::*;

#[test]
fn default_bindings_survive_a_toml_round_trip() {
    let defaults = GamepadBindingsConfig::default();
    let printed = toml::to_string(&defaults).unwrap();
    assert_eq!(
        toml::from_str::<GamepadBindingsConfig>(&printed).unwrap(),
        defaults
    );
}

#[test]
fn playstation_and_xbox_names_select_the_same_buttons() {
    let config: GamepadBindingsConfig = toml::from_str(
        "[typed_keys]\ncross = \"Return\"\nb = \"Escape\"\ncreate = \"Tab\"\nmenu = \
         \"space\"\n[compositor_actions]\nl1 = \"move_left\"\nps = \"toggle_overview\"\n",
    )
    .unwrap();
    assert_eq!(
        config.claimed_role(GamepadButton::Back),
        Some(ButtonRole::TypedKey("Tab"))
    );
    assert_eq!(
        config.claimed_role(GamepadButton::Start),
        Some(ButtonRole::TypedKey("space"))
    );
    assert_eq!(
        config.claimed_role(GamepadButton::South),
        Some(ButtonRole::TypedKey("Return"))
    );
    assert_eq!(
        config.claimed_role(GamepadButton::East),
        Some(ButtonRole::TypedKey("Escape"))
    );
    assert_eq!(
        config.claimed_role(GamepadButton::LeftShoulder),
        Some(ButtonRole::Compositor(CompositorAction::MoveLeft))
    );
    assert_eq!(
        config.claimed_role(GamepadButton::Guide),
        Some(ButtonRole::Compositor(CompositorAction::ToggleOverview))
    );
}

#[test]
fn each_section_claims_buttons_for_its_purpose() {
    let config: GamepadBindingsConfig = toml::from_str(
        "[compositor_actions]\nwest = \"close_window\"\n[commands]\nstart = [\"notify-send\", \
         \"hi\"]\n",
    )
    .unwrap();
    assert_eq!(
        config.claimed_role(GamepadButton::West),
        Some(ButtonRole::Compositor(CompositorAction::CloseWindow))
    );
    assert_eq!(
        config.claimed_role(GamepadButton::Start),
        Some(ButtonRole::Command(&[
            "notify-send".to_owned(),
            "hi".to_owned()
        ]))
    );
    assert_eq!(
        config.claimed_role(GamepadButton::DpadLeft),
        Some(ButtonRole::TypedKey("Left"))
    );
}

#[test]
fn the_defaults_claim_every_button_at_most_once() {
    let config = GamepadBindingsConfig::default();
    assert!(config.conflicting_buttons().is_empty());
    assert_eq!(config.stick_sharing(), StickSharing::Separate);
}

#[test]
fn a_button_in_two_sections_is_reported_and_the_first_purpose_wins() {
    let mut config = GamepadBindingsConfig::default();
    config
        .compositor_actions
        .insert(GamepadButton::LeftTrigger, CompositorAction::CloseWindow);
    assert_eq!(
        config.conflicting_buttons(),
        BTreeSet::from([GamepadButton::LeftTrigger])
    );
    assert_eq!(
        config.claimed_role(GamepadButton::LeftTrigger),
        Some(ButtonRole::PushToTalk)
    );
}

#[test]
fn the_right_trigger_holds_the_left_mouse_button() {
    let steering = GamepadBindingsConfig::default().pointer_steering();
    assert_eq!(
        steering.buttons.get(&GamepadButton::RightTrigger),
        Some(&PointerButton::Left)
    );
    assert!(matches!(
        steering.stick,
        PointerStickBinding::Bound(PointerStickSettings {
            stick: GamepadStick::Right,
            ..
        })
    ));
}

#[test]
fn the_right_stick_press_holds_the_right_mouse_button() {
    let steering = GamepadBindingsConfig::default().pointer_steering();
    assert_eq!(
        steering.buttons.get(&GamepadButton::RightStick),
        Some(&PointerButton::Right)
    );
}
