use corredo::{
    gauges::activity_line,
    orientation::Axis,
    popovers::NestedMenuOpenTrigger,
    press_actions::flat_button,
    theming::{primary_color, secondary_text_color, text_color},
    transitions::{ExpansionState, expand_transition, reveal},
};
use iced::{
    Alignment, Element, Length, Padding,
    widget::{Row, Space, button, container, mouse_area},
};
use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

use super::{
    GroupEvent, HandleScrollForwarding, IndicatorGroupContent, IndicatorGroupExpansionMessage,
};
use crate::{
    clock::Clock,
    desktop_panels::panel_slot::PanelSlot,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_state::{IndicatorState, IndicatorStateTint},
    },
    hover_cards::{HoverTarget, MenuHandleIndicator, PanelPinOrigin, PointerPresence},
    indicator_groups::indicator_group_identity::IndicatorGroupIdentity,
    widgets::shared_metrics::ACCENT_STROKE_PX,
};

impl<'view> IndicatorGroupContent<'view> {
    #[must_use]
    pub fn view_events(
        self,
        config: &'view Config,
        panel_position: Position,
        clock: Clock,
    ) -> Option<Element<'view, GroupEvent>> {
        self.indicators.first()?;
        let highest_alert_severity = self.highest_alert_severity();
        let IndicatorGroupContent {
            group_identity,
            group_title: _,
            expansion,
            on_expansion,
            headline_indicator,
            scroll_forwarding,
            indicators,
            panel_pin,
            ..
        } = self;
        let handle_target = match panel_pin {
            PanelPinOrigin::Pinned(panel_pin) => HoverTarget::PinnedOnPanel(panel_pin),
            PanelPinOrigin::Unpinned => {
                HoverTarget::GroupHeadline(Box::new(group_identity.clone()))
            }
        };
        let expansion_event = move |message| GroupEvent::Interaction(on_expansion(message));
        let toggle = expansion_event(IndicatorGroupExpansionMessage::Toggle);
        let on_enter = expansion_event(IndicatorGroupExpansionMessage::CursorEntered);
        let on_exit = expansion_event(IndicatorGroupExpansionMessage::CursorExited);
        let handle_indicator = expansion.handle_indicator();
        let expansion_duration = config.indicator_group_layout.expansion_duration;
        let animated = expand_transition(expansion.state, expansion_duration, move |reveal| {
            let handle: Element<'view, GroupEvent> =
                button(headline_indicator.handle_view(config, reveal))
                    .padding(0)
                    .style(flat_button(text_color))
                    .on_press(toggle.clone())
                    .into();
            let handle = PanelSlot::new(
                handle,
                edge_indicator(highest_alert_severity, handle_indicator, reveal),
            )
            .hovered(
                panel_position,
                NestedMenuOpenTrigger::Hover,
                handle_target.clone(),
                GroupEvent::from,
            );
            let handle = match scroll_forwarding {
                HandleScrollForwarding::Forwarded(_) => {
                    let expansion_event = expansion_event.clone();
                    mouse_area(handle)
                        .on_scroll(move |delta| {
                            expansion_event(IndicatorGroupExpansionMessage::HandleScrolled(delta))
                        })
                        .into()
                }
                HandleScrollForwarding::NotForwarded => handle,
            };
            Row::new()
                .push(handle)
                .extend((reveal > 0.0).then(|| {
                    revealed_content(
                        config,
                        reveal,
                        indicators_row(
                            config,
                            &group_identity,
                            &indicators,
                            reveal,
                            clock,
                            panel_position,
                        ),
                    )
                }))
                .into()
        });
        Some(Element::from(
            mouse_area(animated).on_enter(on_enter).on_exit(on_exit),
        ))
    }
}

pub(crate) fn fixed_view<'view>(
    indicator: &GroupIndicator<'view>,
    hover_target: HoverTarget,
    highest_alert_severity: Severity,
    handle_indicator: MenuHandleIndicator,
    config: &'view Config,
    panel_position: Position,
    clock: Clock,
) -> Element<'view, GroupEvent> {
    let edge_indicator_view = edge_indicator(
        highest_alert_severity,
        handle_indicator,
        handle_indicator.settled_reveal(),
    );
    indicator.view(
        config,
        1.0,
        clock,
        hover_target,
        edge_indicator_view,
        panel_position,
    )
}

pub(crate) fn edge_indicator<'view, Message: 'view>(
    highest_alert_severity: Severity,
    handle: MenuHandleIndicator,
    reveal: f32,
) -> Element<'view, Message> {
    let thickness = ACCENT_STROKE_PX;
    let base = match handle.pointing {
        PointerPresence::PointedAt => 0.5,
        PointerPresence::NotPointedAt => 0.2,
    };
    let revealing_opacity = base + (1.0 - base) * reveal;
    match highest_alert_severity {
        Severity::Low | Severity::Normal if handle.menu_expansion == ExpansionState::Expanded => {
            activity_line(
                Axis::Horizontal,
                primary_color,
                revealing_opacity,
                thickness,
            )
        }
        Severity::Low | Severity::Normal if handle.pointing == PointerPresence::PointedAt => {
            activity_line(Axis::Horizontal, secondary_text_color, 1.0, thickness)
        }
        Severity::Low | Severity::Normal => Space::new()
            .width(Length::Fill)
            .height(Length::Fixed(thickness))
            .into(),
        severity @ (Severity::Warning | Severity::Critical) => {
            let tint = IndicatorStateTint {
                state: IndicatorState::from(severity),
                opacity: revealing_opacity,
            };
            activity_line(
                Axis::Horizontal,
                move |theme| tint.color(theme),
                1.0,
                thickness,
            )
        }
    }
}

fn indicators_row<'rows, 'view: 'rows>(
    config: &Config,
    group_identity: &IndicatorGroupIdentity,
    rows: impl IntoIterator<Item = &'rows GroupIndicator<'view>>,
    reveal: f32,
    clock: Clock,
    panel_position: Position,
) -> Element<'view, GroupEvent> {
    Row::with_children(rows.into_iter().map(|indicator| {
        indicator.view(
            config,
            reveal,
            clock,
            HoverTarget::Indicator(indicator.indicator_key(group_identity)),
            edge_indicator(
                indicator.shown_alert_severity().unwrap_or_default(),
                MenuHandleIndicator::default(),
                reveal,
            ),
            panel_position,
        )
    }))
    .spacing(config.theme.spacing.item_gap)
    .align_y(Alignment::Center)
    .into()
}

fn revealed_content<'view, Message: 'view>(
    config: &Config,
    reveal_fraction: f32,
    content: Element<'view, Message>,
) -> Element<'view, Message> {
    let gap = config.theme.spacing.item_gap;
    reveal(
        container(content).padding(Padding::ZERO.left(gap)),
        reveal_fraction,
        Axis::Horizontal,
    )
    .into()
}
