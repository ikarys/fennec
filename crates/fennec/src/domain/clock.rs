use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Today's date (UTC) as (year, month, day).
pub fn today() -> (i64, u32, u32) {
    civil_from_days((now_ms() / 86_400_000) as i64)
}

/// A UTC timestamp as "YYYY-MM-DD HH:MM".
pub fn format_utc(ms: u64) -> String {
    let days = (ms / 86_400_000) as i64;
    let seconds = (ms % 86_400_000) / 1000;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        seconds / 3600,
        (seconds % 3600) / 60
    )
}

/// Days since 1970-01-01 to (year, month, day), Howard Hinnant's algorithm;
/// avoids a date crate for one date in the prompt.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_date() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_751), (2026, 10, 25));
    }

    #[test]
    fn format_utc_formats_date_and_time() {
        assert_eq!(format_utc(0), "1970-01-01 00:00");
        let ms = 20_751 * 86_400_000 + 13 * 3_600_000 + 37 * 60_000;
        assert_eq!(format_utc(ms), "2026-10-25 13:37");
    }
}
