mod copilot_cli_monthly_spend;
mod copilot_free_quota_count;
mod copilot_quota_gauge;

use std::path::PathBuf;

use iced::{Subscription, Task};
use vetrata_config::Config;
use vetrata_sources::{
    SourceDelivery,
    ai::{
        copilot::{CopilotAccount, CopilotCcusage, CopilotFree, CopilotMetered},
        working_time_forecast::SpendBudget,
    },
    bus::DesktopBuses,
};

use crate::{
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::{first_delivery, source_subscription},
    widgets::{
        continuous_reading::ContinuousReading,
        simple_icons::SimpleIcon,
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopilotUsageTarget {
    Metered,
    Free,
    Cli,
}

impl From<CopilotUsageTarget> for HoverTarget {
    fn from(target: CopilotUsageTarget) -> Self {
        HoverTarget::CopilotUsage(target)
    }
}

pub struct CopilotUsageGroup {
    metered: ContinuousReading<CopilotMetered>,
    free: ContinuousReading<CopilotFree>,
    cli: ContinuousReading<CopilotCcusage>,
    gh_auth_token: Option<PathBuf>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum CopilotUsageMessage {
    Metered(SourceDelivery<CopilotMetered>),
    Free(SourceDelivery<CopilotFree>),
    Cli(SourceDelivery<CopilotCcusage>),
    Expansion(ExpansionMessage),
}

impl CopilotUsageGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(gh_auth_token: Option<PathBuf>) -> Self {
        Self {
            metered: ContinuousReading::default(),
            free: ContinuousReading::default(),
            cli: ContinuousReading::default(),
            gh_auth_token,
            expansion: GroupExpansion::default(),
        }
    }

    fn cli_budget(config: &Config) -> SpendBudget {
        SpendBudget::new(
            config,
            config.widget_groups.copilot_usage.monthly_budget_usd,
        )
    }

    fn account(&self, buses: &DesktopBuses) -> Option<CopilotAccount> {
        Some(CopilotAccount {
            gh_auth_token: self.gh_auth_token.clone()?,
            buses: buses.clone(),
        })
    }

    pub fn refresh(
        &self,
        target: CopilotUsageTarget,
        config: &Config,
        buses: Option<&DesktopBuses>,
    ) -> Task<CopilotUsageMessage> {
        let account = buses.and_then(|buses| self.account(buses));
        match target {
            CopilotUsageTarget::Metered => account.map_or_else(Task::none, |key| {
                first_delivery::<CopilotMetered>(key).map(CopilotUsageMessage::Metered)
            }),
            CopilotUsageTarget::Free => account.map_or_else(Task::none, |key| {
                first_delivery::<CopilotFree>(key).map(CopilotUsageMessage::Free)
            }),
            CopilotUsageTarget::Cli => first_delivery::<CopilotCcusage>(Self::cli_budget(config))
                .map(CopilotUsageMessage::Cli),
        }
    }

    pub fn subscription(config: &Config) -> Subscription<CopilotUsageMessage> {
        source_subscription::<CopilotCcusage>(Self::cli_budget(config))
            .map(CopilotUsageMessage::Cli)
    }

    pub fn account_subscription(&self, buses: &DesktopBuses) -> Subscription<CopilotUsageMessage> {
        Subscription::batch(self.account(buses).into_iter().flat_map(|account| {
            [
                source_subscription::<CopilotMetered>(account.clone())
                    .map(CopilotUsageMessage::Metered),
                source_subscription::<CopilotFree>(account).map(CopilotUsageMessage::Free),
            ]
        }))
    }

    pub fn update(&mut self, message: CopilotUsageMessage) -> Task<CopilotUsageMessage> {
        match message {
            CopilotUsageMessage::Metered(reading) => self.metered.receive_delivery(reading),
            CopilotUsageMessage::Free(reading) => self.free.receive_delivery(reading),
            CopilotUsageMessage::Cli(reading) => self.cli.receive_delivery(reading),
            CopilotUsageMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: CopilotUsageTarget) -> Option<Tooltip> {
        match target {
            CopilotUsageTarget::Metered => self.metered.tooltip(),
            CopilotUsageTarget::Free => self.free.tooltip(),
            CopilotUsageTarget::Cli => self.cli.tooltip(),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, CopilotUsageMessage> {
        GroupContent::new(
            &self.expansion,
            CopilotUsageMessage::Expansion,
            [
                self.metered.group_item(CopilotUsageTarget::Metered),
                self.free.group_item(CopilotUsageTarget::Free),
                self.cli.group_item(CopilotUsageTarget::Cli),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon {
            glyph: SimpleIcon::GithubCopilot.into(),
            handle: GroupHandle::CopilotUsage,
        })
        .preview(self.metered.display().map(PreviewedValue::Plain))
    }
}
