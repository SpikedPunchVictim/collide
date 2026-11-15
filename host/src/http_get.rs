//! One HTTPS GET, made by `/usr/bin/curl` (ts-0068-wx; the owner's decision: no HTTP crate in the host).
//!
//! Why curl: it ships with macOS, honours the proxy it is told about, and keeps TLS, the system trust store and the
//! redirect and header rules out of this crate's dependency list. The cost is a process per fetch (one a day), and the
//! proxy lookup being ours (below). The alternative is a Rust HTTP crate (`ureq` with `native-tls`), which adds about
//! forty crates to a daemon that has `rusqlite` and `libc` and nothing else, and reads no proxy setting by itself.
//!
//! What the request is: `curl -q` (so `~/.curlrc` is never read), the one URL, no redirect followed, no cookies, one fixed
//! `User-Agent`, `Accept: application/json`, a 5 s limit on connecting and on the whole transfer, and a cap on the size
//! of the answer. It is run with an empty environment: nothing of the user's, no `HOME`, reaches it. Its protocol is
//! `https` only, except that a loopback address may use `http`, which is how the tests serve a recorded response.
//!
//! The proxy is found at each fetch, in this order: `HTTPS_PROXY` / `https_proxy`, then `ALL_PROXY` / `all_proxy`, in the
//! environment the daemon captured from the owner's login shell; then the system's, from `scutil --proxy`
//! (`HTTPSEnable`, `HTTPSProxy`, `HTTPSPort`, then the SOCKS entries). `NO_PROXY` / `no_proxy` is passed on. The system's
//! exceptions list is not read: a host listed there is still fetched through the proxy.

use crate::bounded_run::{run_bounded, tail};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// A GET of one URL. The body is returned when the answer is a success (a status below 400).
pub trait HttpGet: Send + Sync {
    fn get(&self, url: &str, timeout: Duration) -> Result<Vec<u8>, String>;
}

/// The answer is a few hundred bytes; a registry that sends more than this is not the one we asked.
pub const MAX_BODY_BYTES: u64 = 1024 * 1024;

pub const CURL: &str = "/usr/bin/curl";
pub const SCUTIL: &str = "/usr/sbin/scutil";
const USER_AGENT: &str = "collide";

/// Where the proxy comes from: the environment the daemon captured, and the system's settings.
pub trait ProxySource: Send + Sync {
    /// The proxy to use for an `https` URL, as a curl `--proxy` value.
    fn proxy(&self) -> Option<String>;
    /// The `NO_PROXY` list, if the environment has one.
    fn no_proxy(&self) -> Option<String>;
}

/// The environment's proxy variables, then the system's settings.
pub struct SystemProxy {
    env: BTreeMap<String, String>,
    scutil: PathBuf,
}

impl SystemProxy {
    /// `env` is the environment the daemon captured from the owner's login shell.
    pub fn new(env: BTreeMap<String, String>) -> SystemProxy {
        SystemProxy {
            env,
            scutil: PathBuf::from(SCUTIL),
        }
    }
}

impl ProxySource for SystemProxy {
    fn proxy(&self) -> Option<String> {
        proxy_from_env(&self.env).or_else(|| {
            let mut command = Command::new(&self.scutil);
            command.arg("--proxy").env_clear();
            let run = run_bounded(command, None, Duration::from_secs(5)).ok()?;
            (run.code == Some(0))
                .then(|| proxy_from_scutil(&String::from_utf8_lossy(&run.stdout)))
                .flatten()
        })
    }

    fn no_proxy(&self) -> Option<String> {
        first_set(&self.env, &["NO_PROXY", "no_proxy"])
    }
}

/// No proxy at all: a direct connection. The tests that serve a response on loopback use it.
pub struct NoProxy;

impl ProxySource for NoProxy {
    fn proxy(&self) -> Option<String> {
        None
    }
    fn no_proxy(&self) -> Option<String> {
        None
    }
}

/// A proxy a test names.
pub struct FixedProxy(pub String);

impl ProxySource for FixedProxy {
    fn proxy(&self) -> Option<String> {
        Some(self.0.clone())
    }
    fn no_proxy(&self) -> Option<String> {
        None
    }
}

fn first_set(env: &BTreeMap<String, String>, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| env.get(*name))
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

/// `HTTPS_PROXY`, then `ALL_PROXY`, in either case.
pub fn proxy_from_env(env: &BTreeMap<String, String>) -> Option<String> {
    first_set(env, &["HTTPS_PROXY", "https_proxy"])
        .or_else(|| first_set(env, &["ALL_PROXY", "all_proxy"]))
}

/// The proxy `scutil --proxy` describes: the HTTPS one when it is enabled, else the SOCKS one. The text is a dictionary
/// (`  HTTPSEnable : 1`).
pub fn proxy_from_scutil(text: &str) -> Option<String> {
    let entries: BTreeMap<&str, &str> = text
        .lines()
        .filter_map(|line| line.split_once(" : "))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect();
    let find = |enable: &str, host: &str, port: &str, scheme: &str| {
        if entries.get(enable) != Some(&"1") {
            return None;
        }
        let host = entries.get(host).filter(|host| !host.is_empty())?;
        let port = entries
            .get(port)
            .and_then(|port| port.parse::<u16>().ok())?;
        Some(format!("{scheme}://{host}:{port}"))
    };
    find("HTTPSEnable", "HTTPSProxy", "HTTPSPort", "http")
        .or_else(|| find("SOCKSEnable", "SOCKSProxy", "SOCKSPort", "socks5h"))
}

/// Whether `url` is a loopback address, which never leaves the machine.
fn is_loopback_http(url: &str) -> bool {
    ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"]
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

/// The arguments of the curl run. `-q` comes first, so no `.curlrc` is read.
pub fn curl_args(
    url: &str,
    timeout: Duration,
    proxy: Option<&str>,
    no_proxy: Option<&str>,
) -> Result<Vec<String>, String> {
    let protocol = if url.starts_with("https://") {
        "=https"
    } else if is_loopback_http(url) {
        "=http"
    } else {
        return Err(format!("only https addresses are fetched, not {url}"));
    };
    let seconds = timeout.as_secs_f32().max(0.1).to_string();
    let mut args: Vec<String> = [
        "-q",
        "--silent",
        "--show-error",
        "--fail",
        "--globoff",
        "--proto",
        protocol,
        "--max-time",
        &seconds,
        "--connect-timeout",
        &seconds,
        "--max-filesize",
        &MAX_BODY_BYTES.to_string(),
        "--user-agent",
        USER_AGENT,
        "--header",
        "Accept: application/json",
    ]
    .map(String::from)
    .to_vec();
    if let Some(proxy) = proxy {
        args.extend(["--proxy".into(), proxy.into()]);
    }
    if let Some(no_proxy) = no_proxy {
        args.extend(["--noproxy".into(), no_proxy.into()]);
    }
    args.extend(["--url".into(), url.into()]);
    Ok(args)
}

/// Fetches with curl.
pub struct CurlGet {
    curl: PathBuf,
    proxy: Box<dyn ProxySource>,
}

impl CurlGet {
    pub fn new(curl: impl Into<PathBuf>, proxy: Box<dyn ProxySource>) -> CurlGet {
        CurlGet {
            curl: curl.into(),
            proxy,
        }
    }

    /// `/usr/bin/curl`, through the proxy the owner's environment and the system name.
    pub fn system(captured_env: BTreeMap<String, String>) -> CurlGet {
        CurlGet::new(CURL, Box::new(SystemProxy::new(captured_env)))
    }
}

impl HttpGet for CurlGet {
    fn get(&self, url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
        let proxy = if url.starts_with("https://") || is_loopback_http(url) {
            self.proxy.proxy()
        } else {
            None
        };
        let args = curl_args(
            url,
            timeout,
            proxy.as_deref(),
            self.proxy.no_proxy().as_deref(),
        )?;
        let mut command = Command::new(&self.curl);
        command.args(args).env_clear();
        // curl's own limit is `timeout`; this one only catches a curl that does not keep to it.
        let run = run_bounded(command, None, timeout + Duration::from_secs(3))
            .map_err(|error| format!("curl did not run: {error}"))?;
        if run.code == Some(0) {
            Ok(run.stdout)
        } else {
            Err(format!(
                "curl failed ({}): {}",
                run.code
                    .map_or("a signal".to_string(), |code| format!("exit {code}")),
                tail(&run.stderr)
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn the_https_proxy_variable_wins_over_the_all_proxy_one_in_either_case() {
        let both = env(&[("ALL_PROXY", "socks5://a:1"), ("https_proxy", "http://b:2")]);
        assert_eq!(proxy_from_env(&both).as_deref(), Some("http://b:2"));
        let all = env(&[("all_proxy", "socks5://a:1")]);
        assert_eq!(proxy_from_env(&all).as_deref(), Some("socks5://a:1"));
    }

    #[test]
    fn an_empty_or_missing_proxy_variable_is_no_proxy() {
        assert_eq!(proxy_from_env(&env(&[])), None);
        assert_eq!(proxy_from_env(&env(&[("HTTPS_PROXY", "  ")])), None);
    }

    const SCUTIL_HTTPS: &str = "<dictionary> {\n  ExcludeSimpleHostnames : 0\n  HTTPEnable : 0\n  HTTPSEnable : 1\n  HTTPSPort : 8443\n  HTTPSProxy : proxy.corp.example\n  SOCKSEnable : 0\n}\n";

    #[test]
    fn an_enabled_https_proxy_in_the_system_settings_is_used() {
        assert_eq!(
            proxy_from_scutil(SCUTIL_HTTPS).as_deref(),
            Some("http://proxy.corp.example:8443")
        );
    }

    #[test]
    fn a_disabled_https_proxy_falls_to_an_enabled_socks_one() {
        let text = "<dictionary> {\n  HTTPSEnable : 0\n  HTTPSPort : 8443\n  HTTPSProxy : off.example\n  SOCKSEnable : 1\n  SOCKSPort : 1080\n  SOCKSProxy : socks.example\n}\n";
        assert_eq!(
            proxy_from_scutil(text).as_deref(),
            Some("socks5h://socks.example:1080")
        );
    }

    #[test]
    fn system_settings_with_nothing_enabled_or_a_bad_port_give_no_proxy() {
        assert_eq!(
            proxy_from_scutil("<dictionary> {\n  HTTPSEnable : 0\n}\n"),
            None
        );
        assert_eq!(proxy_from_scutil("not a dictionary"), None);
        let bad = "  HTTPSEnable : 1\n  HTTPSProxy : p.example\n  HTTPSPort : none\n";
        assert_eq!(proxy_from_scutil(bad), None);
    }

    #[test]
    fn the_request_is_https_only_quiet_and_carries_no_cookie_or_data() {
        let args = curl_args(
            "https://registry.npmjs.org/x",
            Duration::from_secs(5),
            None,
            None,
        )
        .unwrap();
        assert_eq!(args[0], "-q", "no .curlrc");
        let joined = args.join(" ");
        assert!(joined.contains("--proto =https"), "{joined}");
        assert!(joined.contains("--max-time 5"), "{joined}");
        assert!(joined.contains("--connect-timeout 5"), "{joined}");
        assert!(joined.contains("--fail"), "{joined}");
        assert!(
            joined.ends_with("--url https://registry.npmjs.org/x"),
            "{joined}"
        );
        for forbidden in [
            "--location",
            "--cookie",
            "--data",
            "--upload",
            "--header Cookie",
        ] {
            assert!(!joined.contains(forbidden), "{forbidden} in {joined}");
        }
        assert!(!joined.contains("--proxy"), "{joined}");
    }

    #[test]
    fn a_proxy_and_a_no_proxy_list_are_passed_on() {
        let args = curl_args(
            "https://registry.npmjs.org/x",
            Duration::from_secs(5),
            Some("http://p:1"),
            Some("localhost"),
        )
        .unwrap()
        .join(" ");
        assert!(args.contains("--proxy http://p:1"), "{args}");
        assert!(args.contains("--noproxy localhost"), "{args}");
    }

    #[test]
    fn plain_http_is_refused_unless_it_is_a_loopback_address() {
        for url in [
            "http://registry.npmjs.org/x",
            "ftp://x/y",
            "file:///etc/hosts",
            "http://127.0.0.1.evil.example/x",
        ] {
            assert!(
                curl_args(url, Duration::from_secs(5), None, None).is_err(),
                "{url}"
            );
        }
        for url in [
            "http://127.0.0.1:8080/x",
            "http://localhost:9/x",
            "http://[::1]:7/x",
        ] {
            let args = curl_args(url, Duration::from_secs(5), None, None).unwrap();
            assert!(args.join(" ").contains("--proto =http"), "{url}");
        }
    }
}
