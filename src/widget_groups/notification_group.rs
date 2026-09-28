use iced::{
    Alignment, Element,
    widget::{Row, button},
};

use crate::{
    config::Config,
    state::NotificationState,
    wayland_layers::surface::Position,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        flat_button,
        menu_group::{MenuGroup, MenuMessage},
        phosphor::{Phosphor, Weight},
        source_reading::source_reading,
        tooltip_body::TooltipBody,
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum NotificationToggle {
    DoNotDisturb,
    Sound,
    SpatialLayout,
}

#[derive(Debug, Clone, Copy)]
pub enum NotificationGroupMessage {
    Menu(MenuMessage),
    Toggle(NotificationToggle),
}

#[derive(Default)]
pub struct NotificationGroup {
    pub(crate) menu: MenuGroup,
}

impl NotificationGroup {
    pub fn view<'a>(
        &self,
        config: &'a Config,
        notif_state: &NotificationState,
        position: Position,
        live: usize,
    ) -> Element<'a, NotificationGroupMessage> {
        let status = NotificationStatus {
            dnd: notif_state.dnd,
            live,
        };
        let dnd = notif_state.dnd;
        let sound = notif_state.sound;
        let spatial_layout = notif_state.spatial_layout;

        let face =
            move || source_reading(config, Some(&status), 1.0).unwrap_or_else(|| Row::new().into());
        let content = move |_reveal: f32| {
            let do_not_disturb = toggle(
                config,
                Phosphor::Prohibit(Weight::Regular),
                WidgetState::of_toggle(dnd),
                NotificationToggle::DoNotDisturb,
            );
            let sound = toggle(
                config,
                if sound {
                    Phosphor::Bell(Weight::Regular)
                } else {
                    Phosphor::BellSlash(Weight::Regular)
                },
                WidgetState::of_toggle(sound),
                NotificationToggle::Sound,
            );
            let spatial_layout = toggle(
                config,
                if spatial_layout {
                    Phosphor::TextAlignCenter(Weight::Regular)
                } else {
                    Phosphor::TextAlignRight(Weight::Regular)
                },
                WidgetState::of_toggle(spatial_layout),
                NotificationToggle::SpatialLayout,
            );
            Row::new()
                .push(do_not_disturb)
                .push(sound)
                .push(spatial_layout)
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into()
        };

        self.menu
            .slot(face, content, NotificationGroupMessage::Menu)
            .view(config, position)
    }
}

#[derive(Clone, Copy)]
struct NotificationStatus {
    dnd: bool,
    live: usize,
}

impl BarWidget for NotificationStatus {
    fn icon(&self) -> Glyph {
        if self.dnd {
            Phosphor::ChatCircleSlash(self.state().weight()).into()
        } else {
            Phosphor::ChatCircle(self.state().weight()).into()
        }
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.live).ok().filter(|live| *live > 0)
    }

    fn state(&self) -> WidgetState {
        if self.dnd {
            WidgetState::Off
        } else if self.live > 0 {
            WidgetState::On
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(Vec::new())
    }
}

fn toggle<'a>(
    config: &Config,
    icon: Phosphor,
    state: WidgetState,
    toggle: NotificationToggle,
) -> Element<'a, NotificationGroupMessage> {
    let color = state.color(&config.palette);
    button(Glyph::from(icon.with_weight(state.weight())).view(config.font.icon_size_px, color))
        .padding(0)
        .style(flat_button(color))
        .on_press(NotificationGroupMessage::Toggle(toggle))
        .into()
}
