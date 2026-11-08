//! Is an endpoint on this Mac or a private network? Used to label providers "local"
//! (docs/architecture.md §4.5). When unsure, say "not local".

use std::net::IpAddr;

use url::{Host, Url};

pub fn is_local_url(base_url: &str) -> bool {
    let Ok(url) = Url::parse(base_url) else {
        return false;
    };
    match url.host() {
        Some(Host::Ipv4(ip)) => is_private_ip(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => is_private_ip(IpAddr::V6(ip)),
        Some(Host::Domain(name)) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            name == "localhost"
                || name.ends_with(".localhost")
                || name.ends_with(".local")
                || name.ends_with(".ts.net") // Tailscale MagicDNS
                || !name.contains('.') // single-label LAN / tailnet host, e.g. "gpu-box"
        }
        None => false,
    }
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || (a == 100 && (64..128).contains(&b)) // CGNAT, used by Tailscale
        }
        IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

#[cfg(test)]
mod tests {
    use super::is_local_url;

    #[test]
    fn classifies_hosts() {
        for local in [
            "http://localhost:11434/v1",
            "http://127.0.0.1:1234/v1",
            "http://192.168.3.27:8000/v1",
            "http://100.90.208.26:8000/v1",
            "http://gpu-box:8000/v1",
            "http://sennett-pc.tail1234.ts.net/v1",
            "http://[::1]:8000/v1",
        ] {
            assert!(is_local_url(local), "{local}");
        }
        for remote in [
            "https://api.openai.com/v1",
            "https://openrouter.ai/api/v1",
            "not a url",
        ] {
            assert!(!is_local_url(remote), "{remote}");
        }
    }
}
