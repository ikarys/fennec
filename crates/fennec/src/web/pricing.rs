//! Shared model pricing for the web views: turns a usage ledger's model rows
//! into a dollar estimate, split by token kind, and explains what is priced
//! and what is not.

use crate::domain::lang::Lang;
use crate::domain::preferences::Preferences;
use llm_providers_connector::{Cost, Price};
use std::collections::BTreeMap;
use token_usage::Row;
use token_usage::display::usd;

#[derive(Debug, PartialEq)]
pub struct Pricing {
    /// Summed over the priced models; split by token kind.
    pub cost: Cost,
    /// How many models had a price.
    pub priced: usize,
    /// Model keys without a price, sorted.
    pub unpriced: Vec<String>,
}

impl Pricing {
    /// The dollars, or "—" when no model had a price: zero would claim a free run.
    pub fn value(&self, locale: token_usage::display::Locale) -> String {
        if self.priced == 0 {
            "—".to_string()
        } else {
            usd(self.cost.total(), locale)
        }
    }

    /// How the cost was reached, or why there is none (models without a price).
    pub fn detail(&self, lang: Lang) -> String {
        if self.priced == 0 {
            lang.pick(
                "no model has a price: Settings, Models",
                "aucun modèle n'a de prix : Paramètres, Modèles",
            )
            .to_string()
        } else {
            let mut detail = lang
                .pick(
                    "indicative, at each model's price",
                    "indicatif, au prix de chaque modèle",
                )
                .to_string();
            if !self.unpriced.is_empty() {
                detail.push_str(lang.pick("; without a price: ", " ; sans prix : "));
                detail.push_str(&self.unpriced.join(", "));
            }
            detail
        }
    }

    /// One part of the estimate, or none when no model had a price.
    pub fn part(&self, dollars: f64, locale: token_usage::display::Locale) -> Option<String> {
        if self.priced == 0 {
            None
        } else {
            Some(usd(dollars, locale))
        }
    }
}

/// The estimate for a set of model rows, at the given prices.
pub fn priced(models: &[Row], prices: &BTreeMap<String, Price>) -> Pricing {
    let mut cost = Cost::default();
    let mut priced = 0;
    let mut unpriced = Vec::new();

    for row in models {
        match prices.get(&row.key) {
            Some(price) => {
                cost += price.costs(
                    row.summary.prompt_tokens,
                    row.summary.cached_tokens,
                    row.summary.completion_tokens,
                );
                priced += 1;
            }
            None => unpriced.push(row.key.clone()),
        }
    }

    unpriced.sort();
    Pricing {
        cost,
        priced,
        unpriced,
    }
}

/// The per-model prices from the settings; the first provider's price wins.
pub fn prices(preferences: &Preferences) -> BTreeMap<String, Price> {
    let mut prices = BTreeMap::new();
    for provider in &preferences.providers {
        for (model_id, model_prefs) in &provider.models {
            if let Some(price) = model_prefs.price {
                prices.entry(model_id.clone()).or_insert(price);
            }
        }
    }
    prices
}

#[cfg(test)]
mod tests {
    use super::*;
    use token_usage::{Filter, Ledger};

    fn row(key: &str, prompt_tokens: u64) -> Row {
        let mut summary = Ledger::open_in_memory()
            .unwrap()
            .summary(&Filter::default())
            .unwrap();
        summary.prompt_tokens = prompt_tokens;
        Row {
            key: key.to_string(),
            name: key.to_string(),
            summary,
        }
    }

    #[test]
    fn priced_sums_priced_models_and_lists_unpriced_models() {
        let models = vec![row("a", 1_000_000), row("b", 1_000_000)];
        let prices = BTreeMap::from([(
            "a".to_string(),
            Price {
                input_per_m: 3.0,
                cached_per_m: 0.0,
                output_per_m: 0.0,
            },
        )]);

        let pricing = priced(&models, &prices);

        assert_eq!(pricing.priced, 1);
        assert!((pricing.cost.total() - 3.0).abs() < 1e-9);
        assert_eq!(pricing.unpriced, vec!["b".to_string()]);
    }

    #[test]
    fn detail_explains_the_missing_price_and_the_unpriced_models() {
        let none = Pricing {
            cost: Cost::default(),
            priced: 0,
            unpriced: vec![],
        };
        assert_eq!(
            none.detail(Lang::Fr),
            "aucun modèle n'a de prix : Paramètres, Modèles"
        );
        assert_eq!(
            none.detail(Lang::En),
            "no model has a price: Settings, Models"
        );

        let some = Pricing {
            cost: Cost::default(),
            priced: 1,
            unpriced: vec!["b".to_string()],
        };
        assert_eq!(
            some.detail(Lang::Fr),
            "indicatif, au prix de chaque modèle ; sans prix : b"
        );
        assert_eq!(
            some.detail(Lang::En),
            "indicative, at each model's price; without a price: b"
        );
    }
}
