use std::{cmp::Ordering, path::PathBuf, str::FromStr, time::Duration};

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
const ANALYSIS_MODELS_URL: &str = "https://artificialanalysis.ai/api/v2/data/llms/models";
const OPENROUTER_SITE_URL: &str = "https://openrouter.ai";
const TOOL_CALLING_PARAMETER: &str = "tools";
const AGENTIC_MINIMUM_CONTEXT_TOKENS: u64 = 131_072;
const DATE_SUFFIX_LENGTHS: [usize; 3] = [4, 6, 8];
const REASONING_EFFORT_VARIANTS: [&str; 10] = [
    "reasoning",
    "non-reasoning",
    "thinking",
    "minimal",
    "low",
    "medium",
    "high",
    "xhigh",
    "max",
    "adaptive",
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelRankingInputs {
    pub analysis_key: PathBuf,
    pub max_blended_per_million: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoredModel {
    pub model: String,
    pub benchmarks: CodingBenchmarks,
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
struct AnalysisModel {
    #[serde(deserialize_with = "comparable_model_name")]
    slug: String,
    #[serde(rename = "evaluations")]
    benchmarks: CodingBenchmarks,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct CodingBenchmarks {
    #[serde(rename = "terminalbench_v4_0")]
    pub terminal_bench_v4: Option<f64>,
    #[serde(rename = "terminalbench_v2_1")]
    pub terminal_bench_v2: Option<f64>,
    pub scicode: Option<f64>,
}

impl CodingBenchmarks {
    fn best_of(self, other: Self) -> Self {
        Self {
            terminal_bench_v4: higher_score(self.terminal_bench_v4, other.terminal_bench_v4),
            terminal_bench_v2: higher_score(self.terminal_bench_v2, other.terminal_bench_v2),
            scicode: higher_score(self.scicode, other.scicode),
        }
    }

    fn any_scored(&self) -> bool {
        self.terminal_bench_v4.is_some()
            || self.terminal_bench_v2.is_some()
            || self.scicode.is_some()
    }

    fn ranking_order(&self, other: &Self) -> Ordering {
        [
            (self.terminal_bench_v4, other.terminal_bench_v4),
            (self.terminal_bench_v2, other.terminal_bench_v2),
            (self.scicode, other.scicode),
        ]
        .into_iter()
        .map(|(own_score, other_score)| {
            own_score
                .unwrap_or(f64::NEG_INFINITY)
                .total_cmp(&other_score.unwrap_or(f64::NEG_INFINITY))
        })
        .find(|ordering| ordering.is_ne())
        .unwrap_or(Ordering::Equal)
    }
}

impl TokenPricing {
    fn paid(&self) -> bool {
        self.input_per_million.is_positive() && self.output_per_million.is_positive()
    }

    fn blended_per_million(&self) -> Decimal {
        (self.input_per_million.amount() * Decimal::from(3) + self.output_per_million.amount())
            / Decimal::from(4)
    }
}

impl OpenRouterModel {
    fn fits_agentic_coding(&self) -> bool {
        let canonical = !self.id.contains(':') && !self.id.starts_with('~');
        self.tool_calling
            && self.pricing.paid()
            && canonical
            && self.context_length >= AGENTIC_MINIMUM_CONTEXT_TOKENS
    }

    fn scored(self, analysis_models: &[AnalysisModel]) -> Option<ScoredModel> {
        let model_without_provider = self
            .id
            .split_once('/')
            .map_or(self.id.as_str(), |(_, model)| model);
        let comparable_model = normalized_model_name(model_without_provider);
        let undated_model = without_trailing_date(&comparable_model);
        let benchmarks = [comparable_model.as_str(), undated_model]
            .into_iter()
            .map(|candidate_name| benchmarks_named(candidate_name, analysis_models))
            .find(CodingBenchmarks::any_scored)?;
        Some(ScoredModel {
            benchmarks,
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
        analysis_models: &[AnalysisModel],
        max_blended_per_million: Decimal,
    ) -> Self {
        let mut affordable: Vec<OpenRouterModel> = openrouter_models
            .into_iter()
            .filter(OpenRouterModel::fits_agentic_coding)
            .filter(|model| model.pricing.blended_per_million() <= max_blended_per_million)
            .collect();
        affordable.sort_by(|left, right| left.id.cmp(&right.id));
        affordable.dedup_by(|left, right| left.id == right.id);
        let affordable_count = affordable.len();
        let mut best_first: Vec<ScoredModel> = affordable
            .into_iter()
            .filter_map(|model| model.scored(analysis_models))
            .collect();
        best_first.sort_by(|left, right| right.benchmarks.ranking_order(&left.benchmarks));
        Self {
            max_blended_per_million: usd(max_blended_per_million),
            unscored_count: affordable_count - best_first.len(),
            best_first,
        }
    }

    async fn poll(inputs: ModelRankingInputs) -> Result<Self, Error> {
        let analysis_key = read_secret(&inputs.analysis_key).ok_or(Error::Secret)?;
        let analysis_headers = [("x-api-key", analysis_key.as_str())];
        let (openrouter_listing, analysis_listing) = try_join!(
            http_json_client::get_json::<ModelListing<OpenRouterModel>>(OPENROUTER_MODELS_URL, &[]),
            http_json_client::get_json::<ModelListing<AnalysisModel>>(
                ANALYSIS_MODELS_URL,
                &analysis_headers,
            ),
        )?;
        Ok(Self::ranked(
            openrouter_listing.data,
            &analysis_listing.data,
            inputs.max_blended_per_million,
        ))
    }
}

impl SnapshotSource for OpenRouterModelRankings {
    type Error = Error;
    type StreamInputs = OnlineSourceInputs<ModelRankingInputs>;

    const NAME: &'static str = "OpenRouter model rankings";

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

fn benchmarks_named(candidate_name: &str, analysis_models: &[AnalysisModel]) -> CodingBenchmarks {
    analysis_models
        .iter()
        .filter(|analysis| {
            analysis.slug == candidate_name
                || analysis
                    .slug
                    .strip_prefix(candidate_name)
                    .and_then(|variant| variant.strip_prefix('-'))
                    .is_some_and(|effort| REASONING_EFFORT_VARIANTS.contains(&effort))
        })
        .map(|analysis| analysis.benchmarks)
        .fold(CodingBenchmarks::default(), CodingBenchmarks::best_of)
}

fn without_trailing_date(model_name: &str) -> &str {
    match model_name.rsplit_once('-') {
        Some((undated, date_digits))
            if DATE_SUFFIX_LENGTHS.contains(&date_digits.len())
                && date_digits.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            undated
        }
        _ => model_name,
    }
}

fn higher_score(own_score: Option<f64>, other_score: Option<f64>) -> Option<f64> {
    own_score.into_iter().chain(other_score).reduce(f64::max)
}

fn normalized_model_name(name: &str) -> String {
    name.to_lowercase().replace(['.', '_'], "-")
}

fn comparable_model_name<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    String::deserialize(deserializer).map(|name| normalized_model_name(&name))
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
        {"id":"deepseek/deepseek-v4.1-flash","context_length":1048576,
         "pricing":{"prompt":"0.0000002","completion":"0.0000008"},
         "supported_parameters":["tools","temperature"]},
        {"id":"z-ai/glm-5.2","context_length":200000,
         "pricing":{"prompt":"0.0000004","completion":"0.0000016"},
         "supported_parameters":["tools"]},
        {"id":"tiny/no-tools","context_length":8192,
         "pricing":{"prompt":"0.00000001","completion":"0.00000002"},
         "supported_parameters":["temperature"]},
        {"id":"deepseek/deepseek-v4.1-flash:free","context_length":1048576,
         "pricing":{"prompt":"0","completion":"0"},
         "supported_parameters":["tools"]},
        {"id":"openrouter/auto","context_length":2000000,
         "pricing":{"prompt":"-1","completion":"-1"},
         "supported_parameters":["tools"]},
        {"id":"openai/gpt-6.1-terra-pro","context_length":400000,
         "pricing":{"prompt":"0.00002","completion":"0.00008"},
         "supported_parameters":["tools"]},
        {"id":"~z-ai/glm-latest","context_length":200000,
         "pricing":{"prompt":"0.0000004","completion":"0.0000016"},
         "supported_parameters":["tools"]},
        {"id":"tiny/short-context","context_length":32768,
         "pricing":{"prompt":"0.0000001","completion":"0.0000001"},
         "supported_parameters":["tools"]},
        {"id":"deepseek/deepseek-v4-pro-0813","context_length":1048576,
         "pricing":{"prompt":"0.00000066","completion":"0.00000198"},
         "supported_parameters":["tools"]},
        {"id":"deepseek/deepseek-v4.1-flash-1017","context_length":1048576,
         "pricing":{"prompt":"0.00000015","completion":"0.0000006"},
         "supported_parameters":["tools"]},
        {"id":"obscure/never-evaluated","context_length":262144,
         "pricing":{"prompt":"0.0000001","completion":"0.0000001"},
         "supported_parameters":["tools"]},
        {"id":"obscure/unbenchmarked","context_length":262144,
         "pricing":{"prompt":"0.0000001","completion":"0.0000001"},
         "supported_parameters":["tools"]}
    ]}"#;

    const ANALYSIS_FIXTURE: &str = r#"{"status":200,"data":[
        {"slug":"deepseek-v4-1-flash-reasoning","evaluations":{"terminalbench_v4_0":0.615}},
        {"slug":"deepseek-v4-1-flash-non-reasoning","evaluations":{"terminalbench_v4_0":0.48,"terminalbench_v2_1":0.81}},
        {"slug":"deepseek-v4-pro","evaluations":{"terminalbench_v4_0":0.14}},
        {"slug":"deepseek-v4-pro-0424","evaluations":{"terminalbench_v4_0":0.5}},
        {"slug":"deepseek-v4-1-flash-1017","evaluations":{"terminalbench_v4_0":0.338}},
        {"slug":"glm-5-2","evaluations":{"terminalbench_v4_0":0.642}},
        {"slug":"glm-5-2-1-pro","evaluations":{"terminalbench_v4_0":0.9}},
        {"slug":"gpt-6-1-terra-pro","evaluations":{"terminalbench_v4_0":0.8}},
        {"slug":"unbenchmarked","evaluations":{"terminalbench_v4_0":null,"scicode":0.31}}
    ]}"#;

    fn fixture_rankings(max_blended_per_million: Decimal) -> OpenRouterModelRankings {
        let openrouter_listing: ModelListing<OpenRouterModel> =
            serde_json::from_str(OPENROUTER_FIXTURE).unwrap();
        let analysis_listing: ModelListing<AnalysisModel> =
            serde_json::from_str(ANALYSIS_FIXTURE).unwrap();
        OpenRouterModelRankings::ranked(
            openrouter_listing.data,
            &analysis_listing.data,
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

    #[test]
    fn models_without_terminal_bench_v4_rank_after_scored_ones() {
        let rankings = fixture_rankings(Decimal::from(2));
        assert_eq!(
            ranked_names(&rankings),
            [
                "z-ai/glm-5.2",
                "deepseek/deepseek-v4.1-flash",
                "deepseek/deepseek-v4.1-flash-1017",
                "deepseek/deepseek-v4-pro-0813",
                "obscure/unbenchmarked"
            ]
        );
        assert_eq!(rankings.unscored_count, 1);
    }

    #[test]
    fn reasoning_variant_scores_its_base_model() {
        let rankings = fixture_rankings(Decimal::from(2));
        let flash = scored_model(&rankings, "deepseek/deepseek-v4.1-flash");
        assert_eq!(flash.benchmarks.terminal_bench_v4, Some(0.615));
        assert_eq!(flash.benchmarks.terminal_bench_v2, Some(0.81));
        assert_eq!(flash.input_per_million, usd(Decimal::new(2, 1)));
        assert_eq!(flash.output_per_million, usd(Decimal::new(8, 1)));
        assert_eq!(flash.context_tokens, 1_048_576);
        let glm = scored_model(&rankings, "z-ai/glm-5.2");
        assert_eq!(glm.benchmarks.terminal_bench_v4, Some(0.642));
    }

    #[test]
    fn dated_snapshot_prefers_its_own_entry_then_the_undated_one() {
        let rankings = fixture_rankings(Decimal::from(2));
        let flash_snapshot = scored_model(&rankings, "deepseek/deepseek-v4.1-flash-1017");
        assert_eq!(flash_snapshot.benchmarks.terminal_bench_v4, Some(0.338));
        let pro_snapshot = scored_model(&rankings, "deepseek/deepseek-v4-pro-0813");
        assert_eq!(pro_snapshot.benchmarks.terminal_bench_v4, Some(0.14));
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
                "obscure/unbenchmarked"
            ]
        );
    }
}
