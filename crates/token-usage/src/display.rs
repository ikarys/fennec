//! Text for showing figures to a person: locale-aware decimal and currency
//! formatting, with the units every consumer already uses. Kept here so every
//! consumer (UI, Insight, CLI) writes a figure the same way.

/// How numbers are written: French uses a decimal comma, a narrow no-break space between thousands and a
/// trailing ` $`; English a decimal point, a comma between thousands and a leading `$`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    Fr,
}

/// `950` stays `950`; `12345` becomes `12,3k` in French or `12.3k` in English;
/// `4702851` becomes `4,7M` or `4.7M`.
pub fn tokens(value: u64, locale: Locale) -> String {
    let text = if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1000 {
        format!("{:.1}k", value as f64 / 1000.0)
    } else {
        return value.to_string();
    };
    match locale {
        Locale::Fr => text.replace('.', ","),
        Locale::En => text,
    }
}

/// `42 tok/s`; no decimals, so no locale.
pub fn speed(tokens_per_second: f64) -> String {
    format!("{} tok/s", tokens_per_second.round() as u64)
}

/// `820 ms`, `4,2 s`/`4.2 s`, `3 min 5 s`, `6 h 13 min`.
pub fn duration(secs: f64, locale: Locale) -> String {
    let ms = (secs * 1000.0).round() as u64;
    if ms < 1000 {
        return format!("{ms} ms");
    }
    if ms < 60_000 {
        let text = format!("{:.1} s", ms as f64 / 1000.0);
        return match locale {
            Locale::Fr => text.replace('.', ","),
            Locale::En => text,
        };
    }
    let seconds = (ms as f64 / 1000.0).round() as u64;
    if seconds < 3600 {
        return format!("{} min {} s", seconds / 60, seconds % 60);
    }
    format!("{} h {} min", seconds / 3600, (seconds % 3600) / 60)
}

/// `148 $`/`$148`, `3,20 $`/`$3.20`, `<0,01 $`/`<$0.01`: dollars, locale decimal and grouping.
pub fn usd(value: f64, locale: Locale) -> String {
    if value <= 0.0 {
        return match locale {
            Locale::Fr => "0 $".to_string(),
            Locale::En => "$0".to_string(),
        };
    }
    if value < 0.01 {
        return match locale {
            Locale::Fr => "<0,01 $".to_string(),
            Locale::En => "<$0.01".to_string(),
        };
    }
    if value < 100.0 {
        return match locale {
            Locale::Fr => format!("{:.2} $", value).replace('.', ","),
            Locale::En => format!("${:.2}", value),
        };
    }
    let rounded = value.round() as u64;
    let digits = rounded.to_string();
    let mut out = String::new();
    let len = digits.len();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (len - index).is_multiple_of(3) {
            out.push(match locale {
                Locale::Fr => '\u{202F}',
                Locale::En => ',',
            });
        }
        out.push(digit);
    }
    match locale {
        Locale::Fr => format!("{out} $"),
        Locale::En => format!("${out}"),
    }
}

/// Share of `part` in `whole`, `0` when `whole` is empty.
pub fn percent(part: u64, whole: u64) -> u64 {
    if whole == 0 {
        0
    } else {
        (part as f64 / whole as f64 * 100.0).round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_switch_to_k_at_a_thousand() {
        assert_eq!(tokens(950, Locale::Fr), "950");
        assert_eq!(tokens(950, Locale::En), "950");
        assert_eq!(tokens(12_345, Locale::Fr), "12,3k");
        assert_eq!(tokens(12_345, Locale::En), "12.3k");
        assert_eq!(tokens(4_702_851, Locale::Fr), "4,7M");
        assert_eq!(tokens(4_702_851, Locale::En), "4.7M");
    }

    #[test]
    fn speed_is_rounded() {
        assert_eq!(speed(41.6), "42 tok/s");
    }

    #[test]
    fn durations_pick_the_readable_unit() {
        assert_eq!(duration(0.82, Locale::Fr), "820 ms");
        assert_eq!(duration(0.82, Locale::En), "820 ms");
        assert_eq!(duration(4.24, Locale::Fr), "4,2 s");
        assert_eq!(duration(4.24, Locale::En), "4.2 s");
        assert_eq!(duration(185.0, Locale::Fr), "3 min 5 s");
        assert_eq!(duration(185.0, Locale::En), "3 min 5 s");
        assert_eq!(duration(3600.0, Locale::Fr), "1 h 0 min");
        assert_eq!(duration(3600.0, Locale::En), "1 h 0 min");
        assert_eq!(duration(22_384.0, Locale::Fr), "6 h 13 min");
        assert_eq!(duration(22_384.0, Locale::En), "6 h 13 min");
    }

    #[test]
    fn percent_of_nothing_is_zero() {
        assert_eq!(percent(1, 0), 0);
        assert_eq!(percent(1, 3), 33);
    }

    #[test]
    fn usd_formats_dollars_by_locale() {
        assert_eq!(usd(0.0, Locale::Fr), "0 $");
        assert_eq!(usd(0.0, Locale::En), "$0");
        assert_eq!(usd(-1.0, Locale::Fr), "0 $");
        assert_eq!(usd(-1.0, Locale::En), "$0");
        assert_eq!(usd(0.004, Locale::Fr), "<0,01 $");
        assert_eq!(usd(0.004, Locale::En), "<$0.01");
        assert_eq!(usd(3.2, Locale::Fr), "3,20 $");
        assert_eq!(usd(3.2, Locale::En), "$3.20");
        assert_eq!(usd(99.999, Locale::Fr), "100,00 $");
        assert_eq!(usd(99.999, Locale::En), "$100.00");
        assert_eq!(usd(148.0, Locale::Fr), "148 $");
        assert_eq!(usd(148.0, Locale::En), "$148");
        assert_eq!(usd(1_482.0, Locale::Fr), "1\u{202F}482 $");
        assert_eq!(usd(1_482.0, Locale::En), "$1,482");
        assert_eq!(usd(1_000_000.0, Locale::Fr), "1\u{202F}000\u{202F}000 $");
        assert_eq!(usd(1_000_000.0, Locale::En), "$1,000,000");
    }
}
