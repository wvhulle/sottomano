use std::{ops::Range, path::PathBuf};

use iced::{
    Alignment, Element,
    widget::{Row, text},
};
use sottomano_desktop_sources::desktop_windows_apps::{
    niri_window_snapshot, window_title_labels::WindowLabel,
};
use sottomano_toml_config::Config;

use super::{
    window_list::WindowListMessage,
    window_row_presentation::{ArrangedWindowRows, WindowSlotPresentation},
};
use crate::{
    desktop_panels::panel_slot_drag::PanelSlotContext,
    group_items::reading_display_state::{ReadingDisplayState, ReadingStateTint},
    widgets::{
        configured_theme::{background_color, background_weak_color, primary_color},
        pointer_drag::{
            slot_group::{SlotGroup, SlotTileInset},
            slot_pager::UngroupedHighlightLook,
        },
        text::{LINE_HEIGHT, font_faces_of},
        window_icon::{WindowAssets, window_icon},
    },
};

struct WindowGroupIdentity<'a> {
    label: Option<&'a WindowLabel>,
    icon_path: Option<PathBuf>,
    favicon_key: Option<&'a str>,
}

impl WindowGroupIdentity<'_> {
    fn continues_group_of(&self, previous: &WindowGroupIdentity<'_>) -> bool {
        self.icon_path == previous.icon_path
            && self.favicon_key == previous.favicon_key
            && self
                .label
                .zip(previous.label)
                .is_some_and(|(label, previous_label)| label.continues_location_of(previous_label))
    }

    fn opens_group(&self) -> bool {
        self.label
            .is_some_and(|label| label.location_text().is_some())
    }
}

fn location_group_positions(identities: &[WindowGroupIdentity<'_>]) -> Vec<Range<usize>> {
    identities
        .chunk_by(|previous, current| current.continues_group_of(previous))
        .scan(0, |next_start, run| {
            let start = *next_start;
            *next_start += run.len();
            Some(start..*next_start)
        })
        .filter(|positions| identities[positions.start].opens_group())
        .collect()
}

fn slot_presentations(
    group_positions: &[Range<usize>],
    window_count: usize,
) -> Vec<WindowSlotPresentation> {
    (0..window_count)
        .map(|position| {
            if group_positions
                .iter()
                .any(|positions| positions.contains(&position))
            {
                WindowSlotPresentation::InLocationGroup
            } else {
                WindowSlotPresentation::OutsideLocationGroups
            }
        })
        .collect()
}

fn location_group_heading<'a>(
    window_entry: &niri_window_snapshot::WinEntry,
    icons: WindowAssets<'a>,
    config: &Config,
) -> Option<Element<'a, WindowListMessage>> {
    let location = window_entry.label.as_ref()?.location_text()?;
    let tint = ReadingStateTint {
        state: ReadingDisplayState::Idle,
        opacity: 1.0,
    };
    let icon_path = icons.path_for(window_entry);
    let icon = window_icon(
        icons.favicons,
        window_entry.favicon_key(),
        icon_path.as_deref(),
        config.theme.font.icon_size_px,
        1.0,
    );
    Some(
        Row::new()
            .extend(icon)
            .push(
                text(location.to_owned())
                    .size(config.theme.font.text_size_px)
                    .line_height(LINE_HEIGHT)
                    .font(font_faces_of(&config.theme.font).regular)
                    .style(tint.text_style()),
            )
            .spacing(config.theme.spacing.item_gap)
            .padding([0.0, config.theme.spacing.item_gap])
            .align_y(Alignment::Center)
            .into(),
    )
}

pub(super) fn arranged_location_groups<'a>(
    order: &[&niri_window_snapshot::WinEntry],
    icons: WindowAssets<'a>,
    slot_context: PanelSlotContext<'_>,
) -> ArrangedWindowRows<'a> {
    let group_identities: Vec<_> = order
        .iter()
        .map(|window_entry| WindowGroupIdentity {
            label: window_entry.label.as_ref(),
            icon_path: icons.path_for(window_entry),
            favicon_key: window_entry.favicon_key(),
        })
        .collect();
    let group_positions = location_group_positions(&group_identities);
    let groups = group_positions
        .iter()
        .filter_map(|positions| {
            Some(SlotGroup {
                positions: positions.clone(),
                heading: location_group_heading(
                    order.get(positions.start)?,
                    icons,
                    slot_context.config,
                )?,
                enclosure_color: Box::new(background_weak_color),
                inner_color: Box::new(background_color),
                highlight_color: Box::new(primary_color),
                enclosure_thickness: slot_context.config.theme.spacing.item_gap,
                enclosure_radius: slot_context.panel_corner_radius_px,
            })
        })
        .collect();
    ArrangedWindowRows {
        groups,
        presentations: slot_presentations(&group_positions, order.len()),
        ungrouped_highlight_look: UngroupedHighlightLook::AccentTile(SlotTileInset {
            thickness: slot_context.config.theme.spacing.item_gap,
            enclosure_radius: slot_context.panel_corner_radius_px,
        }),
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::desktop_windows_apps::{
        kitty_terminal_windows::kitty_window_label::{
            ForegroundActivity, KittyWindowLabel, WorkingLocation,
        },
        window_title_labels::WindowLabel,
    };

    use super::{
        WindowGroupIdentity, WindowSlotPresentation, location_group_positions, slot_presentations,
    };

    fn kitty_label_in(directory: &str, foreground_activity: ForegroundActivity) -> WindowLabel {
        WindowLabel::KittyAgent(KittyWindowLabel {
            working_location: WorkingLocation::LocalDirectory(directory.to_owned()),
            foreground_activity,
        })
    }

    fn running(command_line: &str) -> ForegroundActivity {
        ForegroundActivity::ShellCommand {
            command_line: command_line.to_owned(),
            program_name: None,
        }
    }

    fn identity_with_icon<'a>(label: &'a WindowLabel, icon: &str) -> WindowGroupIdentity<'a> {
        WindowGroupIdentity {
            label: Some(label),
            icon_path: Some(icon.into()),
            favicon_key: None,
        }
    }

    #[test]
    fn neighbouring_windows_with_one_location_and_icon_form_one_group() {
        let first_session = kitty_label_in("sottomano", running("first session"));
        let second_session = kitty_label_in("sottomano", running("second session"));
        let cargo = kitty_label_in("sottomano", running("cargo"));
        let idle_shell = kitty_label_in(
            "sottomano",
            ForegroundActivity::ShellPrompt {
                shell_name: "nu".to_owned(),
            },
        );
        let plain_title = WindowLabel::Titled("Files".to_owned());
        let other_directory = kitty_label_in("nix", running("jj"));
        let identities = [
            identity_with_icon(&first_session, "claude"),
            identity_with_icon(&second_session, "claude"),
            identity_with_icon(&cargo, "terminal"),
            identity_with_icon(&idle_shell, "terminal"),
            identity_with_icon(&plain_title, "files"),
            identity_with_icon(&other_directory, "terminal"),
            WindowGroupIdentity {
                label: None,
                icon_path: None,
                favicon_key: None,
            },
        ];
        assert_eq!(location_group_positions(&identities), [0..2, 2..4, 5..6]);
    }

    #[test]
    fn only_windows_inside_a_group_are_presented_as_grouped() {
        use WindowSlotPresentation::{InLocationGroup, OutsideLocationGroups};
        assert_eq!(
            slot_presentations(&[0..2, 3..4], 5),
            [
                InLocationGroup,
                InLocationGroup,
                OutsideLocationGroups,
                InLocationGroup,
                OutsideLocationGroups
            ]
        );
    }
}
