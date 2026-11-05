# corredo

Ready-to-use widgets for the [iced](https://github.com/iced-rs/iced) GUI library.
Every widget works with only its essential arguments and takes the rest from the
iced theme; builder methods change the defaults.

```rust,ignore
use corredo::popup_trigger::popup_trigger;
use iced::widget::{button, text};

popup_trigger(
    button(text("Open")).on_press(Message::Toggled),
    text("Anchored to the button"),
    self.popup_open,
)
.on_dismiss(Message::Dismissed)
```

## Widgets

| Widget | Module | Feature |
| --- | --- | --- |
| Popup attached to a trigger, with connector and morph transition | `popup_trigger` | |
| Popup attached to any rectangle | `anchored_popup` | |
| Record table with headings, groups, totals and truncation | `record_table` | |
| Content-sized table and grid | `content_sized_table` | |
| Drag-to-reorder pager with groups and an animated indicator | `pointer_drag` | |
| Text with placeholder, intro and replace effects | `transitioning_text` | |
| Context menu with notes, dividers and submenus | `context_menu` | |
| Slider that fits inside a row | `inline_slider` | |
| Seek bar that holds its target until playback arrives | `seek_bar` | |
| Button that asks for confirmation | `confirm_button` | |
| Text that copies or acts on press, with a brief flash | `pressable_text` | |
| Toggle switch, fill gauge, refresh dial, marker track | `toggle_switch`, `fill_gauge`, `refresh_dial`, `marker_track` | |
| Text that keeps the width of its widest value | `widest_readout_text` | |
| Scrolling list that follows a selection, with faded edges | `selection_scroller` | |
| Localized weekday picker | `weekday_picker` | `chrono` |
| Text body from a small markup language | `markup_body` | `markup` |
| Line of text swept over time, as in karaoke | `karaoke_line` | `karaoke` |

Smaller building blocks are in `primitives`.

## Features

No feature is on by default, so the default build depends only on iced crates. It needs
both of the renderers in iced's default features, `wgpu` and `tiny-skia`, because
the popup connector draws meshes and the scroll fade is a custom shader.

| Feature | Adds | Requires |
| --- | --- | --- |
| `chrono` | `weekday_picker` | `chrono` |
| `markup` | `markup_body` | `quick-xml` |
| `karaoke` | `karaoke_line` | `unicode-segmentation` |

## Examples

```sh
cargo run --example popup_trigger
cargo run --example reorderable_pager
cargo run --example widget_gallery --all-features
```
