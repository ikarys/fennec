//! The SSRF guard: which addresses a fetched URL may point at, and the small
//! URL pieces the redirect loop needs (no `url` crate).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

/// Only publicly routable addresses pass.
pub(super) fn is_public(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => public_v4(v4),
        IpAddr::V6(v6) => public_v6(v6),
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || (a == 100 && (64..128).contains(&b)))
}

fn public_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    !(ip.is_loopback()
        || ip.is_unspecified()
        || first & 0xfe00 == 0xfc00 // fc00::/7 unique local
        || first & 0xffc0 == 0xfe80) // fe80::/10 link-local
}

/// Resolves the URL's host and refuses it when any address fails `allow`.
pub(super) fn check(url: &str, allow: fn(IpAddr) -> bool) -> Result<(), String> {
    let (host, port) = host_port(url)?;
    let addresses = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|error| format!("Cannot resolve {host}: {error}"))?;
    let mut any = false;
    for address in addresses {
        any = true;
        if !allow(address.ip()) {
            return Err(format!(
                "Refused: {host} resolves to a local or private address ({}).",
                address.ip()
            ));
        }
    }
    if any {
        Ok(())
    } else {
        Err(format!("Cannot resolve {host}"))
    }
}

fn host_port(url: &str) -> Result<(String, u16), String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| "URL must start with http:// or https://".to_string())?;
    let default = if scheme == "https" { 443 } else { 80 };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or("");
    let (host, port) = match authority.strip_prefix('[') {
        Some(inner) => {
            let (host, after) = inner.split_once(']').ok_or("Invalid URL host")?;
            (host, after.strip_prefix(':'))
        }
        None => match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        },
    };
    if host.is_empty() {
        return Err("Invalid URL: no host".into());
    }
    let port = match port {
        Some(text) if !text.is_empty() => text.parse().map_err(|_| "Invalid URL port")?,
        _ => default,
    };
    Ok((host.to_string(), port))
}

/// Resolves a redirect's `Location` against the current URL. Only absolute
/// URLs, `//host/..` and `/path` are followed; other forms are refused.
pub(super) fn join(current: &str, location: &str) -> Result<String, String> {
    let (scheme, rest) = current.split_once("://").ok_or("Invalid URL")?;
    if location.starts_with("http://") || location.starts_with("https://") {
        return Ok(location.to_string());
    }
    if location.starts_with("//") {
        return Ok(format!("{scheme}:{location}"));
    }
    if location.starts_with('/') {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        return Ok(format!("{scheme}://{authority}{location}"));
    }
    Err(format!("Unsupported redirect target: {location}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn local_and_private_addresses_are_refused() {
        for text in [
            "127.0.0.1",
            "::1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "fe80::1",
            "fc00::1",
            "fd12::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::",
        ] {
            assert!(!is_public(ip(text)), "{text} must be refused");
        }
    }

    #[test]
    fn public_addresses_pass() {
        for text in [
            "8.8.8.8",
            "1.1.1.1",
            "172.32.0.1",
            "100.128.0.1",
            "2606:4700::1111",
        ] {
            assert!(is_public(ip(text)), "{text} must pass");
        }
    }

    #[test]
    fn check_refuses_literal_and_named_local_hosts() {
        for url in [
            "http://127.0.0.1/",
            "http://[::1]:8080/x",
            "http://10.0.0.5/",
            "https://192.168.0.1/admin",
            "http://169.254.169.254/latest/meta-data",
            "http://localhost:7777/api",
            "http://user@127.0.0.1/",
        ] {
            assert!(check(url, is_public).is_err(), "{url} must be refused");
        }
    }

    #[test]
    fn host_port_reads_the_authority() {
        assert_eq!(host_port("https://a.b/c").unwrap(), ("a.b".into(), 443));
        assert_eq!(host_port("http://a.b:81?x").unwrap(), ("a.b".into(), 81));
        assert_eq!(host_port("http://[::1]/").unwrap(), ("::1".into(), 80));
    }

    #[test]
    fn join_follows_absolute_and_root_relative_only() {
        let current = "https://a.example/docs/page?x=1";
        assert_eq!(
            join(current, "https://b.example/z").unwrap(),
            "https://b.example/z"
        );
        assert_eq!(join(current, "/other").unwrap(), "https://a.example/other");
        assert_eq!(
            join(current, "//c.example/q").unwrap(),
            "https://c.example/q"
        );
        assert!(join(current, "next").is_err());
    }
}
