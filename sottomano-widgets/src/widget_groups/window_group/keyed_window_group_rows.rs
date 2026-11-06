use std::{ops::Range, path::PathBuf};

use iced::{
    Alignment, Element,
    widget::{Row, text},
};
use sottomano_desktop_sources::desktop_windows_apps::{
    niri_window_snapshot, window_title_labels::WindowGroupKey,
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
        shared_metrics::ACCENT_STROKE_PX,
        text::{LINE_HEIGHT, font_faces_of},
        window_icon::{WindowAssets, window_icon},
    },
};

struct WindowGroupIdentity<'a> {
    group_key: Option<WindowGroupKey<'a>>,
    icon_path: Option<PathBuf>,
    favicon_key: Option<&'a str>,
}

impl WindowGroupIdentity<'_> {
    fn continues_group_of(&self, previous: &WindowGroupIdentity<'_>) -> bool {
        self.group_key.is_some()
            && self.group_key == previous.group_key
            && self.icon_path == previous.icon_path
            && self.favicon_key == previous.favicon_key
    }

    fn opens_group_of(&self, window_count: usize) -> bool {
        match self.group_key {
            None => false,
            Some(WindowGroupKey::Application(_)) => window_count >= 2,
            Some(_) => true,
        }
    }
}

fn keyed_group_positions(identities: &[WindowGroupIdentity<'_>]) -> Vec<Range<usize>> {
    identities
        .chunk_by(|previous, current| current.continues_group_of(previous))
        .scan(0, |next_start, run| {
            let start = *next_start;
            *next_start += run.len();
            Some(start..*next_start)
        })
        .filter(|positions| identities[positions.start].opens_group_of(positions.len()))
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
                WindowSlotPresentation::InKeyedGroup
            } else {
                WindowSlotPresentation::OutsideKeyedGroups
            }
        })
        .collect()
}

fn keyed_group_heading_text(
    window_entry: &niri_window_snapshot::WinEntry,
    icons: WindowAssets<'_>,
) -> Option<String> {
    match window_entry.group_key()? {
        WindowGroupKey::Application(app_id) => Some(
            icons
                .entries
                .get()
                .and_then(|entries| entries.launchable_app(app_id))
                .map_or_else(|| app_id.to_owned(), |app| app.name),
        ),
        group_key => Some(group_key.heading_text().to_owned()),
    }
}

fn keyed_group_heading<'a>(
    window_entry: &niri_window_snapshot::WinEntry,
    icons: WindowAssets<'a>,
    config: &Config,
) -> Option<Element<'a, WindowListMessage>> {
    let heading_text = keyed_group_heading_text(window_entry, icons)?;
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
                text(heading_text)
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

pub(super) fn arranged_keyed_groups<'a>(
    order: &[&niri_window_snapshot::WinEntry],
    icons: WindowAssets<'a>,
    slot_context: PanelSlotContext<'_>,
) -> ArrangedWindowRows<'a> {
    let group_identities: Vec<_> = order
        .iter()
        .map(|window_entry| WindowGroupIdentity {
            group_key: window_entry.group_key(),
            icon_path: icons.path_for(window_entry),
            favicon_key: window_entry.favicon_key(),
        })
        .collect();
    let group_positions = keyed_group_positions(&group_identities);
    let tile_inset_thickness = ACCENT_STROKE_PX;
    let groups = group_positions
        .iter()
        .filter_map(|positions| {
            Some(SlotGroup {
                positions: positions.clone(),
                heading: keyed_group_heading(
                    order.get(positions.start)?,
                    icons,
                    slot_context.config,
                )?,
                enclosure_color: Box::new(background_weak_color),
                inner_color: Box::new(background_color),
                highlight_color: Box::new(primary_color),
                enclosure_thickness: tile_inset_thickness,
                enclosure_radius: slot_context.panel_corner_radius_px,
            })
        })
        .collect();
    ArrangedWindowRows {
        groups,
        presentations: slot_presentations(&group_positions, order.len()),
        ungrouped_highlight_look: UngroupedHighlightLook::AccentTile(SlotTileInset {
            thickness: tile_inset_thickness,
            enclosure_radius: slot_context.panel_corner_radius_px,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use sottomano_desktop_sources::desktop_windows_apps::window_title_labels::WindowGroupKey;

    use super::{
        WindowGroupIdentity, WindowSlotPresentation, keyed_group_positions, slot_presentations,
    };

    fn identity<'a>(group_key: Option<WindowGroupKey<'a>>, icon: &str) -> WindowGroupIdentity<'a> {
        WindowGroupIdentity {
            group_key,
            icon_path: Some(icon.into()),
            favicon_key: None,
        }
    }

    #[test]
    fn neighbouring_windows_with_one_group_key_and_icon_form_one_group() {
        let sottomano = Some(WindowGroupKey::WorkingDirectory("sottomano"));
        let identities = [
            identity(sottomano, "claude"),
            identity(sottomano, "claude"),
            identity(sottomano, "terminal"),
            identity(sottomano, "terminal"),
            identity(None, "files"),
            identity(Some(WindowGroupKey::WorkingDirectory("nix")), "terminal"),
            identity(None, "unknown"),
        ];
        assert_eq!(keyed_group_positions(&identities), [0..2, 2..4, 5..6]);
    }

    #[test]
    fn a_directory_and_a_site_with_one_name_stay_apart() {
        let identities = [
            identity(Some(WindowGroupKey::WorkingDirectory("github.com")), "icon"),
            identity(Some(WindowGroupKey::WebSite("github.com")), "icon"),
        ];
        assert_eq!(keyed_group_positions(&identities), [0..1, 1..2]);
    }

    #[test]
    fn windows_grouped_only_by_their_app_need_a_neighbour() {
        let dolphin = Some(WindowGroupKey::Application("org.kde.dolphin"));
        let identities = [
            identity(dolphin, "dolphin"),
            identity(dolphin, "dolphin"),
            identity(Some(WindowGroupKey::Application("spotify")), "spotify"),
        ];
        assert_eq!(
            keyed_group_positions(&identities),
            [Range { start: 0, end: 2 }]
        );
    }

    #[test]
    fn only_windows_inside_a_group_are_presented_as_grouped() {
        use WindowSlotPresentation::{InKeyedGroup, OutsideKeyedGroups};
        assert_eq!(
            slot_presentations(&[0..2, 3..4], 5),
            [
                InKeyedGroup,
                InKeyedGroup,
                OutsideKeyedGroups,
                InKeyedGroup,
                OutsideKeyedGroups
            ]
        );
    }
}
