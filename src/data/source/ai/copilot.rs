use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use chrono::DateTime;
use serde::Deserialize;

use crate::data::source::{
    BarWidget, Glyph, Level, Occasional, Source, TooltipBody, Unit, Value as GaugeValue,
    ai::{level_from_cost, level_from_used, read_secret},
    command::{self, exists, require},
    http, row,
    simple_icons::SimpleIcon,
};

const USER_URL: &str = "https://api.github.com/copilot_internal/user";
const USER_AGENT: &str = "vetrata";

#[derive(Debug, Clone)]
pub enum Error {
    Secret,
    Fetch(http::Error),
    Api(u16),
    Decode(Arc<serde_json::Error>),
    NoQuota,
    Ccusage(command::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "GitHub token unreadable"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Api(status) => write!(f, "HTTP {status}"),
            Error::Decode(error) => write!(f, "unexpected response: {error}"),
            Error::NoQuota => write!(f, "no metered quota in response"),
            Error::Ccusage(error) => write!(f, "ccusage: {error}"),
        }
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        Error::Fetch(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Decode(Arc::new(error))
    }
}

impl From<command::Error> for Error {
    fn from(error: command::Error) -> Self {
        Error::Ccusage(error)
    }
}

#[derive(Deserialize)]
struct UserInfo {
    limited_user_quotas: Option<FreeQuotas>,
    token_based_billing: Option<bool>,
    quota_snapshots: Option<QuotaSnapshots>,
    copilot_plan: Option<String>,
    sku: Option<String>,
    limited_user_reset_date: Option<i64>,
    quota_reset_date_utc: Option<String>,
    quota_reset_date: Option<String>,
}

#[derive(Deserialize)]
struct FreeQuotas {
    chat: i64,
    completions: i64,
}

#[derive(Deserialize)]
struct QuotaSnapshots {
    premium_interactions: Option<QuotaSnapshot>,
    chat: Option<QuotaSnapshot>,
}

#[derive(Deserialize)]
struct QuotaSnapshot {
    has_quota: bool,
    entitlement: i64,
    remaining: i64,
    percent_remaining: Option<f64>,
}

#[derive(Deserialize)]
struct MonthlyReport {
    monthly: Vec<MonthEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MonthEntry {
    month: String,
    total_tokens: u64,
    total_cost: f64,
    models_used: Vec<String>,
}

impl UserInfo {
    fn metered_plan(&self) -> bool {
        self.limited_user_quotas.is_none() && self.token_based_billing != Some(true)
    }

    fn free_plan(&self) -> bool {
        self.limited_user_quotas.is_some()
    }

    fn plan(&self) -> String {
        self.copilot_plan
            .as_ref()
            .or(self.sku.as_ref())
            .filter(|plan| !plan.is_empty())
            .map_or_else(|| "unknown".to_owned(), ToOwned::to_owned)
    }

    fn metered_quota(&self) -> Option<(&QuotaSnapshot, &'static str)> {
        let snapshots = self.quota_snapshots.as_ref()?;
        usable(snapshots.premium_interactions.as_ref(), "Premium requests")
            .or_else(|| usable(snapshots.chat.as_ref(), "Chat"))
    }
}

impl QuotaSnapshot {
    #[allow(clippy::cast_possible_truncation)]
    fn percent_used(&self) -> i64 {
        100 - self.percent_remaining.map_or_else(
            || (self.remaining * 100) / self.entitlement,
            |percent| percent as i64,
        )
    }
}

fn usable<'a>(
    snapshot: Option<&'a QuotaSnapshot>,
    label: &'static str,
) -> Option<(&'a QuotaSnapshot, &'static str)> {
    snapshot
        .filter(|snap| snap.has_quota && snap.entitlement > 0 && snap.remaining >= 0)
        .map(|snap| (snap, label))
}

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotMetered {
    plan: String,
    quota: String,
    used: i64,
    limit: i64,
    remaining: i64,
    percent_used: i64,
    reset: Option<String>,
}

impl BarWidget for CopilotMetered {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn level(&self) -> Level {
        level_from_used(self.percent_used)
    }

    fn value(&self) -> Option<GaugeValue> {
        Some(GaugeValue::Gauge {
            percent: self.percent_used,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![
            row("Plan", self.plan.clone()),
            row("Source", "/copilot_internal/user"),
            row("Quota", self.quota.clone()),
            row("Used", self.used.to_string()),
            row("Limit", self.limit.to_string()),
            row("Remaining", self.remaining.to_string()),
        ];
        if let Some(reset) = &self.reset {
            rows.push(row("Resets", reset.clone()));
        }
        TooltipBody::rows(rows)
    }
}

impl Source for CopilotMetered {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "copilot-usage-metered";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn available(key: &PathBuf) -> bool {
        user_info(key).await.is_ok_and(|info| info.metered_plan())
    }

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let info = user_info(&key).await?;
        let (snapshot, label) = info.metered_quota().ok_or(Error::NoQuota)?;
        Ok(CopilotMetered {
            plan: info.plan(),
            quota: label.to_owned(),
            used: snapshot.entitlement - snapshot.remaining,
            limit: snapshot.entitlement,
            remaining: snapshot.remaining,
            percent_used: snapshot.percent_used(),
            reset: quota_reset(&info),
        })
    }
}

impl Occasional for CopilotMetered {
    fn present(&self) -> bool {
        self.percent_used > 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotFree {
    plan: String,
    chat: i64,
    completions: i64,
    reset: Option<String>,
}

impl BarWidget for CopilotFree {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.chat.to_string())
    }

    fn level(&self) -> Level {
        if self.chat <= 5 {
            Level::Critical
        } else if self.chat <= 20 {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![
            row("Plan", self.plan.clone()),
            row("Source", "/copilot_internal/user"),
            row("Chat", self.chat.to_string()),
            row("Completions", self.completions.to_string()),
        ];
        if let Some(reset) = &self.reset {
            rows.push(row("Resets", reset.clone()));
        }
        TooltipBody::rows(rows)
    }
}

impl Source for CopilotFree {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "copilot-usage-free";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn available(key: &PathBuf) -> bool {
        user_info(key).await.is_ok_and(|info| info.free_plan())
    }

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let info = user_info(&key).await?;
        let quotas = info.limited_user_quotas.as_ref().ok_or(Error::NoQuota)?;
        Ok(CopilotFree {
            plan: info.plan(),
            chat: quotas.chat,
            completions: quotas.completions,
            reset: info
                .limited_user_reset_date
                .filter(|epoch| *epoch > 0)
                .and_then(format_epoch),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CopilotCcusage {
    month: String,
    tokens: u64,
    cost: f64,
    models: String,
}

impl Occasional for CopilotCcusage {
    fn present(&self) -> bool {
        self.tokens > 0 || self.cost > 0.0
    }
}

impl BarWidget for CopilotCcusage {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn label(&self) -> Option<String> {
        Some(if self.cost > 0.0 {
            format!("${:.2}", self.cost)
        } else if self.tokens > 0 {
            "cli".to_owned()
        } else {
            String::new()
        })
    }

    fn level(&self) -> Level {
        level_from_cost(self.cost)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            row("Plan", "copilot-cli"),
            row("Source", "ccusage"),
            row("Scope", "local CLI session data"),
            row("Month", self.month.clone()),
            row("MonthTokens", self.tokens.to_string()),
            row("MonthCost", format!("${:.2}", self.cost)),
            row("Models", self.models.clone()),
        ])
    }
}

impl Source for CopilotCcusage {
    type Error = Error;
    type Key = ();

    const NAME: &'static str = "copilot-usage-ccusage";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn available((): &()) -> bool {
        exists("ccusage").await
    }

    async fn poll((): ()) -> Result<Self, Self::Error> {
        let raw = require("ccusage", &["copilot", "monthly", "--json"]).await?;
        let report: MonthlyReport = serde_json::from_str(&raw)?;
        let Some(entry) = report.monthly.into_iter().next_back() else {
            return Ok(CopilotCcusage::default());
        };
        Ok(CopilotCcusage {
            month: entry.month,
            tokens: entry.total_tokens,
            cost: entry.total_cost,
            models: entry.models_used.join(", "),
        })
    }
}

fn quota_reset(info: &UserInfo) -> Option<String> {
    [
        info.quota_reset_date_utc.as_ref(),
        info.quota_reset_date.as_ref(),
    ]
    .into_iter()
    .flatten()
    .find(|date| !date.is_empty())
    .map(ToOwned::to_owned)
}

async fn user_info(key: &Path) -> Result<UserInfo, Error> {
    let token = read_secret(key).ok_or(Error::Secret)?;
    let auth = format!("token {token}");
    let resp = http::get(
        USER_URL,
        &[
            ("Authorization", &auth),
            ("Accept", "application/json"),
            ("User-Agent", USER_AGENT),
        ],
        10,
    )
    .await?;
    if resp.status != 200 {
        return Err(Error::Api(resp.status));
    }
    Ok(serde_json::from_value(
        resp.body.ok_or(Error::Api(resp.status))?,
    )?)
}

fn format_epoch(epoch: i64) -> Option<String> {
    DateTime::from_timestamp(epoch, 0).map(|at| at.format("%Y-%m-%d").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_INFO: &str = r#"{
        "copilot_plan": "individual",
        "sku": null,
        "token_based_billing": false,
        "limited_user_quotas": null,
        "limited_user_reset_date": null,
        "quota_reset_date": "2026-09-01",
        "quota_reset_date_utc": "2026-09-01T00:00:00Z",
        "quota_snapshots": {
            "premium_interactions": {
                "has_quota": false, "entitlement": 0, "remaining": 0,
                "quota_remaining": 0.0, "percent_remaining": 0.0,
                "quota_reset_at": 0, "unlimited": false
            },
            "chat": {
                "has_quota": true, "entitlement": 200, "remaining": 150,
                "quota_remaining": 150.0, "percent_remaining": 75.0,
                "quota_reset_at": 0, "unlimited": false
            }
        }
    }"#;

    const FREE_USER_INFO: &str = r#"{
        "sku": "free",
        "limited_user_quotas": { "chat": 12, "completions": 900 },
        "limited_user_reset_date": 1788220800
    }"#;

    const MONTHLY: &str = r#"{
        "monthly": [{
            "month": "2026-08", "totalTokens": 4321, "totalCost": 1.25,
            "modelsUsed": ["gpt-5.4"], "inputTokens": 1, "outputTokens": 2
        }],
        "totals": {}
    }"#;

    #[test]
    fn metered_account_picks_the_chat_snapshot() {
        let info: UserInfo = serde_json::from_str(USER_INFO).unwrap();
        assert!(info.metered_plan());
        assert_eq!(info.plan(), "individual");
        let (snapshot, label) = info.metered_quota().unwrap();
        assert_eq!(label, "Chat");
        assert_eq!(snapshot.entitlement, 200);
        assert_eq!(snapshot.percent_used(), 25);
        assert_eq!(quota_reset(&info).as_deref(), Some("2026-09-01T00:00:00Z"));
    }

    #[test]
    fn free_account_is_not_metered() {
        let info: UserInfo = serde_json::from_str(FREE_USER_INFO).unwrap();
        assert!(!info.metered_plan());
        assert!(info.free_plan());
        let quotas = info.limited_user_quotas.unwrap();
        assert_eq!(quotas.chat, 12);
        assert_eq!(quotas.completions, 900);
    }

    #[test]
    fn token_based_billing_account_is_neither_metered_nor_free() {
        let info: UserInfo = serde_json::from_str(
            r#"{"limited_user_quotas": null, "token_based_billing": true,
                "quota_snapshots": {"chat": {"has_quota": true, "entitlement": 200,
                "remaining": 200, "quota_remaining": 200.0, "percent_remaining": 100.0}}}"#,
        )
        .unwrap();
        assert!(!info.metered_plan());
        assert!(!info.free_plan());
    }

    #[test]
    fn monthly_report_entry_decodes() {
        let report: MonthlyReport = serde_json::from_str(MONTHLY).unwrap();
        let entry = report.monthly.into_iter().next_back().unwrap();
        assert_eq!(entry.month, "2026-08");
        assert_eq!(entry.total_tokens, 4321);
        assert_eq!(entry.models_used, vec!["gpt-5.4".to_owned()]);
    }

    #[test]
    fn empty_monthly_report_is_not_present() {
        let report: MonthlyReport = serde_json::from_str(r#"{"monthly":[]}"#).unwrap();
        assert!(report.monthly.is_empty());
        assert!(!CopilotCcusage::default().present());
    }
}
