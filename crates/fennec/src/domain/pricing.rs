//! Shared model pricing: turns a usage ledger's model rows
//! into a dollar estimate, split by token kind, and explains what is priced
//! and what is not.

use crate::domain::lang::Lang;
use crate::domain::preferences::{LOCAL_PROVIDER, Preferences};
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

/// A local model's price is a simulation, a paid provider's is a bill.
pub fn billed_by_default(provider: &str) -> bool {
    provider != LOCAL_PROVIDER
}

/// Whether a model's spend counts toward the projects' budgets: the explicit
/// `billing` entry wins, else the provider's default.
pub fn billed(preferences: &Preferences, provider: &str, model: &str) -> bool {
    preferences
        .billing
        .get(&format!("{provider}/{model}"))
        .copied()
        .unwrap_or(billed_by_default(provider))
}

/// The per-model prices from the settings, keeping only the models `keep` allows;
/// the first provider's price wins.
fn filtered_prices(
    preferences: &Preferences,
    keep: impl Fn(&str, &str) -> bool,
) -> BTreeMap<String, Price> {
    let mut prices = BTreeMap::new();
    for provider in &preferences.providers {
        for (model_id, model_prefs) in &provider.models {
            if !keep(&provider.id, model_id) {
                continue;
            }
            if let Some(price) = model_prefs.price {
                prices.entry(model_id.clone()).or_insert(price);
            }
        }
    }
    prices
}

/// The per-model prices from the settings; the first provider's price wins.
pub fn prices(preferences: &Preferences) -> BTreeMap<String, Price> {
    filtered_prices(preferences, |_, _| true)
}

/// The per-model prices that count toward the projects' budgets; the first
/// billed provider's price wins.
pub fn billed_prices(preferences: &Preferences) -> BTreeMap<String, Price> {
    filtered_prices(preferences, |provider, model| {
        billed(preferences, provider, model)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::preferences::{ModelPrefs, Provider};
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

    const PRICE: Price = Price {
        input_per_m: 1.0,
        cached_per_m: 0.0,
        output_per_m: 0.0,
    };

    fn prefs_with_model(
        provider: &str,
        model: &str,
        price: Price,
        billing: BTreeMap<String, bool>,
    ) -> Preferences {
        let mut models = BTreeMap::new();
        models.insert(
            model.to_string(),
            ModelPrefs {
                price: Some(price),
                ..Default::default()
            },
        );
        let providers = vec![Provider {
            id: provider.to_string(),
            name: provider.to_string(),
            base_url: String::new(),
            api_key: None,
            models,
        }];
        Preferences {
            providers,
            billing,
            ..Default::default()
        }
    }

    #[test]
    fn billed_prices_omits_an_unbilled_local_model_but_prices_keeps_it() {
        let prefs = prefs_with_model("local", "m", PRICE, BTreeMap::new());

        assert!(!billed_prices(&prefs).contains_key("m"));
        assert!(prices(&prefs).contains_key("m"));
    }

    #[test]
    fn an_explicit_billing_entry_makes_a_local_model_billed() {
        let billing = BTreeMap::from([("local/m".to_string(), true)]);
        let prefs = prefs_with_model("local", "m", PRICE, billing);

        assert!(billed_prices(&prefs).contains_key("m"));
    }

    #[test]
    fn an_explicit_billing_entry_makes_a_paid_model_unbilled() {
        let billing = BTreeMap::from([("paid/m".to_string(), false)]);
        let prefs = prefs_with_model("paid", "m", PRICE, billing);

        assert!(!billed_prices(&prefs).contains_key("m"));
        assert!(prices(&prefs).contains_key("m"));
    }
}
