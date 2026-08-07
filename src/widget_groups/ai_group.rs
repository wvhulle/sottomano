use iced::{
    Element, Subscription,
    widget::{Row, button},
};

use crate::{
    config::{Config, SecretPaths},
    data::source::{
        Tooltip,
        ai::{
            ApiSource,
            claude::{ClaudeCcusage, ClaudeOauth},
            copilot::{CopilotCcusage, CopilotFree, CopilotMetered},
            openrouter::OpenRouter,
        },
        font_awesome::Solid,
    },
    message_subscriptions::poll_arg,
    wayland_layers::{
        surface::Position,
        tooltip::{CarriesTooltip, HoverTarget, TooltipMessage},
    },
    widget_groups::{pills::Pills, source_cell::SourceCell},
    widgets::{
        flat_button,
        menu_group::{MenuGroup, MenuMessage},
    },
};

pub struct AiGroup {
    openrouter: SourceCell<OpenRouter>,
    claude_oauth: SourceCell<ClaudeOauth>,
    claude_ccusage: SourceCell<ClaudeCcusage>,
    copilot_metered: SourceCell<CopilotMetered>,
    copilot_free: SourceCell<CopilotFree>,
    copilot_cli: SourceCell<CopilotCcusage>,
    secrets: SecretPaths,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum AiMessage {
    OpenRouter(Option<OpenRouter>),
    ClaudeOauth(Option<ClaudeOauth>),
    ClaudeCcusage(Option<ClaudeCcusage>),
    CopilotMetered(Option<CopilotMetered>),
    CopilotFree(Option<CopilotFree>),
    CopilotCli(Option<CopilotCcusage>),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
}

impl CarriesTooltip for AiMessage {
    fn into_tooltip(self) -> Result<TooltipMessage, Self> {
        match self {
            Self::Tooltip(tooltip) => Ok(tooltip),
            message => Err(message),
        }
    }

    fn from_tooltip(tooltip: TooltipMessage) -> Self {
        Self::Tooltip(tooltip)
    }
}

impl AiGroup {
    pub fn new(menu_expand_ms: u64, secrets: SecretPaths) -> Self {
        Self {
            openrouter: SourceCell::default(),
            claude_oauth: SourceCell::default(),
            claude_ccusage: SourceCell::default(),
            copilot_metered: SourceCell::default(),
            copilot_free: SourceCell::default(),
            copilot_cli: SourceCell::default(),
            secrets,
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn subscription(&self) -> Subscription<AiMessage> {
        let secrets = &self.secrets;
        let mut subscriptions = Vec::new();
        if secrets.openrouter_management.is_some() {
            subscriptions.push(
                poll_arg::<OpenRouter>(secrets.openrouter_management.clone())
                    .map(AiMessage::OpenRouter),
            );
        }
        if secrets.claude_code_oauth.is_some() {
            subscriptions.push(
                poll_arg::<ClaudeOauth>(secrets.claude_code_oauth.clone())
                    .map(AiMessage::ClaudeOauth),
            );
            subscriptions.push(
                poll_arg::<ClaudeCcusage>(secrets.claude_code_oauth.clone())
                    .map(AiMessage::ClaudeCcusage),
            );
        }
        if secrets.gh_auth_token.is_some() {
            subscriptions.push(
                poll_arg::<CopilotMetered>(secrets.gh_auth_token.clone())
                    .map(AiMessage::CopilotMetered),
            );
            subscriptions.push(
                poll_arg::<CopilotFree>(secrets.gh_auth_token.clone()).map(AiMessage::CopilotFree),
            );
        }
        subscriptions.push(poll_arg::<CopilotCcusage>(None).map(AiMessage::CopilotCli));
        Subscription::batch(subscriptions)
    }

    pub fn update(&mut self, message: AiMessage) {
        match message {
            AiMessage::OpenRouter(reading) => self.openrouter.load(reading),
            AiMessage::ClaudeOauth(reading) => self.claude_oauth.load(reading),
            AiMessage::ClaudeCcusage(reading) => self.claude_ccusage.load(reading),
            AiMessage::CopilotMetered(reading) => self.copilot_metered.load(reading),
            AiMessage::CopilotFree(reading) => self.copilot_free.load(reading),
            AiMessage::CopilotCli(reading) => self.copilot_cli.load(reading),
            AiMessage::Menu(menu) => {
                self.menu.update(menu);
            }
            AiMessage::Tooltip(_) => {}
        }
    }

    fn configured(&self) -> bool {
        let secrets = &self.secrets;
        secrets.openrouter_management.is_some()
            || secrets.claude_code_oauth.is_some()
            || secrets.gh_auth_token.is_some()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::AiOpenRouter => tooltip_of(&self.openrouter),
            HoverTarget::AiClaudeOauth => tooltip_of(&self.claude_oauth),
            HoverTarget::AiClaudeCcusage => tooltip_of(&self.claude_ccusage),
            HoverTarget::AiCopilotMetered => tooltip_of(&self.copilot_metered),
            HoverTarget::AiCopilotFree => tooltip_of(&self.copilot_free),
            HoverTarget::AiCopilotCli => tooltip_of(&self.copilot_cli),
            _ => None,
        }
    }

    pub fn view<'a>(&self, config: &Config, position: Position) -> Option<Element<'a, AiMessage>> {
        let reveal = self.menu.reveal();
        let pills = Pills::new()
            .alpha(reveal)
            .push(self.claude_oauth.get(), HoverTarget::AiClaudeOauth)
            .push(self.claude_ccusage.get(), HoverTarget::AiClaudeCcusage)
            .push(self.copilot_metered.get(), HoverTarget::AiCopilotMetered)
            .push(self.copilot_free.get(), HoverTarget::AiCopilotFree)
            .push(self.copilot_cli.get(), HoverTarget::AiCopilotCli)
            .push(self.openrouter.get(), HoverTarget::AiOpenRouter)
            .row(config);

        if !self.configured() && pills.is_none() {
            return None;
        }

        let handle = button(robot_handle(config))
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(AiMessage::Menu(MenuMessage::Toggle))
            .into();
        let content = pills.unwrap_or_else(Row::new);
        Some(self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            AiMessage::Menu,
        ))
    }
}

fn tooltip_of<S: ApiSource>(cell: &SourceCell<S>) -> Option<Tooltip> {
    let widget = cell.get()?;
    let tooltip = Tooltip {
        body: widget.tooltip(),
        footer: cell.footer(S::INTERVAL),
    };
    (!tooltip.is_empty()).then_some(tooltip)
}

fn robot_handle<'a, Message: 'a>(config: &Config) -> Element<'a, Message> {
    Solid::Robot
        .widget()
        .size(config.font.icon_size_px)
        .color(config.palette.bar_foreground)
        .into()
}
