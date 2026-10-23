# iced_controller_diagram

An [iced](https://iced.rs) widget that draws a game controller and labels its
buttons and sticks with your own elements, joined to them by leader lines.

```rust
use iced::widget::text;
use iced_controller_diagram::{
    ControllerArtwork, ControllerModel, DiagramButton, DiagramStick, controller_diagram,
};

let artwork = ControllerArtwork::decoded();

let diagram = controller_diagram(artwork.clone(), ControllerModel::DualSense)
    .body_width(280.0)
    .callout(DiagramStick::Left, text("navigate"))
    .callout(DiagramButton::South, text("confirm"))
    .callout(DiagramButton::RightTrigger, text("click"));
```

Create the `ControllerArtwork` once and keep it in your application state, then
pass a clone to the diagram in `view`. It holds the parsed drawings; cloning is
cheap and keeps their identity, so iced reuses the same rasterisations every
frame.

The controller is drawn from a vector front view of the chosen
`ControllerModel` (DualSense, Xbox Series or Switch Pro), so it stays sharp at
any width and scale factor. Labels are placed in a column on each side of the
controller, in the vertical order of their buttons, and never overlap. Buttons
are named by position, as in SDL's gamepad API (`South` is Cross on PlayStation,
A on Xbox and B on Switch).

Run the gallery with
`cargo run -p iced_controller_diagram --example controller_diagram_gallery`.

## Artwork and license

The code is MIT licensed (see `LICENSE`).

The controller drawings in `assets/` are the VSCView overlays from
[AL2009man's Gamepad Asset Pack](https://github.com/AL2009man/Gamepad-Asset-Pack),
MIT licensed (see `LICENSE-GAMEPAD-ASSET-PACK`), unchanged. Controller names and
logos belong to their respective trademark owners.
