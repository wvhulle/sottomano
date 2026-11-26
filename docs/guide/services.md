# Services

Bottom panel, right.

<div class="beside">
<div class="beside-text">

Hovering a CI run, then the Claude subscription windows.

</div>

<img src="https://sottomano.work/recordings/tour_services.webp" srcset="https://sottomano.work/recordings/tour_services.webp 2x" alt="Opening the Services menu, hovering a CI run and the Claude usage windows">

</div>

## Llama server

| Indicator | Press | Popup |
|---|---|---|
| llama-server | switch: start or stop llama-server | status and available models |
| Loaded model, per server | | configuration and GGUF metadata |
| Context fill, Generation rate, Busy slots, per server | | slots and throughput |

## CI runs

Runs of the Woodpecker and Forgejo runners on this computer.

| Indicator | Press | Popup |
|---|---|---|
| One row per run | right press: cancel the run | running step, image, CPU, memory, log link |

<div class="beside">
<div class="beside-text">

Hovering a Woodpecker run.

</div>

<img src="https://sottomano.work/recordings/ci_runs_task.webp" srcset="https://sottomano.work/recordings/ci_runs_task.webp 2x" alt="Opening the CI runs group from the Services menu and hovering a Woodpecker run">

</div>

## Claude usage

Budget under `[panel_menus.services.claude_usage]`.

| Indicator | Press | Popup |
|---|---|---|
| Subscription windows | | use per window and extra usage |
| 5-hour block spend | | cost estimate of the active block |
| API-priced spend | | spend against the monthly budget |
| Enterprise credit spend | | usage credits |

<div class="beside">
<div class="beside-text">

Hovering the Claude subscription windows.

</div>

<img src="https://sottomano.work/recordings/claude_usage_windows.webp" srcset="https://sottomano.work/recordings/claude_usage_windows.webp 2x" alt="Opening the AI usage group from the Services menu and hovering the Claude usage windows">

</div>

## Copilot usage

Budget under `[panel_menus.services.copilot_usage]`.

| Indicator | Press | Popup |
|---|---|---|
| Copilot quota, Copilot free quota | | subscription and remaining quota |
| Copilot CLI spend | | spend this month |

## OpenRouter credit

Budgets under `[panel_menus.services.openrouter_credit]`.

| Indicator | Press | Popup |
|---|---|---|
| OpenRouter soft budget, OpenRouter strict budget | | credits and key limits |
| Best models within price cap | | tool-capable models under the price cap |

## Systemd

Thresholds under `[panel_menus.services.systemd_manager]`.

| Indicator | Press | Popup |
|---|---|---|
| Boot timeline | | boot stages and startup units |
| Last shutdown | open the journal in a terminal | slowest stops |
| Failed systemd units | | units, with restart and journal buttons in the toast |
| Slow systemd jobs | | |

## Journal

A system and a user group. Pressing a group opens the journal in a terminal.

| Indicator | Press | Popup |
|---|---|---|
| Coredumps, Kernel log, OOM kills, Service errors | | the matching entries |

## Pending upgrades

| Indicator | Press | Popup |
|---|---|---|
| Available upgrades | | count |
| Firmware upgrades | run `fwupdmgr update` in a terminal | |
