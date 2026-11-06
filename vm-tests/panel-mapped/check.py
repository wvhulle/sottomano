import time

start_all()

try:
    with subtest("the compositor session starts"):
        wait_for_compositor()

    with subtest("the sottomano unit stays active"):
        wait_as_user("systemctl --user is-active sottomano.service", timeout=60)
        time.sleep(10)
        as_user("systemctl --user is-active sottomano.service")

    with subtest("the panel layer surface is mapped"):
        if COMPOSITOR == "sway":
            machine.wait_for_text("[0-9]{2}:[0-9]{2}", timeout=60)
        else:
            wait_for_layer_namespace("sottomano-panel")
        machine.screenshot("panel_mapped")
except Exception:
    dump_session_state()
    raise
