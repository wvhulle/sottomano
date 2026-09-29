pub mod amount_text;
pub mod claude_code_daily_spend;
pub mod claude_subscription_gauge;
pub mod copilot_cli_monthly_spend;
pub mod copilot_free_quota_count;
pub mod copilot_quota_gauge;
pub mod openrouter_credit_spend;
pub mod spend_to_level;

use iced::{Element, Subscription, Task, widget::Row};

use crate::{
    config::{Config, SecretPaths},
    data::source::{
        Source,
        ai::{
            claude::{self, ClaudeCcusage, ClaudeOauth},
            copilot::{self, CopilotCcusage, CopilotFree, CopilotMetered},
            openrouter::{self, OpenRouter},
        },
    },
    message_subscriptions::polling,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        last_poll::LastPoll,
        menu_group::{MenuGroup, MenuMessage},
        reading_row::{Reading, ReadingRow},
        tooltip_body::Tooltip,
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
    claude: ClaudeAvailable,
    copilot: CopilotAvailable,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeAvailable {
    oauth: bool,
    ccusage: bool,
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
            let subscription = match &secrets.claude_code_oauth {
                Some(key) => ClaudeOauth::available(key).await,
                None => false,
            };
            let claude = ClaudeAvailable {
                oauth: subscription,
                ccusage: !subscription && ClaudeCcusage::available(&()).await,
            };
            let (metered, free) = match &secrets.gh_auth_token {
                Some(key) => (
                    CopilotMetered::available(key).await,
                    CopilotFree::available(key).await,
                ),
                None => (false, false),
            };
            AiMessage::Available(Available {
                claude,
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
        if let Some(key) = secrets.claude_code_oauth.clone()
            && available.claude.oauth
        {
            subscriptions.push(polling::<ClaudeOauth>(key).map(AiMessage::ClaudeOauth));
        }
        if available.claude.ccusage {
            subscriptions.push(polling::<ClaudeCcusage>(()).map(AiMessage::ClaudeCcusage));
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

    fn readings(&self) -> [Reading<'_, AiMessage>; 6] {
        [
            Reading::new(self.claude_oauth.present(), HoverTarget::AiClaudeOauth),
            Reading::new(self.claude_ccusage.display(), HoverTarget::AiClaudeCcusage),
            Reading::new(
                self.copilot_metered.present(),
                HoverTarget::AiCopilotMetered,
            ),
            Reading::new(self.copilot_free.display(), HoverTarget::AiCopilotFree),
            Reading::new(self.copilot_cli.present(), HoverTarget::AiCopilotCli),
            Reading::new(self.openrouter.present(), HoverTarget::AiOpenRouter),
        ]
    }

    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, AiMessage>> {
        let face_index = self.readings().iter().position(Reading::is_present)?;

        let face = move || {
            self.readings()
                .into_iter()
                .skip(face_index)
                .take(1)
                .fold(ReadingRow::new(), ReadingRow::push)
                .row(config, AiMessage::Tooltip)
                .unwrap_or_else(Row::new)
                .into()
        };
        let content = move |reveal: f32| {
            self.readings()
                .into_iter()
                .skip(face_index + 1)
                .fold(ReadingRow::new().opacity(reveal), ReadingRow::push)
                .row(config, AiMessage::Tooltip)
                .unwrap_or_else(Row::new)
                .into()
        };
        Some(
            self.menu
                .slot(face, content, AiMessage::Menu)
                .view(config, position),
        )
    }
}
