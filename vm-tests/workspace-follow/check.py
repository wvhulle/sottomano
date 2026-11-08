start_all()

try:
    with subtest("the compositor session and the bar start"):
        wait_for_compositor()
        wait_as_user("systemctl --user is-active sottomano.service", timeout=60)

    with subtest("a window on the first workspace shows in the bar"):
        spawn_named_window("alpha")
        wait_for_focused_window("alpha")
        machine.wait_for_text("alpha", timeout=timedelta(seconds=60))

    with subtest("a window on the second workspace shows in the bar after switching"):
        focus_workspace_number(2)
        spawn_named_window("bravo")
        wait_for_focused_window("bravo")
        machine.wait_for_text("bravo", timeout=timedelta(seconds=60))

    with subtest("switching back brings the first window back into the bar"):
        focus_workspace_number(1)
        wait_for_focused_window("alpha")
        machine.wait_for_text("alpha", timeout=timedelta(seconds=60))
        machine.screenshot("workspace_follow")
except Exception:
    dump_session_state()
    raise
