//! Pure release-version parsing and comparison for update checks.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.strip_prefix('v').unwrap_or(s);
        let core = s.split(['-', '+']).next()?;
        let mut parts = core.split('.');

        let major = parse_number(parts.next()?)?;
        let minor = parse_number(parts.next()?)?;
        let patch = parse_number(parts.next()?)?;

        if parts.next().is_some() {
            return None;
        }

        Some(Version {
            major,
            minor,
            patch,
        })
    }
}

fn parse_number(part: &str) -> Option<u64> {
    if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

pub fn tag_from_latest_url(url: &str) -> Option<&str> {
    let path = url.split(['?', '#']).next()?;
    let start = path.rfind("/tag/")?;
    let tag = &path[start + "/tag/".len()..];
    let tag = tag.trim_end_matches('/');

    if tag.is_empty() { None } else { Some(tag) }
}

pub fn is_newer(current: &str, latest_tag: &str) -> bool {
    matches!(
        (Version::parse(current), Version::parse(latest_tag)),
        (Some(current), Some(latest)) if latest > current
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_plain_and_prefixed_tags() {
        let expected = Version {
            major: 0,
            minor: 4,
            patch: 0,
        };

        assert_eq!(Version::parse("0.4.0"), Some(expected));
        assert_eq!(Version::parse("v0.4.0"), Some(expected));
        assert_eq!(Version::parse("0.4.0-edge"), Some(expected));
        assert_eq!(Version::parse("v0.4.0+build"), Some(expected));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!(Version::parse(""), None);
        assert_eq!(Version::parse("v1.2"), None);
        assert_eq!(Version::parse("a.b.c"), None);
    }

    #[test]
    fn ordering_uses_numeric_fields() {
        assert!(Version::parse("0.10.0").unwrap() > Version::parse("0.9.9").unwrap());
    }

    #[test]
    fn tag_from_latest_url_extracts_tag() {
        assert_eq!(
            tag_from_latest_url("https://github.com/ikarys/fennec/releases/tag/v0.5.0"),
            Some("v0.5.0")
        );
        assert_eq!(
            tag_from_latest_url("https://github.com/ikarys/fennec/releases/latest"),
            None
        );
        assert_eq!(
            tag_from_latest_url("https://github.com/ikarys/fennec/releases/tag/"),
            None
        );
    }

    #[test]
    fn is_newer_handles_compare_and_garbage() {
        assert!(is_newer("0.4.0", "v0.5.0"));
        assert!(!is_newer("0.5.0", "v0.4.0"));
        assert!(!is_newer("0.4.0", "v0.4.0"));
        assert!(!is_newer("garbage", "v0.4.0"));
    }
}
