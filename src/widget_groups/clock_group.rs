use std::time::Duration;

use chrono::{DateTime, Local};
use iced::{
    Alignment, Element, Subscription, Task,
    futures::channel::mpsc::Sender,
    stream,
    widget::{Row, button},
};
use tokio::time::sleep;

use crate::{
    config::Config,
    wayland_layers::surface::Position,
    widgets::{
        flat_button,
        menu_group::{MenuGroup, MenuMessage},
        text::bar_text,
    },
};

const TIME_FORMAT: &str = "%H:%M:%S";
const DATE_FORMAT: &str = "%a %d %b";

pub struct ClockGroup {
    time_text: String,
    date_text: String,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone, Copy)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
    Menu(MenuMessage),
}

impl Default for ClockGroup {
    fn default() -> Self {
        let now = Local::now();
        Self {
            time_text: now.format(TIME_FORMAT).to_string(),
            date_text: now.format(DATE_FORMAT).to_string(),
            menu: MenuGroup::default(),
        }
    }
}

impl ClockGroup {
    pub fn update(&mut self, msg: ClockMessage) -> Task<ClockMessage> {
        match msg {
            ClockMessage::Tick(now) => {
                self.time_text = now.format(TIME_FORMAT).to_string();
                self.date_text = now.format(DATE_FORMAT).to_string();
            }
            ClockMessage::Menu(m) => {
                self.menu.update(m);
            }
        }
        Task::none()
    }

    pub fn subscription() -> Subscription<ClockMessage> {
        use iced::futures::SinkExt;
        Subscription::run(|| {
            stream::channel(1, |mut output: Sender<ClockMessage>| async move {
                loop {
                    sleep(until_next_boundary(Local::now(), period(TIME_FORMAT))).await;
                    if output.send(ClockMessage::Tick(Local::now())).await.is_err() {
                        return;
                    }
                }
            })
        })
    }

    pub fn view<'a>(&self, config: &Config, position: Position) -> Element<'a, ClockMessage> {
        let handle: Element<'a, ClockMessage> =
            button(bar_text(&config.font, self.time_text.clone()))
                .padding(0)
                .style(flat_button(config.palette.bar_foreground))
                .on_press(ClockMessage::Menu(MenuMessage::Toggle))
                .into();

        let content = Row::new()
            .push(bar_text(&config.font, self.date_text.clone()))
            .spacing(config.spacing.widget_spacing_px)
            .align_y(Alignment::Center);

        self.menu
            .view(config, position, handle, content.into(), ClockMessage::Menu)
    }
}

fn period(format: &str) -> Duration {
    let has_seconds = ["%S", "%T", "%X", "%r", "%s"]
        .iter()
        .any(|spec| format.contains(spec));
    if has_seconds {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(60)
    }
}

fn until_next_boundary(now: DateTime<Local>, period: Duration) -> Duration {
    let secs_into = now
        .timestamp()
        .rem_euclid(period.as_secs().cast_signed())
        .cast_unsigned();
    let subsec_ns = u64::from(now.timestamp_subsec_nanos()) % 1_000_000_000;
    Duration::from_secs(period.as_secs() - secs_into)
        .saturating_sub(Duration::from_nanos(subsec_ns))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn boundary_delay_completes_the_period() {
        let now = Local.timestamp_opt(1_000, 250_000_000).unwrap();
        assert_eq!(
            until_next_boundary(now, Duration::from_secs(1)),
            Duration::from_millis(750)
        );
        assert_eq!(
            until_next_boundary(now, Duration::from_secs(60)),
            Duration::from_millis(19750)
        );
    }

    #[test]
    fn exact_boundary_waits_a_full_period() {
        let now = Local.timestamp_opt(1_000, 0).unwrap();
        assert_eq!(
            until_next_boundary(now, Duration::from_secs(1)),
            Duration::from_secs(1)
        );
    }
}
