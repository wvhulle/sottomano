import json
import shlex
from datetime import timedelta

SESSION_USER = "session-user"
RUNTIME_DIRECTORY = "/run/user/1000"


def compositor_environment():
    if COMPOSITOR == "niri":
        return f"NIRI_SOCKET=$(ls {RUNTIME_DIRECTORY}/niri.*.sock | head -n1)"
    if COMPOSITOR == "sway":
        return "SWAYSOCK=/tmp/sway-ipc.sock"
    return f"HYPRLAND_INSTANCE_SIGNATURE=$(ls {RUNTIME_DIRECTORY}/hypr | head -n1)"


def user_command(command):
    quoted = shlex.quote(f"export {compositor_environment()}; {command}")
    return (
        f"runuser -u {SESSION_USER} -- env XDG_RUNTIME_DIR={RUNTIME_DIRECTORY} "
        f"DBUS_SESSION_BUS_ADDRESS=unix:path={RUNTIME_DIRECTORY}/bus "
        f"sh -c {quoted}"
    )


def as_user(command):
    return machine.succeed(user_command(command))


def wait_as_user(command, timeout=60):
    return machine.wait_until_succeeds(user_command(command), timeout=timedelta(seconds=timeout))


def wait_for_compositor():
    machine.wait_for_unit("multi-user.target")
    wait_as_user(
        {
            "niri": "niri msg --json outputs",
            "sway": "swaymsg -t get_outputs",
            "hyprland": "hyprctl -j monitors",
        }[COMPOSITOR],
        timeout=120,
    )
    wait_as_user("systemctl --user is-active graphical-session.target", timeout=60)


def wait_for_layer_namespace(namespace, timeout=60):
    machine.wait_until_succeeds(
        user_command(
            {
                "niri": "niri msg --json layers",
                "hyprland": "hyprctl -j layers",
            }[COMPOSITOR]
            + f" | jq -e '[.. | .namespace? // empty] | index({json.dumps(namespace)})'"
        ),
        timeout=timedelta(seconds=timeout),
    )


def spawn_named_window(window_name):
    command = f"foot --app-id={window_name} --title={window_name} sleep infinity"
    as_user(
        {
            "niri": f"niri msg action spawn -- {command}",
            "sway": f"swaymsg exec {shlex.quote(command)}",
            "hyprland": "hyprctl dispatch "
            + shlex.quote(f"hl.dsp.exec_cmd({json.dumps(command)})"),
        }[COMPOSITOR]
    )


def focus_workspace_number(number):
    as_user(
        {
            "niri": f"niri msg action focus-workspace {number}",
            "sway": f"swaymsg workspace number {number}",
            "hyprland": "hyprctl dispatch "
            + shlex.quote(f"hl.dsp.focus({{ workspace = {number} }})"),
        }[COMPOSITOR]
    )


FOCUSED_WINDOW_NAME_QUERIES = {
    "niri": "niri msg --json focused-window | jq -r '.app_id'",
    "sway": "swaymsg --raw -t get_tree | jq -r '.. | objects | select(.focused == true) | .app_id'",
    "hyprland": "hyprctl -j activewindow | jq -r '.class'",
}


def wait_for_focused_window(window_name, timeout=60):
    wait_as_user(
        f"{FOCUSED_WINDOW_NAME_QUERIES[COMPOSITOR]} | grep -qx {shlex.quote(window_name)}",
        timeout=timeout,
    )


COMPOSITOR_STATE_QUERIES = {
    "niri": ["niri msg --json windows", "niri msg --json workspaces", "niri msg --json outputs"],
    "sway": ["swaymsg --raw -t get_tree", "swaymsg --raw -t get_workspaces"],
    "hyprland": ["hyprctl -j clients", "hyprctl -j workspaces", "hyprctl -j monitors"],
}


def dump_session_state():
    for query in COMPOSITOR_STATE_QUERIES[COMPOSITOR]:
        machine.execute(user_command(f"{query} >&2"))
    machine.execute(user_command("journalctl --user --no-pager -n 200 >&2"))
    machine.screenshot("failure_state")
