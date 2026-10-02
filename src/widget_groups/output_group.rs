use std::collections::HashMap;

use iced::{Element, widget::button};

use crate::{
    config::Config,
    data::source::niri::{OutputInfo, OutputMode},
    desktop_shell::{
        monitor_surfaces::Position,
        hover_tooltips::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        bar_widget::Glyph,
        flat_button,
        pager_slot::permanently_inactive_slot,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Row, Tooltip, row},
    },
};

#[derive(Debug, Clone)]
pub enum OutputMessage {
    Clicked,
    Tooltip(TooltipMessage),
}

pub fn view<'a>(output: &str, config: &Config, position: Position) -> Element<'a, OutputMessage> {
    let glyph = Glyph::from(Phosphor::Stack(Weight::Regular))
        .view(config.font.icon_size_px, config.palette.bar_foreground);
    let pressable = button(permanently_inactive_slot(glyph, config, position))
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(OutputMessage::Clicked);
    hoverable(
        pressable.into(),
        HoverTarget::Output(output.to_owned()),
        OutputMessage::Tooltip,
    )
}

pub fn tooltip(outputs: &HashMap<String, OutputInfo>, output: &str) -> Tooltip {
    let mut rows = vec![row("Output", output.to_owned())];
    if let Some(info) = outputs.get(output) {
        rows.extend(product_rows(info));
        rows.extend(geometry_rows(info));
    }
    Tooltip::table(rows)
}

fn product_rows(info: &OutputInfo) -> Vec<Row> {
    [("Make", &info.make), ("Model", &info.model)]
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| row(key, value.clone()))
        .collect()
}

fn geometry_rows(info: &OutputInfo) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(OutputMode {
        width,
        height,
        refresh_hz,
    }) = info.mode
    {
        rows.push(row("Resolution", format!("{width}x{height}")));
        rows.push(row("Refresh", format!("{refresh_hz:.2} Hz")));
    }
    rows.push(row("Scale", format!("{:.2}", info.scale)));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tooltip_body::{Cell, Row, SectionBody, TooltipBody, row};

    fn laptop_panel() -> HashMap<String, OutputInfo> {
        HashMap::from([(
            "eDP-1".to_owned(),
            OutputInfo {
                make: "LG Display".to_owned(),
                model: String::new(),
                mode: Some(OutputMode {
                    width: 1920,
                    height: 1200,
                    refresh_hz: 60.0,
                }),
                scale: 1.2,
                logical_height: 1000,
            },
        )])
    }

    fn labelled(tooltip: &Tooltip) -> Vec<Row> {
        match &tooltip.body {
            TooltipBody::Sections(sections) => sections
                .iter()
                .filter_map(|section| match &section.body {
                    SectionBody::Rows(rows) | SectionBody::Columns { rows, .. } => Some(rows),
                    SectionBody::Tree { .. } => None,
                })
                .flatten()
                .cloned()
                .collect(),
            _ => panic!("expected sections"),
        }
    }

    #[test]
    fn placeholder_model_is_not_shown() {
        let rows = labelled(&tooltip(&laptop_panel(), "eDP-1"));
        assert!(rows.iter().all(|row| row.key != Cell::from("Model")));
        assert_eq!(
            rows,
            [
                row("Output", "eDP-1"),
                row("Make", "LG Display"),
                row("Resolution", "1920x1200"),
                row("Refresh", "60.00 Hz"),
                row("Scale", "1.20"),
            ]
        );
    }

    #[test]
    fn unknown_output_still_names_the_connector() {
        let rows = labelled(&tooltip(&HashMap::new(), "DP-2"));
        assert_eq!(rows, [row("Output", "DP-2")]);
    }
}
