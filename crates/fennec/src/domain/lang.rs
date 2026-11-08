//! The language the human reads fennec in: the texts the server builds follow it.

use serde::{Deserialize, Serialize};

/// Stored as `display.language`: `en` (the default) or `fr`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Fr,
}

impl Lang {
    /// The text for this language.
    pub fn pick<'a>(self, en: &'a str, fr: &'a str) -> &'a str {
        match self {
            Lang::En => en,
            Lang::Fr => fr,
        }
    }

    /// How the figures are written for this language.
    pub fn locale(self) -> token_usage::display::Locale {
        match self {
            Lang::En => token_usage::display::Locale::En,
            Lang::Fr => token_usage::display::Locale::Fr,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Lang;

    #[test]
    fn stored_as_its_lowercase_code() {
        assert_eq!(serde_json::to_string(&Lang::Fr).unwrap(), "\"fr\"");
        assert_eq!(serde_json::from_str::<Lang>("\"en\"").unwrap(), Lang::En);
        assert!(serde_json::from_str::<Lang>("\"de\"").is_err());
    }

    #[test]
    fn pick_returns_the_chosen_language_text() {
        assert_eq!(Lang::En.pick("hello", "bonjour"), "hello");
        assert_eq!(Lang::Fr.pick("hello", "bonjour"), "bonjour");
    }

    #[test]
    fn locale_maps_to_the_display_locale() {
        assert!(matches!(
            Lang::En.locale(),
            token_usage::display::Locale::En
        ));
        assert!(matches!(
            Lang::Fr.locale(),
            token_usage::display::Locale::Fr
        ));
    }
}
