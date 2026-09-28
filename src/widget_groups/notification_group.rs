use iced::{
    Alignment, Element,
    widget::{Row, button},
};

use crate::{
    config::Config,
    state::NotificationState,
    wayland_layers::surface::Position,
    widgets::{
        DISABLED_ALPHA,
        bar_widget::{BarWidget, Glyph},
        flat_button,
        font_awesome::{FontAwesome, Regular, Solid},
        menu_group::{MenuGroup, MenuMessage},
        source_reading::source_reading,
        tooltip_body::TooltipBody,
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
                Solid::Ban.into(),
                dnd,
                NotificationToggle::DoNotDisturb,
            );
            let sound = toggle(
                config,
                if sound {
                    Solid::Bell.into()
                } else {
                    Solid::BellSlash.into()
                },
                sound,
                NotificationToggle::Sound,
            );
            let spatial_layout = toggle(
                config,
                if spatial_layout {
                    Solid::AlignCenter.into()
                } else {
                    Solid::AlignRight.into()
                },
                spatial_layout,
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
            Solid::CommentSlash.into()
        } else if self.live > 0 {
            Solid::Comment.into()
        } else {
            Regular::Comment.into()
        }
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.live).ok().filter(|live| *live > 0)
    }

    fn disabled(&self) -> bool {
        self.dnd
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(Vec::new())
    }
}

fn toggle<'a>(
    config: &Config,
    icon: FontAwesome,
    active: bool,
    toggle: NotificationToggle,
) -> Element<'a, NotificationGroupMessage> {
    let opacity = if active { 1.0 } else { DISABLED_ALPHA };
    let color = config.palette.bar_foreground.scale_alpha(opacity);
    button(icon.view(config.font.icon_size_px, color))
        .padding(0)
        .style(flat_button(color))
        .on_press(NotificationGroupMessage::Toggle(toggle))
        .into()
}
