use std::{path::PathBuf, str::FromStr, time::Duration};

use futures::{Stream, try_join};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, de};

use crate::{
    SnapshotDelivery, SnapshotSource,
    ai_usage_spend::{
        openrouter_credits::Error,
        spend_money::{Money, usd},
    },
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::internet_connectivity_gate::{self, OnlineSourceInputs},
    secret_file_reads::read_secret,
    system_protocol_clients::http_json_client,
};

const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const OPENROUTER_BENCHMARKS_URL: &str = "https://openrouter.ai/api/v1/benchmarks?source=artificial-analysis&task_type=intelligence&max_results=100";
const OPENROUTER_SITE_URL: &str = "https://openrouter.ai";
const TOOL_CALLING_PARAMETER: &str = "tools";
const AGENTIC_MINIMUM_CONTEXT_TOKENS: u64 = 131_072;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelRankingInputs {
    pub management_key: PathBuf,
    pub max_blended_per_million: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoredModel {
    pub model: String,
    pub intelligence_index: f64,
    pub input_per_million: Money,
    pub output_per_million: Money,
    pub context_tokens: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenRouterModelRankings {
    pub max_blended_per_million: Money,
    pub best_first: Vec<ScoredModel>,
    pub unscored_count: usize,
}

#[derive(Deserialize)]
struct ModelListing<T> {
    data: Vec<T>,
}

#[derive(Deserialize)]
struct OpenRouterModel {
    id: String,
    #[serde(default)]
    canonical_slug: String,
    #[serde(default)]
    context_length: u64,
    pricing: TokenPricing,
    #[serde(
        default,
        rename = "supported_parameters",
        deserialize_with = "lists_tool_calling"
    )]
    tool_calling: bool,
}

#[derive(Deserialize)]
struct TokenPricing {
    #[serde(rename = "prompt", deserialize_with = "usd_per_million_tokens")]
    input_per_million: Money,
    #[serde(rename = "completion", deserialize_with = "usd_per_million_tokens")]
    output_per_million: Money,
}

#[derive(Deserialize)]
struct ModelBenchmarkRow {
    model_permaslug: String,
    intelligence_index: Option<f64>,
}

impl TokenPricing {
    fn blended_per_million(&self) -> Decimal {
        (self.input_per_million.amount() * Decimal::from(3) + self.output_per_million.amount())
            / Decimal::from(4)
    }
}

impl OpenRouterModel {
    fn scored(self, benchmark_rows: &[ModelBenchmarkRow]) -> Option<ScoredModel> {
        let intelligence_index = benchmark_rows
            .iter()
            .filter(|benchmark_row| benchmark_row.model_permaslug == self.canonical_slug)
            .filter_map(|benchmark_row| benchmark_row.intelligence_index)
            .reduce(f64::max)?;
        Some(ScoredModel {
            intelligence_index,
            input_per_million: self.pricing.input_per_million,
            output_per_million: self.pricing.output_per_million,
            context_tokens: self.context_length,
            model: self.id,
        })
    }
}

impl ScoredModel {
    #[must_use]
    pub fn openrouter_page(&self) -> String {
        format!("{OPENROUTER_SITE_URL}/{}", self.model)
    }
}

impl OpenRouterModelRankings {
    fn ranked(
        openrouter_models: Vec<OpenRouterModel>,
        benchmark_rows: &[ModelBenchmarkRow],
        max_blended_per_million: Decimal,
    ) -> Self {
        let mut affordable: Vec<OpenRouterModel> = openrouter_models
            .into_iter()
            .filter(|model| {
                let canonical = !model.id.contains(':') && !model.id.starts_with('~');
                model.tool_calling
                    && model.pricing.input_per_million.is_positive()
                    && model.pricing.output_per_million.is_positive()
                    && canonical
                    && model.context_length >= AGENTIC_MINIMUM_CONTEXT_TOKENS
            })
            .filter(|model| model.pricing.blended_per_million() <= max_blended_per_million)
            .collect();
        affordable.sort_by(|left, right| left.id.cmp(&right.id));
        affordable.dedup_by(|left, right| left.id == right.id);
        let affordable_count = affordable.len();
        let mut best_first: Vec<ScoredModel> = affordable
            .into_iter()
            .filter_map(|model| model.scored(benchmark_rows))
            .collect();
        best_first
            .sort_by(|left, right| right.intelligence_index.total_cmp(&left.intelligence_index));
        Self {
            max_blended_per_million: usd(max_blended_per_million),
            unscored_count: affordable_count - best_first.len(),
            best_first,
        }
    }

    async fn poll(inputs: ModelRankingInputs) -> Result<Self, Error> {
        let management_key = read_secret(&inputs.management_key).ok_or(Error::Secret)?;
        let authorization = format!("Bearer {management_key}");
        let benchmark_headers = [("Authorization", authorization.as_str())];
        let (openrouter_listing, benchmark_listing) = try_join!(
            http_json_client::get_json::<ModelListing<OpenRouterModel>>(OPENROUTER_MODELS_URL, &[]),
            http_json_client::get_json::<ModelListing<ModelBenchmarkRow>>(
                OPENROUTER_BENCHMARKS_URL,
                &benchmark_headers,
            ),
        )?;
        Ok(Self::ranked(
            openrouter_listing.data,
            &benchmark_listing.data,
            inputs.max_blended_per_million,
        ))
    }
}

impl SnapshotSource for OpenRouterModelRankings {
    type Error = Error;
    type StreamInputs = OnlineSourceInputs<ModelRankingInputs>;

    const NAME: &'static str = "Best models within price cap";

    fn delivery_stream(
        OnlineSourceInputs {
            buses,
            source_inputs: inputs,
        }: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        internet_connectivity_gate::while_online(buses, move || {
            let inputs = inputs.clone();
            PeriodicPolling::every(Duration::from_hours(6))
                .poll_deliveries(move || Self::poll(inputs.clone()))
        })
    }
}

fn lists_tool_calling<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Vec::<String>::deserialize(deserializer).map(|parameters| {
        parameters
            .iter()
            .any(|parameter| parameter == TOOL_CALLING_PARAMETER)
    })
}

fn usd_per_million_tokens<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Money, D::Error> {
    let per_token_text = String::deserialize(deserializer)?;
    Decimal::from_str(&per_token_text)
        .map(|per_token| usd(per_token * Decimal::from(1_000_000)))
        .map_err(de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPENROUTER_FIXTURE: &str = r#"{"data":[
        {"id":"deepseek/deepseek-v4.1-flash","canonical_slug":"deepseek/deepseek-v4.1-flash-20260601",
         "context_length":1048576,
         "pricing":{"prompt":"0.0000002","completion":"0.0000008"},
         "supported_parameters":["tools","temperature"]},
        {"id":"z-ai/glm-5.2","canonical_slug":"z-ai/glm-5.2-20260701","context_length":200000,
         "pricing":{"prompt":"0.0000004","completion":"0.0000016"},
         "supported_parameters":["tools"]},
        {"id":"tiny/no-tools","canonical_slug":"tiny/no-tools-20260101","context_length":8192,
         "pricing":{"prompt":"0.00000001","completion":"0.00000002"},
         "supported_parameters":["temperature"]},
        {"id":"deepseek/deepseek-v4.1-flash:free","canonical_slug":"deepseek/deepseek-v4.1-flash-20260601",
         "context_length":1048576,
         "pricing":{"prompt":"0","completion":"0"},
         "supported_parameters":["tools"]},
        {"id":"openrouter/auto","canonical_slug":"openrouter/auto","context_length":2000000,
         "pricing":{"prompt":"-1","completion":"-1"},
         "supported_parameters":["tools"]},
        {"id":"openai/gpt-6.1-terra-pro","canonical_slug":"openai/gpt-6.1-terra-pro-20261026",
         "context_length":400000,
         "pricing":{"prompt":"0.00002","completion":"0.00008"},
         "supported_parameters":["tools"]},
        {"id":"~z-ai/glm-latest","canonical_slug":"z-ai/glm-5.2-20260701","context_length":200000,
         "pricing":{"prompt":"0.0000004","completion":"0.0000016"},
         "supported_parameters":["tools"]},
        {"id":"tiny/short-context","canonical_slug":"tiny/short-context-20260101","context_length":32768,
         "pricing":{"prompt":"0.0000001","completion":"0.0000001"},
         "supported_parameters":["tools"]},
        {"id":"deepseek/deepseek-v4-pro-0813","canonical_slug":"deepseek/deepseek-v4-pro-20260813",
         "context_length":1048576,
         "pricing":{"prompt":"0.00000066","completion":"0.00000198"},
         "supported_parameters":["tools"]},
        {"id":"deepseek/deepseek-v4.1-flash-1017","canonical_slug":"deepseek/deepseek-v4.1-flash-20261017",
         "context_length":1048576,
         "pricing":{"prompt":"0.00000015","completion":"0.0000006"},
         "supported_parameters":["tools"]},
        {"id":"obscure/low-scoring","canonical_slug":"obscure/low-scoring-20260101","context_length":262144,
         "pricing":{"prompt":"0.0000001","completion":"0.0000001"},
         "supported_parameters":["tools"]},
        {"id":"obscure/similar-slug","canonical_slug":"obscure/similar-slug-20260101","context_length":262144,
         "pricing":{"prompt":"0.0000001","completion":"0.0000001"},
         "supported_parameters":["tools"]},
        {"id":"obscure/no-canonical-slug","context_length":262144,
         "pricing":{"prompt":"0.0000001","completion":"0.0000001"},
         "supported_parameters":["tools"]},
        {"id":"obscure/null-index","canonical_slug":"obscure/null-index-20260101","context_length":262144,
         "pricing":{"prompt":"0.0000001","completion":"0.0000001"},
         "supported_parameters":["tools"]},
        {"id":"obscure/no-benchmark-row","canonical_slug":"obscure/no-benchmark-row-20260101","context_length":262144,
         "pricing":{"prompt":"0.0000001","completion":"0.0000001"},
         "supported_parameters":["tools"]}
    ]}"#;

    const BENCHMARK_FIXTURE: &str = r#"{"data":[
        {"source":"artificial-analysis","model_permaslug":"deepseek/deepseek-v4.1-flash-20260601","intelligence_index":48.0},
        {"source":"artificial-analysis","model_permaslug":"deepseek/deepseek-v4.1-flash-20260601","intelligence_index":61.5},
        {"source":"artificial-analysis","model_permaslug":"deepseek/deepseek-v4.1-flash-20261017","intelligence_index":33.8},
        {"source":"artificial-analysis","model_permaslug":"deepseek/deepseek-v4-pro-20260813","intelligence_index":14.0},
        {"source":"artificial-analysis","model_permaslug":"z-ai/glm-5.2-20260701","intelligence_index":64.2},
        {"source":"artificial-analysis","model_permaslug":"openai/gpt-6.1-terra-pro-20261026","intelligence_index":80.0},
        {"source":"artificial-analysis","model_permaslug":"obscure/low-scoring-20260101","intelligence_index":3.1},
        {"source":"artificial-analysis","model_permaslug":"obscure/similar-slug-20260102","intelligence_index":90.0},
        {"source":"artificial-analysis","model_permaslug":"obscure/null-index-20260101","intelligence_index":null}
    ],"meta":{"model_count":9,"version":"v1"}}"#;

    fn fixture_rankings(max_blended_per_million: Decimal) -> OpenRouterModelRankings {
        let openrouter_listing: ModelListing<OpenRouterModel> =
            serde_json::from_str(OPENROUTER_FIXTURE).unwrap();
        let benchmark_listing: ModelListing<ModelBenchmarkRow> =
            serde_json::from_str(BENCHMARK_FIXTURE).unwrap();
        OpenRouterModelRankings::ranked(
            openrouter_listing.data,
            &benchmark_listing.data,
            max_blended_per_million,
        )
    }

    fn ranked_names(rankings: &OpenRouterModelRankings) -> Vec<&str> {
        rankings
            .best_first
            .iter()
            .map(|scored| scored.model.as_str())
            .collect()
    }

    fn scored_model<'a>(rankings: &'a OpenRouterModelRankings, model: &str) -> &'a ScoredModel {
        rankings
            .best_first
            .iter()
            .find(|scored| scored.model == model)
            .unwrap()
    }

    fn assert_intelligence_index(scored: &ScoredModel, expected_index: f64) {
        assert!(scored.intelligence_index.total_cmp(&expected_index).is_eq());
    }

    #[test]
    fn models_rank_by_intelligence_index_within_the_agentic_filter() {
        let rankings = fixture_rankings(Decimal::from(2));
        assert_eq!(
            ranked_names(&rankings),
            [
                "z-ai/glm-5.2",
                "deepseek/deepseek-v4.1-flash",
                "deepseek/deepseek-v4.1-flash-1017",
                "deepseek/deepseek-v4-pro-0813",
                "obscure/low-scoring"
            ]
        );
    }

    #[test]
    fn models_without_a_usable_benchmark_count_as_unscored() {
        let rankings = fixture_rankings(Decimal::from(2));
        assert_eq!(rankings.unscored_count, 4);
        let ranked = ranked_names(&rankings);
        assert!(!ranked.contains(&"obscure/similar-slug"));
        assert!(!ranked.contains(&"obscure/no-canonical-slug"));
        assert!(!ranked.contains(&"obscure/null-index"));
        assert!(!ranked.contains(&"obscure/no-benchmark-row"));
    }

    #[test]
    fn highest_row_wins_when_a_slug_has_several() {
        let rankings = fixture_rankings(Decimal::from(2));
        let flash = scored_model(&rankings, "deepseek/deepseek-v4.1-flash");
        assert_intelligence_index(flash, 61.5);
        assert_eq!(flash.input_per_million, usd(Decimal::new(2, 1)));
        assert_eq!(flash.output_per_million, usd(Decimal::new(8, 1)));
        assert_eq!(flash.context_tokens, 1_048_576);
    }

    #[test]
    fn dated_canonical_slug_selects_its_own_snapshot() {
        let rankings = fixture_rankings(Decimal::from(2));
        let flash_snapshot = scored_model(&rankings, "deepseek/deepseek-v4.1-flash-1017");
        assert_intelligence_index(flash_snapshot, 33.8);
        let pro_snapshot = scored_model(&rankings, "deepseek/deepseek-v4-pro-0813");
        assert_intelligence_index(pro_snapshot, 14.0);
        assert_eq!(
            pro_snapshot.openrouter_page(),
            "https://openrouter.ai/deepseek/deepseek-v4-pro-0813"
        );
    }

    #[test]
    fn price_cap_excludes_dearer_models() {
        let rankings = fixture_rankings(Decimal::new(5, 1));
        assert_eq!(
            ranked_names(&rankings),
            [
                "deepseek/deepseek-v4.1-flash",
                "deepseek/deepseek-v4.1-flash-1017",
                "obscure/low-scoring"
            ]
        );
    }
}
