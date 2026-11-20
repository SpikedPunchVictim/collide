//! The published `claude` version (ts-0068-wx; 0.3, Two version chips): fetched from the registry's dist-tags through
//! curl, kept when a fetch fails, refreshed on a timer, switched off by `claude.check_published: false`, and never fetched
//! unless a source was given. The registry is a recorded-shape response served on a loopback address by this file's own
//! server: no test reaches a network. Every curl runs with an empty environment, and every server is stopped on drop.

use super::common::{Grandchild, TempRoot};
use collide_host::adapters::claude::version::{ClaudeVersion, PUBLISHED_URL};
use collide_host::claude_versions::{
    ClaudeVersions, FETCH_TIMEOUT, PublishedSource, REFRESH_EVERY, RegistrySource, VersionsOptions,
};
use collide_host::http_get::{CURL, CurlGet, FixedProxy, HttpGet, NoProxy};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(super) const BODY: &str = include_str!("fixtures/registry-dist-tags.json");

/// What the server does with the next request.
#[derive(Clone)]
pub(super) enum Reply {
    Body(String),
    Status(u16),
    /// Reads the request and says nothing for this long.
    Hang(Duration),
}

/// A loopback HTTP server that answers each request as `replies` says (the last reply repeats) and keeps what it was
/// asked.
pub(super) struct Server {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    pub(super) fn start(replies: Vec<Reply>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (kept, stopping) = (Arc::clone(&requests), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            let mut answered = 0;
            for stream in listener.incoming() {
                if stopping.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(mut stream) = stream else { continue };
                let mut request = String::new();
                let mut buffer = [0u8; 1024];
                while !request.contains("\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => request.push_str(&String::from_utf8_lossy(&buffer[..read])),
                    }
                }
                kept.lock().unwrap().push(request);
                let reply = replies[answered.min(replies.len() - 1)].clone();
                answered += 1;
                match reply {
                    Reply::Body(body) => respond(&mut stream, 200, &body),
                    Reply::Status(code) => respond(&mut stream, code, "{}"),
                    Reply::Hang(how_long) => {
                        let until = Instant::now() + how_long;
                        while Instant::now() < until && !stopping.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                    }
                }
            }
        });
        Server {
            port,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub(super) fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn respond(stream: &mut TcpStream, code: u16, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

pub(super) fn source(url: &str) -> RegistrySource {
    RegistrySource::new(Box::new(CurlGet::new(CURL, Box::new(NoProxy))), url)
}

fn log() -> Arc<dyn Fn(&str) + Send + Sync> {
    Arc::new(|_| {})
}

fn versions(source: Option<Arc<dyn PublishedSource>>, home: Option<&Path>) -> Arc<ClaudeVersions> {
    let versions = ClaudeVersions::new(VersionsOptions {
        refresh_every: REFRESH_EVERY,
        home: home.map(|home| home.to_str().unwrap().to_string()),
        log: log(),
    });
    if let Some(source) = source {
        versions.use_source(source);
    }
    versions
}

fn published_of(versions: &ClaudeVersions) -> Option<collide_core::owner::PublishedVersions> {
    versions.status().agents.remove(0).published
}

fn write_config(root: &TempRoot, text: &str) {
    let dir = root.join(".config/collide");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.yaml"), text).unwrap();
}

#[test]
fn the_registry_answer_gives_latest_and_stable() {
    let server = Server::start(vec![Reply::Body(BODY.into())]);
    let tags = source(&server.url("/dist-tags")).fetch().unwrap();
    assert_eq!(tags.latest, ClaudeVersion::new(2, 1, 325));
    assert_eq!(tags.stable, Some(ClaudeVersion::new(2, 1, 311)));
}

#[test]
fn the_request_carries_no_user_data() {
    let server = Server::start(vec![Reply::Body(BODY.into())]);
    source(&server.url("/-/package/x/dist-tags"))
        .fetch()
        .unwrap();
    let request = server.requests().remove(0);
    let mut lines = request.lines();
    assert_eq!(
        lines.next(),
        Some("GET /-/package/x/dist-tags HTTP/1.1"),
        "no query string, no body"
    );
    let names: Vec<String> = lines
        .take_while(|line| !line.is_empty())
        .map(|line| line.split(':').next().unwrap().to_ascii_lowercase())
        .collect();
    for name in &names {
        assert!(
            ["host", "user-agent", "accept"].contains(&name.as_str()),
            "a header the request should not carry: {name} in {names:?}"
        );
    }
    assert!(request.contains("User-Agent: collide\r\n"), "{request}");
}

#[test]
fn a_server_that_is_not_there_is_a_failed_fetch() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let error = source(&format!("http://127.0.0.1:{port}/x"))
        .fetch()
        .unwrap_err();
    assert!(error.contains("curl failed"), "{error}");
}

#[test]
fn a_server_that_does_not_answer_is_cut_off_at_the_limit() {
    let server = Server::start(vec![Reply::Hang(Duration::from_secs(30))]);
    let started = Instant::now();
    let error = CurlGet::new(CURL, Box::new(NoProxy))
        .get(&server.url("/x"), Duration::from_millis(500))
        .unwrap_err();
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "{:?}",
        started.elapsed()
    );
    assert!(error.contains("exit 28"), "curl's timeout code: {error}");
}

#[test]
fn the_limit_the_chip_uses_is_five_seconds_and_the_refresh_is_a_day() {
    assert_eq!(FETCH_TIMEOUT, Duration::from_secs(5));
    assert_eq!(REFRESH_EVERY, Duration::from_secs(86_400));
}

#[test]
fn an_error_status_a_non_https_address_and_a_body_with_no_latest_are_failed_fetches() {
    let server = Server::start(vec![Reply::Status(500)]);
    assert!(source(&server.url("/x")).fetch().is_err());
    assert!(
        source("http://registry.npmjs.org/x")
            .fetch()
            .unwrap_err()
            .contains("only https")
    );
    let server = Server::start(vec![Reply::Body(r#"{"stable":"2.1.311"}"#.into())]);
    assert!(
        source(&server.url("/x"))
            .fetch()
            .unwrap_err()
            .contains("latest")
    );
}

#[test]
fn a_fetch_through_a_proxy_goes_to_the_proxy_and_not_to_the_address() {
    let proxy = Server::start(vec![Reply::Body(BODY.into())]);
    // Port 9 has no listener: only the proxy can have answered.
    let through = RegistrySource::new(
        Box::new(CurlGet::new(
            CURL,
            Box::new(FixedProxy(format!("http://127.0.0.1:{}", proxy.port))),
        )),
        "http://127.0.0.1:9/dist-tags",
    );
    assert_eq!(
        through.fetch().unwrap().latest,
        ClaudeVersion::new(2, 1, 325)
    );
    let request = proxy.requests().remove(0);
    assert!(
        request.starts_with("GET http://127.0.0.1:9/dist-tags HTTP/1.1"),
        "{request}"
    );
}

#[test]
fn the_published_address_is_the_registrys_dist_tags_of_the_package() {
    assert_eq!(
        PUBLISHED_URL,
        "https://registry.npmjs.org/-/package/@anthropic-ai%2fclaude-code/dist-tags"
    );
}

// ---- the daemon's state ------------------------------------------------------------------------------------------

#[test]
fn a_fetch_fills_the_status_with_both_tags_and_the_time() {
    let server = Server::start(vec![Reply::Body(BODY.into())]);
    let root = TempRoot::new();
    let versions = versions(Some(Arc::new(source(&server.url("/x")))), Some(root.path()));
    assert_eq!(
        published_of(&versions).map(|published| published.latest),
        Some(None),
        "unknown until the first fetch ends"
    );
    versions.refresh_published();
    let published = published_of(&versions).unwrap();
    assert_eq!(published.latest.as_deref(), Some("2.1.325"));
    assert_eq!(published.stable.as_deref(), Some("2.1.311"));
    assert!(published.fetched_ms.is_some());
    assert_eq!(published.error, None);
}

#[test]
fn a_failed_fetch_keeps_the_last_value_and_its_time_and_says_why() {
    let server = Server::start(vec![Reply::Body(BODY.into()), Reply::Status(503)]);
    let root = TempRoot::new();
    let versions = versions(Some(Arc::new(source(&server.url("/x")))), Some(root.path()));
    versions.refresh_published();
    let first = published_of(&versions).unwrap();
    versions.refresh_published();
    let second = published_of(&versions).unwrap();
    assert_eq!(second.latest, first.latest);
    assert_eq!(
        second.fetched_ms, first.fetched_ms,
        "the age is the first fetch's"
    );
    assert!(second.error.is_some());
}

#[test]
fn a_fetch_that_never_succeeded_is_unknown_with_the_error() {
    let server = Server::start(vec![Reply::Status(503)]);
    let root = TempRoot::new();
    let versions = versions(Some(Arc::new(source(&server.url("/x")))), Some(root.path()));
    versions.refresh_published();
    let published = published_of(&versions).unwrap();
    assert_eq!(published.latest, None);
    assert_eq!(published.fetched_ms, None);
    assert!(published.error.is_some());
}

#[test]
fn check_published_false_stops_the_fetch_and_hides_the_chip() {
    let server = Server::start(vec![Reply::Body(BODY.into())]);
    let root = TempRoot::new();
    write_config(&root, "claude:\n  check_published: false\n");
    let versions = versions(Some(Arc::new(source(&server.url("/x")))), Some(root.path()));
    versions.refresh_published();
    assert_eq!(published_of(&versions), None);
    assert_eq!(server.requests().len(), 0, "no request was made");
}

#[test]
fn check_published_is_read_again_at_each_refresh() {
    let server = Server::start(vec![Reply::Body(BODY.into())]);
    let root = TempRoot::new();
    let versions = versions(Some(Arc::new(source(&server.url("/x")))), Some(root.path()));
    versions.refresh_published();
    assert_eq!(server.requests().len(), 1);
    write_config(&root, "claude:\n  check_published: false\n");
    versions.refresh_published();
    assert_eq!(server.requests().len(), 1);
    assert_eq!(published_of(&versions), None);
}

#[test]
fn a_config_file_that_cannot_be_read_leaves_the_fetch_on_and_says_so() {
    let server = Server::start(vec![Reply::Body(BODY.into())]);
    let root = TempRoot::new();
    write_config(&root, "claude: [not, a, map\n");
    let logged = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&logged);
    let versions = ClaudeVersions::new(VersionsOptions {
        refresh_every: REFRESH_EVERY,
        home: Some(root.path().to_str().unwrap().to_string()),
        log: Arc::new(move |line| sink.lock().unwrap().push(line.to_string())),
    });
    versions.use_source(Arc::new(source(&server.url("/x"))));
    versions.refresh_published();
    assert_eq!(server.requests().len(), 1);
    assert!(
        logged
            .lock()
            .unwrap()
            .iter()
            .any(|line| line.contains("config.yaml")),
        "{:?}",
        logged.lock().unwrap()
    );
}

#[test]
fn with_no_source_nothing_is_fetched_and_the_chip_is_hidden() {
    let root = TempRoot::new();
    let versions = versions(None, Some(root.path()));
    versions.refresh_published();
    assert_eq!(published_of(&versions), None);
}

#[test]
fn the_published_versions_are_fetched_at_start_and_again_on_the_timer_until_stopped() {
    let server = Server::start(vec![Reply::Body(BODY.into())]);
    let root = TempRoot::new();
    let versions = ClaudeVersions::new(VersionsOptions {
        refresh_every: Duration::from_millis(150),
        home: Some(root.path().to_str().unwrap().to_string()),
        log: log(),
    });
    let thread = versions.start_refreshing(Arc::new(source(&server.url("/x"))));
    super::common::wait_until("three fetches", Duration::from_secs(20), || {
        server.requests().len() >= 3
    });
    versions.stop();
    thread.join().unwrap();
    let after_stop = server.requests().len();
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(server.requests().len(), after_stop, "no fetch after stop");
}

#[test]
fn a_curl_whose_stdout_a_grandchild_holds_open_is_an_error_not_an_empty_body() {
    let root = TempRoot::new();
    let pid_file = root.join("pid");
    let mut sleeper = Grandchild::new(pid_file.clone());
    let curl = root.script(
        "curl",
        &format!(
            "#!/bin/sh\n/bin/sleep 30 2>/dev/null &\necho $! > {}\necho '{{}}'\n",
            pid_file.display()
        ),
    );
    let result = CurlGet::new(curl.to_str().unwrap(), Box::new(NoProxy))
        .get("http://127.0.0.1:9/x", Duration::from_secs(5));
    sleeper.pid(Duration::from_secs(10));
    let error = result.unwrap_err();
    assert!(error.contains("not read in time"), "{error}");
}
