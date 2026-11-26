# Agenda

Top panel, middle.

<div class="beside">
<div class="beside-text">

Opening the Agenda menu and hovering the upcoming events, the rain and the location estimate.

</div>

<img src="https://sottomano.work/recordings/tour_agenda.webp" srcset="https://sottomano.work/recordings/tour_agenda.webp 2x" alt="Opening the Agenda menu, hovering the calendar events, the rain and the location estimate">

</div>

## Clock

The local time is pinned on the panel by default (`panel_menus.agenda.clock.local_time.pin`).

| Indicator | Press | Popup |
|---|---|---|
| Local time | | offset from UTC |
| Local date | | calendar |
| One row per zone in `calendar.other_time_zones` | | time zone |

## Calendar

Events and tasks over CalDAV, configured under `[panel_menus.agenda.caldav]`.

| Indicator | Press | Popup |
|---|---|---|
| Upcoming events | | agenda and links |
| Open tasks | | deadlines and other tasks |

<div class="beside">
<div class="beside-text">

Hovering the upcoming events and open tasks.

</div>

<img src="https://sottomano.work/recordings/calendar_events.webp" srcset="https://sottomano.work/recordings/calendar_events.webp 2x" alt="Opening the Agenda menu and hovering its events and tasks">

</div>

## Weather

At the position the Location group reports.

| Indicator | Press | Popup |
|---|---|---|
| Temperature forecast | | today's forecast by hour |
| Rain | | rain timeline for the coming hours |
| Wind | | speed, gusts and direction |
| Pollen, one row per allergen | | level and thresholds |

## Location

Where [situo](https://codeberg.org/wvhulle/situo) places you.

| Indicator | Press | Popup |
|---|---|---|
| Place | | position, how it was found, map links |
| Estimate | | which source decided the fix |
| Radio | | Wi-Fi scan, held-back access points, serving cells |
| Updates | | status and schedule |

<div class="beside">
<div class="beside-text">

Hovering the estimate.

</div>

<img src="https://sottomano.work/recordings/location_estimate.webp" srcset="https://sottomano.work/recordings/location_estimate.webp 2x" alt="Opening the location group from the Agenda menu and hovering the estimate">

</div>

## Quiet schedule

| Indicator | Press | Popup |
|---|---|---|
| Follow schedule | switch: start and end do not disturb on the daily schedule | next change |
| Quiet from, Quiet until | slider | |
| Screen off while quiet | slider | |
| Working days | toggle a day | schedule |

## Notifications

| Indicator | Press | Popup |
|---|---|---|
| Pending notifications | | counts |
| Do not disturb | switch: start or end do not disturb now | |
| Notification sound | switch: play a sound with each notification | |

## Focus session

Thresholds are under `[panel_menus.agenda.session_focus]`.

| Indicator | Press | Popup |
|---|---|---|
| Current focus | | session, alerts, window and projects |
| Timeline | | last 24 hours and settings |
| Context switches | | window switches and alerts |
| Daily focus budget | | screen, applications and projects |

<div class="beside">
<div class="beside-text">

Hovering the timeline of the last 24 hours.

</div>

<img src="https://sottomano.work/recordings/session_focus_timeline.webp" srcset="https://sottomano.work/recordings/session_focus_timeline.webp 2x" alt="Pressing the focus session in the panel and hovering its timeline">

</div>
