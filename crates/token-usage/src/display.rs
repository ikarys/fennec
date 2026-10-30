//! Text for showing figures to a person: French decimal comma, the units the
//! fennec UI already used. Kept here so every consumer (UI, Insight, CLI)
//! writes a figure the same way.

/// `950` stays `950`, `12345` becomes `12,3k`, `4702851` becomes `4,7M`.
pub fn tokens(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0).replace('.', ",")
    } else if value >= 1000 {
        format!("{:.1}k", value as f64 / 1000.0).replace('.', ",")
    } else {
        value.to_string()
    }
}

/// `42 tok/s`.
pub fn speed(tokens_per_second: f64) -> String {
    format!("{} tok/s", tokens_per_second.round() as u64)
}

/// `820 ms`, `4,2 s`, `3 min 5 s`.
pub fn duration(secs: f64) -> String {
    let ms = (secs * 1000.0).round() as u64;
    if ms < 1000 {
        return format!("{ms} ms");
    }
    if ms < 60_000 {
        return format!("{:.1} s", ms as f64 / 1000.0).replace('.', ",");
    }
    let seconds = (ms as f64 / 1000.0).round() as u64;
    format!("{} min {} s", seconds / 60, seconds % 60)
}

/// `148 $`, `3,20 $`, `<0,01 $`: dollars, French decimal comma.
pub fn usd(value: f64) -> String {
    if value <= 0.0 {
        return "0 $".to_string();
    }
    if value < 0.01 {
        return "<0,01 $".to_string();
    }
    if value < 100.0 {
        return format!("{:.2} $", value).replace('.', ",");
    }
    let rounded = value.round() as u64;
    let digits = rounded.to_string();
    let mut out = String::new();
    let len = digits.len();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (len - index).is_multiple_of(3) {
            out.push('\u{202F}');
        }
        out.push(digit);
    }
    format!("{out} $")
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
        assert_eq!(tokens(950), "950");
        assert_eq!(tokens(12_345), "12,3k");
        assert_eq!(tokens(4_702_851), "4,7M");
    }

    #[test]
    fn speed_is_rounded() {
        assert_eq!(speed(41.6), "42 tok/s");
    }

    #[test]
    fn durations_pick_the_readable_unit() {
        assert_eq!(duration(0.82), "820 ms");
        assert_eq!(duration(4.24), "4,2 s");
        assert_eq!(duration(185.0), "3 min 5 s");
    }

    #[test]
    fn percent_of_nothing_is_zero() {
        assert_eq!(percent(1, 0), 0);
        assert_eq!(percent(1, 3), 33);
    }

    #[test]
    fn usd_formats_dollars_with_french_comma() {
        assert_eq!(usd(0.0), "0 $");
        assert_eq!(usd(-1.0), "0 $");
        assert_eq!(usd(0.004), "<0,01 $");
        assert_eq!(usd(3.2), "3,20 $");
        assert_eq!(usd(99.999), "100,00 $");
        assert_eq!(usd(148.0), "148 $");
        assert_eq!(usd(1_482.0), "1\u{202F}482 $");
        assert_eq!(usd(1_000_000.0), "1\u{202F}000\u{202F}000 $");
    }
}
