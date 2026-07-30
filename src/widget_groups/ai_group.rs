//! odin's `WidgetGroup/AiUsage.qml`: a robot-glyph handle that unfolds a row of
//! AI-provider usage/spend pills to its right. Each pill is one
//! [`ApiSource`](crate::data::source::ai) reading, polled with its own secret
//! file path. The whole group hides until at least one provider has something
//! to show.

use iced::{
    Alignment, Element, Subscription,
    widget::{Row, mouse_area},
};

use crate::{
    config::resolved::{ResolvedConfig, SecretPaths},
    data::source::{
        BarWidget, Tooltip,
        ai::{
            ApiSource,
            claude::{ClaudeCcusage, ClaudeOauth},
            copilot::{CopilotCcusage, CopilotFree, CopilotMetered},
            openrouter::OpenRouter,
        },
    },
    message_subscriptions::poll_arg,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widget_groups::source_cell::SourceCell,
    widgets::{
        icon::Icon,
        icon_gauge::source_reading,
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

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, AiMessage>> {
        let reveal = self.menu.reveal();
        let pills: Vec<Element<'a, AiMessage>> = [
            self.claude_oauth
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::AiClaudeOauth)),
            self.claude_ccusage
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::AiClaudeCcusage)),
            self.copilot_metered
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::AiCopilotMetered)),
            self.copilot_free
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::AiCopilotFree)),
            self.copilot_cli
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::AiCopilotCli)),
            self.openrouter
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::AiOpenRouter)),
        ]
        .into_iter()
        .flatten()
        .filter_map(|(widget, target)| {
            let reading = source_reading(config, Some(widget), reveal)?;
            Some(hoverable(reading, target, AiMessage::Tooltip))
        })
        .collect();

        if !self.configured() && pills.is_empty() {
            return None;
        }

        let handle = mouse_area(robot_handle(config))
            .on_press(AiMessage::Menu(MenuMessage::Toggle))
            .into();
        let content = Row::with_children(pills)
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);
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

fn robot_handle<'a, Message: 'a>(config: &ResolvedConfig) -> Element<'a, Message> {
    Icon::Robot
        .solid()
        .size(config.font.icon_size_px)
        .color(config.palette.bar_foreground)
        .into()
}
