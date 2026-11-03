//! The language the human reads fennec in: the texts the server builds follow it.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Fr,
}

impl Lang {
    /// The stored `display.language`; anything but "fr" reads as English, the default.
    pub fn from_code(code: &str) -> Lang {
        if code == "fr" { Lang::Fr } else { Lang::En }
    }

    /// The text for this language.
    pub fn pick<'a>(self, en: &'a str, fr: &'a str) -> &'a str {
        match self {
            Lang::En => en,
            Lang::Fr => fr,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Lang;

    #[test]
    fn from_code_reads_fr_and_defaults_to_english() {
        assert_eq!(Lang::from_code("fr"), Lang::Fr);
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("de"), Lang::En);
    }

    #[test]
    fn pick_returns_the_chosen_language_text() {
        assert_eq!(Lang::En.pick("hello", "bonjour"), "hello");
        assert_eq!(Lang::Fr.pick("hello", "bonjour"), "bonjour");
    }
}
