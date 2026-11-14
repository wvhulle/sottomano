# corredo

Ready-to-use widgets for the [iced](https://github.com/iced-rs/iced) GUI library.
Every widget works with only its essential arguments and takes the rest from the
iced theme; builder methods change the defaults.

```rust,ignore
use corredo::popovers::popup_trigger;
use iced::widget::{button, text};

popup_trigger(
    button(text("Open")).on_press(Message::Toggled),
    text("Anchored to the button"),
    self.popup_open,
)
.on_dismiss(Message::Dismissed)
```

## Widgets

Each module groups the widgets for one kind of job and exports their constructors and
types directly, as in `corredo::popovers::popup_trigger`.

| Module | Widgets | Feature |
| --- | --- | --- |
| `popovers` | `popup_trigger`: popup attached to a trigger, with connector and morph transition; `anchored_popup`: popup attached to any rectangle; `context_menu`: menu with notes, dividers and submenus | |
| `drag_reorder` | `reorderable_list`: drag-to-reorder list with groups and an animated highlight; `draggable_slot`, `drag_ghost` and the drag state machine | |
| `selection_scroll` | `selection_following_scroll`: scrolling list that follows a selection, with faded edges | |
| `value_controls` | `inline_slider`: slider that fits inside a row; `seek_bar`: seek bar that holds its target until playback arrives; `toggle_switch`; `weekday_picker`: localized weekday picker | `chrono` for `weekday_picker` |
| `press_actions` | `confirm_button`: button that asks for confirmation; `flash_on_press_text`: text that copies or acts on press, with a brief flash; `flat_button` | |
| `data_tables` | `grouped_table`: table with headings, groups, totals and truncation; `auto_width_table`: table and grid sized to their content | |
| `changing_text` | `animated_text`: text with placeholder, intro and replace effects; `steady_width_readout`: text that keeps the width of its widest value; `markup_text`: text body from a small markup language; `karaoke_line`: line of text swept over time | `markup`, `karaoke` |
| `gauges` | `fill_gauge`, `countdown_dial`, `color_dot_track`, `activity_line` | |
| `transitions` | `expand_transition`, `horizontal_reveal` / `vertical_reveal`, `sliding_pane_swap`: slide and fade between two panes | |
| `width_layout` | `equal_width_column`, `steady_width_label`, `fair_column_widths` | |
| `orientation` | `Axis` and `Edge`, the orientation vocabulary every directional widget takes | |
| `theming` | theme colour accessors, `FrameStyle`, font faces, `dotted_rule`, `tinted_svg_icon` | |

## Features

No feature is on by default, so the default build depends only on iced crates. It needs
both of the renderers in iced's default features, `wgpu` and `tiny-skia`, because
the popup connector draws meshes and the scroll fade is a custom shader.

| Feature | Adds | Requires |
| --- | --- | --- |
| `chrono` | `weekday_picker` | `chrono` |
| `markup` | `markup_text` | `quick-xml` |
| `karaoke` | `karaoke_line` | `unicode-segmentation` |

## Examples

```sh
cargo run --example popup_trigger
cargo run --example reorderable_list
cargo run --example widget_gallery --all-features
```
