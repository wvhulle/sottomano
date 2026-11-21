use std::ops::Range;

use corredo::{
    drag_reorder::{SlotGroup, SlotTileInset, UngroupedSlotLook},
    theming::{LINE_HEIGHT, background_color, background_weak_color, primary_color},
};
use iced::{
    Alignment, Element,
    widget::{Row, text},
};
use sottomano_desktop_sources::desktop_windows_apps::{
    window_title_labels::WindowGroupKey, workspace_window_snapshot,
};
use sottomano_toml_config::Config;

use super::{
    window_list::WindowListInteraction,
    window_row_presentation::{ArrangedWindowRows, WindowSlotPresentation},
};
use crate::{
    desktop_panels::panel_slot_drag::PanelSlotContext,
    group_indicators::indicator_state::{IndicatorState, IndicatorStateTint},
    icon_catalog::{
        icon_handle::IconHandle,
        window_icons::{WindowAssets, window_icon},
    },
    widgets::{shared_metrics::ACCENT_STROKE_PX, text::resolved_font_faces},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GroupIconPlacement {
    OnHeading,
    OnEachWindow,
}

impl From<WindowGroupKey<'_>> for GroupIconPlacement {
    fn from(group_key: WindowGroupKey<'_>) -> Self {
        match group_key {
            WindowGroupKey::WorkingDirectory(_) | WindowGroupKey::RemoteHost(_) => {
                GroupIconPlacement::OnEachWindow
            }
            WindowGroupKey::WebSite(_)
            | WindowGroupKey::EditorProject(_)
            | WindowGroupKey::MailAccount(_)
            | WindowGroupKey::Application(_) => GroupIconPlacement::OnHeading,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum HeadingIcon<'window> {
    Favicon(&'window str),
    AppIcon(&'window IconHandle),
    Bare,
}

enum WindowGroupIdentity<'window> {
    Ungrouped,
    Keyed {
        group_key: WindowGroupKey<'window>,
        heading_icon: HeadingIcon<'window>,
    },
}

fn window_group_identity<'window>(
    window_entry: &'window workspace_window_snapshot::WinEntry,
    icons: WindowAssets<'window>,
) -> WindowGroupIdentity<'window> {
    let Some(group_key) = window_entry.group_key() else {
        return WindowGroupIdentity::Ungrouped;
    };
    let heading_icon = match window_entry.caption.favicon_key() {
        Some(favicon_key) => HeadingIcon::Favicon(favicon_key),
        None => match icons.icon_for(window_entry) {
            Some(drawable_icon) => HeadingIcon::AppIcon(&drawable_icon.handle),
            None => HeadingIcon::Bare,
        },
    };
    WindowGroupIdentity::Keyed {
        group_key,
        heading_icon,
    }
}

fn shares_keyed_group(
    previous: &WindowGroupIdentity<'_>,
    current: &WindowGroupIdentity<'_>,
) -> bool {
    let WindowGroupIdentity::Keyed {
        group_key: previous_group_key,
        heading_icon: previous_heading_icon,
    } = previous
    else {
        return false;
    };
    let WindowGroupIdentity::Keyed {
        group_key: current_group_key,
        heading_icon: current_heading_icon,
    } = current
    else {
        return false;
    };
    if previous_group_key != current_group_key {
        return false;
    }
    match GroupIconPlacement::from(*current_group_key) {
        GroupIconPlacement::OnEachWindow => true,
        GroupIconPlacement::OnHeading => previous_heading_icon == current_heading_icon,
    }
}

fn keyed_group_positions(identities: &[WindowGroupIdentity<'_>]) -> Vec<Range<usize>> {
    identities
        .chunk_by(shares_keyed_group)
        .scan(0, |next_start, run| {
            let start = *next_start;
            *next_start += run.len();
            Some(start..*next_start)
        })
        .filter(|positions| match identities[positions.start] {
            WindowGroupIdentity::Ungrouped => false,
            WindowGroupIdentity::Keyed {
                group_key: WindowGroupKey::Application(_),
                ..
            } => positions.len() >= 2,
            WindowGroupIdentity::Keyed { .. } => true,
        })
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
    window_entry: &workspace_window_snapshot::WinEntry,
    icons: WindowAssets<'_>,
) -> Option<String> {
    match window_entry.group_key()? {
        WindowGroupKey::Application(app_id) => Some(
            icons
                .desktop_entries
                .launchable_app(app_id)
                .map_or_else(|| app_id.to_owned(), |app| app.name),
        ),
        group_key => Some(group_key.heading_text().to_owned()),
    }
}

fn keyed_group_heading<'view>(
    window_entry: &workspace_window_snapshot::WinEntry,
    icons: WindowAssets<'view>,
    config: &Config,
) -> Option<Element<'view, WindowListInteraction>> {
    let heading_text = keyed_group_heading_text(window_entry, icons)?;
    let tint = IndicatorStateTint {
        state: IndicatorState::Idle,
        opacity: 1.0,
    };
    let icon = window_entry
        .group_key()
        .filter(|group_key| GroupIconPlacement::from(*group_key) == GroupIconPlacement::OnHeading)
        .and_then(|_| {
            window_icon(
                icons.favicons,
                window_entry.caption.favicon_key(),
                icons.icon_for(window_entry),
                config.theme.font.icon_size_px,
                1.0,
            )
        });
    Some(
        Row::new()
            .extend(icon)
            .push(
                text(heading_text)
                    .size(config.theme.font.text_size_px)
                    .line_height(LINE_HEIGHT)
                    .font(resolved_font_faces(&config.theme.font).regular)
                    .style(tint.text_style()),
            )
            .spacing(config.theme.spacing.item_gap)
            .padding([0.0, config.theme.spacing.item_gap])
            .align_y(Alignment::Center)
            .into(),
    )
}

pub(super) fn arranged_keyed_groups<'view>(
    order: &[&workspace_window_snapshot::WinEntry],
    icons: WindowAssets<'view>,
    slot_context: PanelSlotContext<'_>,
) -> ArrangedWindowRows<'view> {
    let group_identities: Vec<_> = order
        .iter()
        .map(|window_entry| window_group_identity(window_entry, icons))
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
        ungrouped_slot_look: UngroupedSlotLook::RestingTile {
            tile_inset: SlotTileInset {
                thickness: tile_inset_thickness,
                enclosure_radius: slot_context.panel_corner_radius_px,
            },
            resting_color: Box::new(background_color),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::{ops::Range, path::Path};

    use sottomano_desktop_sources::desktop_windows_apps::window_title_labels::WindowGroupKey;

    use super::{
        HeadingIcon, WindowGroupIdentity, WindowSlotPresentation, keyed_group_positions,
        slot_presentations,
    };
    use crate::icon_catalog::icon_handle::IconHandle;

    fn named_icon(icon_name: &str) -> IconHandle {
        IconHandle::at(Path::new(&format!("{icon_name}.svg")))
    }

    fn identity<'window>(
        group_key: WindowGroupKey<'window>,
        icon: &'window IconHandle,
    ) -> WindowGroupIdentity<'window> {
        WindowGroupIdentity::Keyed {
            group_key,
            heading_icon: HeadingIcon::AppIcon(icon),
        }
    }

    #[test]
    fn neighbouring_terminals_in_one_directory_form_one_group() {
        let sottomano = WindowGroupKey::WorkingDirectory("sottomano");
        let (claude_icon, terminal_icon) = (named_icon("claude"), named_icon("terminal"));
        let identities = [
            identity(sottomano, &claude_icon),
            identity(sottomano, &claude_icon),
            identity(sottomano, &terminal_icon),
            identity(sottomano, &terminal_icon),
            WindowGroupIdentity::Ungrouped,
            identity(WindowGroupKey::WorkingDirectory("nix"), &terminal_icon),
            WindowGroupIdentity::Ungrouped,
        ];
        assert_eq!(keyed_group_positions(&identities), [0..4, 5..6]);
    }

    #[test]
    fn neighbouring_sites_with_different_favicons_stay_apart() {
        let github = WindowGroupKey::WebSite("github.com");
        let identities = [
            WindowGroupIdentity::Keyed {
                group_key: github,
                heading_icon: HeadingIcon::Favicon("first"),
            },
            WindowGroupIdentity::Keyed {
                group_key: github,
                heading_icon: HeadingIcon::Favicon("second"),
            },
        ];
        assert_eq!(keyed_group_positions(&identities), [0..1, 1..2]);
    }

    #[test]
    fn a_directory_and_a_site_with_one_name_stay_apart() {
        let shared_icon = named_icon("icon");
        let identities = [
            identity(WindowGroupKey::WorkingDirectory("github.com"), &shared_icon),
            identity(WindowGroupKey::WebSite("github.com"), &shared_icon),
        ];
        assert_eq!(keyed_group_positions(&identities), [0..1, 1..2]);
    }

    #[test]
    fn windows_grouped_only_by_their_app_need_a_neighbour() {
        let dolphin = WindowGroupKey::Application("org.kde.dolphin");
        let (dolphin_icon, spotify_icon) = (named_icon("dolphin"), named_icon("spotify"));
        let identities = [
            identity(dolphin, &dolphin_icon),
            identity(dolphin, &dolphin_icon),
            identity(WindowGroupKey::Application("spotify"), &spotify_icon),
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
