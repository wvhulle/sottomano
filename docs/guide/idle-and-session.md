# Idle and session

When the screen turns off and locks, what the session runs on, and how long you have been focused.

## Idle timeouts: Screen off

<div class="beside">
<div class="beside-text">

Each timer adds to the previous enabled one in the order lock, screen off, standby, hibernate, power-down; the slider names the step it counts from and the card says when it fires and through which mechanism.

</div>

<img src="https://sottomano.work/recordings/idle_screen_off.webp" srcset="https://sottomano.work/recordings/idle_screen_off.webp 2x" alt="Opening the idle timeouts from the Session menu and hovering the screen-off timer">

</div>

## Idle timeouts: Other timers

These timers work like the screen-off timer above.

| Timer | What it shows |
|---|---|
| Lock | The first step, counted from the last input. |
| Standby | Suspends; a row names the logind call: suspend, suspend-then-hibernate or hibernate. |
| Hibernate | With standby on, the time spent asleep before hibernating, written as systemd's `HibernateDelaySec` and shown next to the system's current value. With standby off, a regular idle step. |
| Power-down | Marked unreachable while standby or hibernate is on, since the machine is asleep by then. |

## Login session: Kernel and compositor

<div class="beside">
<div class="beside-text">

The kernel and the compositor, each with a logo and a wiki link.

</div>

<img src="https://sottomano.work/recordings/login_session_kernel.webp" srcset="https://sottomano.work/recordings/login_session_kernel.webp 2x" alt="Opening the login session group from the Session menu and hovering the kernel and compositor">

</div>

## Focus session: Timeline

<div class="beside">
<div class="beside-text">

The last 24 hours on one timeline: active stretches, breaks, idle and screen-off moments, and the warning and critical alerts still ahead, followed by the settings that decide when a pause counts as a break and when the alerts fire.

</div>

<img src="https://sottomano.work/recordings/focus_timeline.webp" srcset="https://sottomano.work/recordings/focus_timeline.webp 2x" alt="Pressing the focus session in the panel and hovering its timeline">

</div>
