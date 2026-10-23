# iced_controller_diagram

An [iced](https://iced.rs) widget that draws a game controller and labels its
buttons and sticks with your own elements, joined to them by leader lines.

```rust
use iced::widget::text;
use iced_controller_diagram::{DiagramButton, DiagramStick, FaceStyle, controller_diagram};

let diagram = controller_diagram(FaceStyle::PlayStation)
    .scale(0.5)
    .callout(DiagramStick::Left, text("navigate"))
    .callout(DiagramButton::South, text("confirm"))
    .callout(DiagramButton::RightTrigger, text("click"));
```

Labels are placed in a column on each side of the controller, in the vertical
order of their buttons, and never overlap. Buttons are named by position, as in
SDL's gamepad API (`South` is Cross on PlayStation and A on Xbox); the face
overlay shows the symbols of the chosen `FaceStyle`.

Run the gallery with
`cargo run -p iced_controller_diagram --example controller_diagram_gallery`.

## Requirements

The controller artwork is PNG. Enable PNG decoding in your application: iced's
`image` feature, or `image-without-codecs` together with the `image` crate's
`png` feature.

## Artwork and license

The code is MIT licensed (see `LICENSE`).

The controller images and button coordinates come from SDL's controller test
program (`test/gamepad_*.png` and `test/gamepadutils.c` in SDL 3.4.16), under
SDL's zlib license (see `LICENSE-SDL`). `assets/gamepad_front.png` is altered:
its white background was made transparent. The face overlays are unchanged.
