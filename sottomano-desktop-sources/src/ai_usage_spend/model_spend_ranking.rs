use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::ai_usage_spend::spend_money::{Money, usd};

#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpend {
    pub model: String,
    pub cost: Money,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModelRanking {
    pub model_spends: Vec<ModelSpend>,
}

impl ModelRanking {
    #[must_use]
    pub fn total(&self) -> Money {
        usd(self
            .model_spends
            .iter()
            .map(|spend| *spend.cost.amount())
            .sum())
    }

    #[must_use]
    pub fn cost_of(&self, model: &str) -> Option<Money> {
        self.model_spends
            .iter()
            .find(|spend| spend.model == model)
            .map(|spend| spend.cost)
    }
}

impl FromIterator<ModelSpend> for ModelRanking {
    fn from_iter<I: IntoIterator<Item = ModelSpend>>(spends: I) -> Self {
        let mut merged = spends
            .into_iter()
            .fold(Vec::<ModelSpend>::new(), |mut merged, spend| {
                match merged
                    .iter_mut()
                    .find(|existing| existing.model == spend.model)
                {
                    Some(existing) => {
                        existing.cost = usd(existing.cost.amount() + spend.cost.amount());
                    }
                    None => merged.push(spend),
                }
                merged
            });
        merged.sort_by(|a, b| b.cost.amount().cmp(a.cost.amount()));
        Self {
            model_spends: merged,
        }
    }
}

impl Display for ModelRanking {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let names: Vec<&str> = self
            .model_spends
            .iter()
            .map(|spend| spend.model.as_str())
            .collect();
        write!(f, "{}", names.join(", "))
    }
}
