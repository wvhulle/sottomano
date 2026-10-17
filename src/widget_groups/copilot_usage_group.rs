mod copilot_cli_monthly_spend;
mod copilot_free_quota_count;
mod copilot_quota_gauge;

use std::path::PathBuf;

use iced::{Subscription, Task};

use crate::{
    config::Config,
    data::source::{
        SourceDelivery,
        ai::{
            copilot::{CopilotCcusage, CopilotFree, CopilotMetered},
            working_time_forecast::SpendBudget,
        },
    },
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::{ForcedRefresh, source_subscription},
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

    pub fn new(gh_auth_token: Option<PathBuf>) -> Self {
        Self {
            metered: ContinuousReading::default(),
            free: ContinuousReading::default(),
            cli: ContinuousReading::default(),
            gh_auth_token,
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(
        &self,
        config: &Config,
        forced: Option<ForcedRefresh>,
    ) -> Subscription<CopilotUsageMessage> {
        let account = self.gh_auth_token.clone().into_iter().flat_map(|key| {
            [
                source_subscription::<CopilotMetered>(key.clone(), forced)
                    .map(CopilotUsageMessage::Metered),
                source_subscription::<CopilotFree>(key, forced).map(CopilotUsageMessage::Free),
            ]
        });
        let budget = SpendBudget::new(
            config,
            config.widget_groups.copilot_usage.monthly_budget_usd,
        );
        Subscription::batch(account.chain([
            source_subscription::<CopilotCcusage>(budget, forced).map(CopilotUsageMessage::Cli),
        ]))
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
