use iced::{
    Element, Subscription, Task,
    widget::{Row, button},
};

use crate::{
    config::{Config, SecretPaths},
    data::{
        last_poll::LastPoll,
        source::{
            Source, Tooltip,
            ai::{
                claude::{self, ClaudeCcusage, ClaudeOauth},
                copilot::{self, CopilotCcusage, CopilotFree, CopilotMetered},
                openrouter::{self, OpenRouter},
            },
            font_awesome::Solid,
        },
    },
    message_subscriptions::polling,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        flat_button,
        menu_group::{MenuGroup, MenuMessage},
        reading_row::{Reading, ReadingRow},
    },
};

pub struct AiGroup {
    openrouter: LastPoll<OpenRouter>,
    claude_oauth: LastPoll<ClaudeOauth>,
    claude_ccusage: LastPoll<ClaudeCcusage>,
    copilot_metered: LastPoll<CopilotMetered>,
    copilot_free: LastPoll<CopilotFree>,
    copilot_cli: LastPoll<CopilotCcusage>,
    secrets: SecretPaths,
    available: Available,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Available {
    claude_ccusage: bool,
    copilot: CopilotAvailable,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CopilotAvailable {
    metered: bool,
    free: bool,
    cli: bool,
}

#[derive(Debug, Clone)]
pub enum AiMessage {
    Available(Available),
    OpenRouter(Result<OpenRouter, openrouter::Error>),
    ClaudeOauth(Result<ClaudeOauth, claude::Error>),
    ClaudeCcusage(Result<ClaudeCcusage, claude::Error>),
    CopilotMetered(Result<CopilotMetered, copilot::Error>),
    CopilotFree(Result<CopilotFree, copilot::Error>),
    CopilotCli(Result<CopilotCcusage, copilot::Error>),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
}

impl AiGroup {
    pub fn new(secrets: SecretPaths) -> Self {
        Self {
            openrouter: LastPoll::default(),
            claude_oauth: LastPoll::default(),
            claude_ccusage: LastPoll::default(),
            copilot_metered: LastPoll::default(),
            copilot_free: LastPoll::default(),
            copilot_cli: LastPoll::default(),
            secrets,
            available: Available::default(),
            menu: MenuGroup::default(),
        }
    }

    pub fn probe(&self) -> Task<AiMessage> {
        let secrets = self.secrets.clone();
        Task::future(async move {
            let claude_ccusage = match &secrets.claude_code_oauth {
                Some(key) => ClaudeCcusage::available(key).await,
                None => false,
            };
            let (metered, free) = match &secrets.gh_auth_token {
                Some(key) => (
                    CopilotMetered::available(key).await,
                    CopilotFree::available(key).await,
                ),
                None => (false, false),
            };
            AiMessage::Available(Available {
                claude_ccusage,
                copilot: CopilotAvailable {
                    metered,
                    free,
                    cli: CopilotCcusage::available(&()).await,
                },
            })
        })
    }

    pub fn subscription(&self) -> Subscription<AiMessage> {
        let secrets = &self.secrets;
        let available = self.available;
        let mut subscriptions = Vec::new();
        if let Some(key) = secrets.openrouter_management.clone() {
            subscriptions.push(polling::<OpenRouter>(key).map(AiMessage::OpenRouter));
        }
        if let Some(key) = secrets.claude_code_oauth.clone() {
            subscriptions.push(polling::<ClaudeOauth>(key.clone()).map(AiMessage::ClaudeOauth));
            if available.claude_ccusage {
                subscriptions.push(polling::<ClaudeCcusage>(key).map(AiMessage::ClaudeCcusage));
            }
        }
        if let Some(key) = secrets.gh_auth_token.clone() {
            if available.copilot.metered {
                subscriptions
                    .push(polling::<CopilotMetered>(key.clone()).map(AiMessage::CopilotMetered));
            }
            if available.copilot.free {
                subscriptions.push(polling::<CopilotFree>(key).map(AiMessage::CopilotFree));
            }
        }
        if available.copilot.cli {
            subscriptions.push(polling::<CopilotCcusage>(()).map(AiMessage::CopilotCli));
        }
        Subscription::batch(subscriptions)
    }

    pub fn update(&mut self, message: AiMessage) -> Task<AiMessage> {
        match message {
            AiMessage::Available(available) => self.available = available,
            AiMessage::OpenRouter(reading) => self.openrouter.receive(reading),
            AiMessage::ClaudeOauth(reading) => self.claude_oauth.receive(reading),
            AiMessage::ClaudeCcusage(reading) => self.claude_ccusage.receive(reading),
            AiMessage::CopilotMetered(reading) => self.copilot_metered.receive(reading),
            AiMessage::CopilotFree(reading) => self.copilot_free.receive(reading),
            AiMessage::CopilotCli(reading) => self.copilot_cli.receive(reading),
            AiMessage::Menu(menu) => {
                self.menu.update(menu);
            }
            AiMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    fn configured(&self) -> bool {
        let secrets = &self.secrets;
        secrets.openrouter_management.is_some()
            || secrets.claude_code_oauth.is_some()
            || secrets.gh_auth_token.is_some()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::AiOpenRouter => self.openrouter.tooltip(),
            HoverTarget::AiClaudeOauth => self.claude_oauth.tooltip(),
            HoverTarget::AiClaudeCcusage => self.claude_ccusage.tooltip(),
            HoverTarget::AiCopilotMetered => self.copilot_metered.tooltip(),
            HoverTarget::AiCopilotFree => self.copilot_free.tooltip(),
            HoverTarget::AiCopilotCli => self.copilot_cli.tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(&self, config: &Config, position: Position) -> Option<Element<'a, AiMessage>> {
        let reveal = self.menu.reveal();
        let readings = ReadingRow::new()
            .opacity(reveal)
            .push(Reading::new(
                self.claude_oauth.present(),
                HoverTarget::AiClaudeOauth,
            ))
            .push(Reading::new(
                self.claude_ccusage.display(),
                HoverTarget::AiClaudeCcusage,
            ))
            .push(Reading::new(
                self.copilot_metered.present(),
                HoverTarget::AiCopilotMetered,
            ))
            .push(Reading::new(
                self.copilot_free.display(),
                HoverTarget::AiCopilotFree,
            ))
            .push(Reading::new(
                self.copilot_cli.present(),
                HoverTarget::AiCopilotCli,
            ))
            .push(Reading::new(
                self.openrouter.present(),
                HoverTarget::AiOpenRouter,
            ))
            .row(config, AiMessage::Tooltip);

        if !self.configured() && readings.is_none() {
            return None;
        }

        let handle = button(robot_handle(config))
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(AiMessage::Menu(MenuMessage::Toggle))
            .into();
        let content = readings.unwrap_or_else(Row::new);
        Some(
            self.menu
                .view(config, position, handle, content.into(), AiMessage::Menu),
        )
    }
}

fn robot_handle<'a, Message: 'a>(config: &Config) -> Element<'a, Message> {
    Solid::Robot
        .glyph()
        .size(config.font.icon_size_px)
        .color(config.palette.bar_foreground)
        .into()
}
