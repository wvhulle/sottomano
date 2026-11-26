use futures::Stream;
use serde::Deserialize;

use super::steam_log_lines::SteamAppId;
use crate::{SnapshotDelivery, SnapshotSource, system_protocol_clients::http_json_client};

const PROTONDB_SUMMARIES_API: &str = "https://www.protondb.com/api/v1/reports/summaries";
const NOT_FOUND_STATUS: u16 = 404;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProtonDbTier {
    Platinum,
    Gold,
    Silver,
    Bronze,
    Borked,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportConfidence {
    Strong,
    Good,
    Moderate,
    Low,
    Inadequate,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtonDbSummary {
    pub tier: ProtonDbTier,
    pub best_reported_tier: ProtonDbTier,
    pub trending_tier: ProtonDbTier,
    pub confidence: ReportConfidence,
    pub score: f64,
    pub total: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProtonDbCompatibility {
    Rated(ProtonDbSummary),
    Unreported,
}

#[must_use]
pub fn compatibility_summary_url(app_id: SteamAppId) -> String {
    format!("{PROTONDB_SUMMARIES_API}/{}.json", app_id.number)
}

#[must_use]
pub fn compatibility_page_url(app_id: SteamAppId) -> String {
    format!("https://www.protondb.com/app/{}", app_id.number)
}

async fn fetched_compatibility(
    app_id: SteamAppId,
) -> Result<ProtonDbCompatibility, http_json_client::Error> {
    let response = http_json_client::get(&compatibility_summary_url(app_id), &[]).await?;
    match response.status {
        200 => Ok(ProtonDbCompatibility::Rated(http_json_client::decode(
            response.body,
        )?)),
        NOT_FOUND_STATUS => Ok(ProtonDbCompatibility::Unreported),
        status => Err(http_json_client::Error::status(status, response.body)),
    }
}

impl SnapshotSource for ProtonDbCompatibility {
    type Error = http_json_client::Error;
    type StreamInputs = SteamAppId;

    const NAME: &'static str = "On ProtonDB";

    fn delivery_stream(app_id: SteamAppId) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::single_read_deliveries(fetched_compatibility(app_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_reads_the_tiers_confidence_and_report_count() {
        let summary: ProtonDbSummary = serde_json::from_str(
            r#"{
                "bestReportedTier": "platinum",
                "confidence": "strong",
                "score": 0.91,
                "tier": "platinum",
                "total": 761,
                "trendingTier": "gold"
            }"#,
        )
        .unwrap();
        assert_eq!(summary.tier, ProtonDbTier::Platinum);
        assert_eq!(summary.trending_tier, ProtonDbTier::Gold);
        assert_eq!(summary.confidence, ReportConfidence::Strong);
        assert_eq!(summary.total, 761);
    }

    #[test]
    fn an_unknown_tier_is_rejected() {
        assert!(serde_json::from_str::<ProtonDbTier>(r#""diamond""#).is_err());
    }

    #[test]
    fn summary_url_points_at_the_summaries_api() {
        assert_eq!(
            compatibility_summary_url(SteamAppId { number: 1_145_360 }),
            "https://www.protondb.com/api/v1/reports/summaries/1145360.json"
        );
    }
}
