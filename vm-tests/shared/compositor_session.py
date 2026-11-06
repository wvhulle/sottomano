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


def compositor_query(query_name):
    if COMPOSITOR == "niri":
        return json.loads(as_user(f"niri msg --json {query_name}"))
    if COMPOSITOR == "sway":
        return json.loads(as_user(f"swaymsg --raw -t get_{query_name}"))
    return json.loads(as_user(f"hyprctl -j {query_name}"))


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


def dump_session_state():
    machine.execute(user_command("journalctl --user --no-pager -n 200 >&2"))
    machine.screenshot("failure_state")
