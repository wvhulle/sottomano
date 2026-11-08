start_all()

RULES_FILES = {
    "niri": "niri.kdl",
    "sway": "sway.conf",
    "hyprland": "hyprland.lua",
}
RULES_PATH = f"/home/{SESSION_USER}/.config/sottomano/{RULES_FILES[COMPOSITOR]}"
DEFAULT_GAPS_PX = 16

try:
    with subtest("the compositor session starts"):
        wait_for_compositor()

    with subtest("sottomano writes the rule file of the running compositor"):
        wait_as_user(f"test -s {RULES_PATH}", timeout=60)
        print(as_user(f"cat {RULES_PATH}"))

    with subtest("the compositor accepts the rule file"):
        if COMPOSITOR == "niri":
            as_user("niri validate --config /etc/sottomano-test/niri.kdl")
        elif COMPOSITOR == "sway":
            as_user("sway --validate --config /etc/sottomano-test/sway.conf")
        else:
            config_errors = as_user("hyprctl configerrors")
            assert "sottomano" not in config_errors, config_errors

    if COMPOSITOR == "hyprland":
        with subtest("Hyprland applies the gaps from the rule file"):
            machine.wait_until_succeeds(
                user_command(
                    "hyprctl -j getoption general:gaps_out"
                    f" | jq -e '.css | split(\" \") | all(. == \"{DEFAULT_GAPS_PX}\")'"
                ),
                timeout=timedelta(seconds=30),
            )
    machine.screenshot("styling_rules")
except Exception:
    dump_session_state()
    raise
