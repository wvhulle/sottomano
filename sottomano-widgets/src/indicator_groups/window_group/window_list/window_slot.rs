use corredo::{
    drag_reorder::{HoverReorder, draggable_slot},
    gauges::activity_line,
    orientation::Axis,
    popovers::NestedMenuOpenTrigger,
    theming::{background_color, background_strong_color, danger_color, warning_color},
};
use iced::{
    Alignment, Element,
    widget::{Row, container, progress_bar, right, stack, text, text::Wrapping},
};
use sottomano_compositor_model::compositor_window_facts::{Urgency, WindowFocus, WindowTiling};
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        unity_launcher_badges::{LauncherCount, LauncherProgress},
        window_title_labels::WindowCaption,
        workspace_window_snapshot,
    },
    media_playback_audio::pipewire_audio::WindowAudio,
};

use super::{
    WindowListInteraction, capped_window_title, dragged,
    slot_extras::{Extras, TilePress, WindowSound, audible_pid, entry_tile_press},
};
use crate::{
    desktop_panels::{
        panel_slot::PanelSlot,
        panel_slot_drag::{PanelSlotContext, SlotClick, SlotPress},
    },
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        program_brand_icons::program_brand_icon,
        simple_icons::view_simple_icon,
    },
    group_indicators::indicator_state::{IndicatorState, IndicatorStateTint},
    icon_catalog::window_icons::{WindowAssets, window_icon},
    indicator_groups::window_group::{
        WindowTarget,
        keyed_window_group_rows::GroupIconPlacement,
        window_row_presentation::{SlotBackdrop, WindowSlotPresentation, WindowTileMarking},
    },
    widgets::{
        pointer_drag::DraggedPanelSlot,
        shared_metrics::{ACCENT_STROKE_PX, pill_corners},
        text::resolved_font_faces,
        window_title::WindowTitle,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UrgencyMarking {
    Tile,
    Line,
    Unmarked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowAttention {
    Urgent,
    Calm,
}

pub(super) struct WindowSlot<'snapshot> {
    pub(super) window_entry: &'snapshot workspace_window_snapshot::WinEntry,
    pub(super) item: DraggedPanelSlot,
    pub(super) extras: Extras,
    pub(super) position_in_panel: usize,
    pub(super) opacity: f32,
    pub(super) presentation: WindowSlotPresentation,
    pub(super) slot_context: PanelSlotContext<'snapshot>,
}

fn glyph_color(
    tint: IndicatorStateTint,
    slot_backdrop: SlotBackdrop,
) -> impl Fn(&iced::Theme) -> iced::Color {
    move |theme| match slot_backdrop {
        SlotBackdrop::OpaqueTile => background_color(theme).scale_alpha(tint.opacity),
        SlotBackdrop::Bare => tint.color(theme),
    }
}

impl WindowSlot<'_> {
    fn attention(&self) -> WindowAttention {
        if self.window_entry.urgency == Urgency::Urgent
            || self.extras.launcher_urgency == Urgency::Urgent
        {
            WindowAttention::Urgent
        } else {
            WindowAttention::Calm
        }
    }

    pub(super) fn urgency_marking(&self) -> UrgencyMarking {
        match (self.attention(), self.presentation) {
            (WindowAttention::Calm, _) => UrgencyMarking::Unmarked,
            (WindowAttention::Urgent, WindowSlotPresentation::Flat) => UrgencyMarking::Line,
            (
                WindowAttention::Urgent,
                WindowSlotPresentation::InKeyedGroup | WindowSlotPresentation::OutsideKeyedGroups,
            ) => UrgencyMarking::Tile,
        }
    }

    fn tile_marking(&self) -> WindowTileMarking {
        match (self.window_entry.focus, self.attention()) {
            (WindowFocus::Focused, _) | (WindowFocus::Unfocused, WindowAttention::Urgent) => {
                WindowTileMarking::Marked
            }
            (WindowFocus::Unfocused, WindowAttention::Calm) => WindowTileMarking::Unmarked,
        }
    }

    fn tile_press(&self) -> TilePress {
        entry_tile_press(self.window_entry)
    }

    fn audio_pid(&self) -> Option<i32> {
        audible_pid(self.window_entry, &self.extras)
    }

    pub(super) fn view<'view>(
        &self,
        icons: WindowAssets<'view>,
    ) -> Element<'view, WindowListInteraction> {
        let press = SlotPress {
            item: self.item,
            position: self.position_in_panel,
            click: if self.tile_press() == TilePress::TogglesMaximize {
                SlotClick::MaximizeColumn
            } else {
                SlotClick::FocusWindow(self.window_entry.id)
            },
        };
        let hover_reorder = if self.window_entry.layout.column().is_some()
            && self.window_entry.layout.tiling == WindowTiling::Tiled
        {
            HoverReorder::ReordersOnHover
        } else {
            HoverReorder::StaysOnHover
        };
        let pressable = draggable_slot(
            self.panel_slot(icons),
            press,
            hover_reorder,
            &self.slot_context.drag,
            dragged,
        );
        match self.audio_pid() {
            Some(pid) => pressable
                .on_middle_press(WindowListInteraction::ToggleAudio(pid))
                .into(),
            None => pressable.into(),
        }
    }

    fn panel_slot<'view>(
        &self,
        icons: WindowAssets<'view>,
    ) -> Element<'view, WindowListInteraction> {
        let config = self.slot_context.config;
        let edge = self.slot_context.edge;
        let window = self.window_entry.id;
        let window_slot = PanelSlot::new(self.button_content(icons), self.indicator_line())
            .padded_along_panel(config.theme.spacing.item_gap);
        let window_slot = match self.progress_ticks() {
            Some(ticks) => window_slot.ticks(ticks),
            None => window_slot,
        }
        .view(edge);
        let glyph_slot = |icon: fn(Weight) -> Phosphor, target: WindowTarget| {
            PanelSlot::new(self.secondary_glyph(icon), self.indicator_line()).hovered(
                edge,
                NestedMenuOpenTrigger::Hover,
                target.into(),
                WindowListInteraction::HoverCard,
            )
        };
        let focused_glyph_slots = (self.window_entry.focus == WindowFocus::Focused).then(|| {
            [
                glyph_slot(Phosphor::UserFocus, WindowTarget::Usage(window)),
                glyph_slot(Phosphor::Package, WindowTarget::Inspect(window)),
            ]
        });
        Row::new()
            .push(window_slot)
            .extend(focused_glyph_slots.into_iter().flatten())
            .spacing(config.theme.spacing.item_gap)
            .into()
    }

    fn secondary_glyph<'view>(
        &self,
        icon: fn(Weight) -> Phosphor,
    ) -> Element<'view, WindowListInteraction> {
        let tint = IndicatorStateTint {
            state: IndicatorState::Off,
            opacity: self.opacity,
        };
        IconGlyph::from(icon(tint.state.weight())).view(
            self.slot_context.config.theme.font.icon_size_px,
            glyph_color(tint, self.slot_backdrop()),
        )
    }

    fn slot_backdrop(&self) -> SlotBackdrop {
        self.presentation.slot_backdrop(self.tile_marking())
    }

    pub(super) fn button_content<'view>(
        &self,
        icons: WindowAssets<'view>,
    ) -> Element<'view, WindowListInteraction> {
        let config = self.slot_context.config;
        let title_row = Row::new().extend(self.badged_icon(icons));
        let title_row = match &self.window_entry.caption {
            WindowCaption::Labelled(label) => title_row.push(capped_window_title(
                self.presentation.styled_title(
                    WindowTitle::new(label, config)
                        .opacity(self.opacity)
                        .wrapping(Wrapping::None),
                    self.tile_marking(),
                ),
                config,
                self.slot_context.window_title_length,
            )),
            WindowCaption::Unlabelled => title_row,
        };
        title_row
            .extend(self.audio_glyph())
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }

    fn audio_glyph<'view>(&self) -> Option<Element<'view, WindowListInteraction>> {
        let (state, icon) = match self.extras.audio {
            WindowSound::Untracked | WindowSound::Tracked(WindowAudio::Silent) => return None,
            WindowSound::Tracked(WindowAudio::Audible) => {
                (IndicatorState::On, Phosphor::SpeakerHigh(Weight::Regular))
            }
            WindowSound::Tracked(WindowAudio::Muted) => {
                (IndicatorState::Off, Phosphor::SpeakerX(Weight::Regular))
            }
        };
        let tint = IndicatorStateTint {
            state,
            opacity: self.opacity,
        };
        Some(IconGlyph::from(icon.with_weight(state.weight())).view(
            self.slot_context.config.theme.font.icon_size_px,
            glyph_color(tint, self.slot_backdrop()),
        ))
    }

    fn indicator_line<'view>(&self) -> Element<'view, WindowListInteraction> {
        let severity = match self.urgency_marking() {
            UrgencyMarking::Line => 1.0,
            UrgencyMarking::Tile | UrgencyMarking::Unmarked => 0.0,
        };
        activity_line(
            Axis::Horizontal,
            danger_color,
            severity * self.opacity,
            ACCENT_STROKE_PX,
        )
    }

    fn progress_ticks<'view>(&self) -> Option<Element<'view, WindowListInteraction>> {
        let LauncherProgress::Shown(fraction) = self.extras.progress else {
            return None;
        };
        let thickness = ACCENT_STROKE_PX;
        let opacity = self.opacity;
        Some(
            progress_bar(0.0..=1.0, fraction)
                .girth(thickness)
                .style(move |theme| progress_bar::Style {
                    background: background_strong_color(theme).scale_alpha(opacity).into(),
                    bar: warning_color(theme).scale_alpha(opacity).into(),
                    border: pill_corners(thickness),
                })
                .into(),
        )
    }

    fn count_badge<'view>(&self) -> Option<Element<'view, WindowListInteraction>> {
        let config = self.slot_context.config;
        let LauncherCount::Shown(count) = self.extras.count else {
            return None;
        };
        if count <= 0 {
            return None;
        }
        let label = if count > 99 {
            "99+".to_owned()
        } else {
            count.to_string()
        };
        let size = config.theme.font.text_size_px;
        let opacity = self.opacity;
        let badge = container(
            text(label)
                .size(size)
                .font(resolved_font_faces(&config.theme.font).regular)
                .style(move |theme| text::Style {
                    color: Some(background_color(theme).scale_alpha(opacity)),
                }),
        )
        .padding([0.0, config.theme.spacing.item_gap])
        .style(move |theme| {
            container::Style::default()
                .background(danger_color(theme).scale_alpha(opacity))
                .border(pill_corners(size))
        });
        Some(right(badge).into())
    }

    fn badged_icon<'view>(
        &self,
        icons: WindowAssets<'view>,
    ) -> Option<Element<'view, WindowListInteraction>> {
        let badge = self.count_badge();
        if self.group_icon_placement() == Some(GroupIconPlacement::OnHeading) && badge.is_none() {
            return None;
        }
        let icon = self.slot_icon(icons)?;
        Some(match badge {
            Some(badge) => stack([icon, badge]).into(),
            None => icon,
        })
    }

    fn group_icon_placement(&self) -> Option<GroupIconPlacement> {
        match self.presentation {
            WindowSlotPresentation::InKeyedGroup => {
                self.window_entry.group_key().map(GroupIconPlacement::from)
            }
            WindowSlotPresentation::Flat | WindowSlotPresentation::OutsideKeyedGroups => None,
        }
    }

    pub(super) fn slot_icon<'view>(
        &self,
        icons: WindowAssets<'view>,
    ) -> Option<Element<'view, WindowListInteraction>> {
        let Some(brand_icon) = self
            .window_entry
            .caption
            .program_name()
            .and_then(program_brand_icon)
        else {
            return self.resolved_icon(icons);
        };
        let tint = IndicatorStateTint {
            state: IndicatorState::Idle,
            opacity: self.opacity,
        };
        Some(view_simple_icon(
            brand_icon,
            self.slot_context.config.theme.font.icon_size_px,
            glyph_color(tint, self.slot_backdrop()),
        ))
    }

    fn resolved_icon<'view>(
        &self,
        icons: WindowAssets<'view>,
    ) -> Option<Element<'view, WindowListInteraction>> {
        window_icon(
            icons.favicons,
            self.window_entry.caption.favicon_key(),
            icons.icon_for(self.window_entry),
            self.slot_context.config.theme.font.icon_size_px,
            self.opacity,
        )
    }
}
