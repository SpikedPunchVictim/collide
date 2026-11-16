//! Just enough HTTP/1.1 for the bridge: one request a connection, `Connection: close`, no chunked bodies, no
//! keep-alive. Nothing but `std` is used, so the bridge adds no dependency.
//!
//! The policy is the S6 record's endpoint checks, and a token: a foreign `Host` is refused (DNS rebinding), a foreign
//! `Origin` is refused, a call has to carry the run's token as a bearer, and a body has to say it is JSON, so a browser
//! has to preflight and a preflight is never answered.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// The most a request's head may hold, and the most its body may.
const MAX_HEAD: usize = 16 * 1024;
const MAX_BODY: usize = 1024 * 1024;

pub struct Request {
    pub method: String,
    /// The target without its query.
    pub path: String,
    headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// The value of a header, by its lower-case name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Why a request was not answered by the route it asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    UnsupportedMediaType,
}

impl Refusal {
    pub fn status(self) -> (u16, &'static str) {
        match self {
            Refusal::BadRequest => (400, "Bad Request"),
            Refusal::Unauthorized => (401, "Unauthorized"),
            Refusal::Forbidden => (403, "Forbidden"),
            Refusal::NotFound => (404, "Not Found"),
            Refusal::MethodNotAllowed => (405, "Method Not Allowed"),
            Refusal::UnsupportedMediaType => (415, "Unsupported Media Type"),
        }
    }
}

/// Reads one request. The head is read up to its blank line, and the body up to its `Content-Length`.
pub fn read_request(stream: &mut impl Read) -> Result<Request, Refusal> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = find(&bytes, b"\r\n\r\n") {
            break at;
        }
        if bytes.len() > MAX_HEAD {
            return Err(Refusal::BadRequest);
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(Refusal::BadRequest),
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
        }
    };
    let head = std::str::from_utf8(&bytes[..head_end]).map_err(|_| Refusal::BadRequest)?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (first.next(), first.next(), first.next(), first.next())
    else {
        return Err(Refusal::BadRequest);
    };
    if !version.starts_with("HTTP/1.") {
        return Err(Refusal::BadRequest);
    }
    let headers: Vec<(String, String)> = lines
        .map(|line| {
            let (name, value) = line.split_once(':').ok_or(Refusal::BadRequest)?;
            Ok((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect::<Result<_, Refusal>>()?;
    let length = match headers.iter().find(|(name, _)| name == "content-length") {
        None => 0,
        Some((_, value)) => value.parse::<usize>().map_err(|_| Refusal::BadRequest)?,
    };
    if length > MAX_BODY {
        return Err(Refusal::BadRequest);
    }
    let mut body = bytes[head_end + 4..].to_vec();
    while body.len() < length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(Refusal::BadRequest),
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    body.truncate(length);
    Ok(Request {
        method: method.to_string(),
        path: target.split('?').next().unwrap_or_default().to_string(),
        headers,
        body,
    })
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Who may call: the `Host` names the bridge's own loopback address, and the token is the one made for this run.
pub struct Policy {
    hosts: Vec<String>,
    token: String,
}

impl Policy {
    pub fn new(port: u16, token: String) -> Policy {
        Policy {
            hosts: vec![format!("127.0.0.1:{port}"), format!("localhost:{port}")],
            token,
        }
    }

    /// The first two checks, which every request meets, the page included: the `Host`, and the `Origin` when there is
    /// one. A request with no `Origin` is not from a page of another site.
    pub fn check_origin(&self, request: &Request) -> Result<(), Refusal> {
        let host = request.header("host").ok_or(Refusal::Forbidden)?;
        if !self.hosts.iter().any(|allowed| allowed == host) {
            return Err(Refusal::Forbidden);
        }
        match request.header("origin") {
            None => Ok(()),
            Some(origin) if origin == format!("http://{host}") => Ok(()),
            Some(_) => Err(Refusal::Forbidden),
        }
    }

    /// The run's token, as a bearer.
    pub fn check_token(&self, request: &Request) -> Result<(), Refusal> {
        let given = request
            .header("authorization")
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(Refusal::Unauthorized)?;
        if same(given.as_bytes(), self.token.as_bytes()) {
            Ok(())
        } else {
            Err(Refusal::Unauthorized)
        }
    }
}

/// A body of JSON, so that a browser cannot send it without a preflight.
pub fn check_json(request: &Request) -> Result<(), Refusal> {
    let kind = request.header("content-type").unwrap_or_default();
    let kind = kind.split(';').next().unwrap_or_default().trim();
    if kind.eq_ignore_ascii_case("application/json") {
        Ok(())
    } else {
        Err(Refusal::UnsupportedMediaType)
    }
}

/// Compares two byte strings without stopping at the first difference.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

pub fn write_response(
    stream: &mut impl Write,
    status: (u16, &str),
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        status.0,
        status.1,
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

/// The file of the page that `path` names, below `dir`. `/` is the page itself. A path with a dot-dot, a percent escape
/// or a backslash names nothing.
pub fn file_for(dir: &Path, path: &str) -> Option<PathBuf> {
    let relative = match path {
        "/" => "bridge.html",
        other => other.strip_prefix('/')?,
    };
    if relative.contains('%') || relative.contains('\\') {
        return None;
    }
    let mut file = dir.to_path_buf();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return None;
        }
        file.push(part);
    }
    file.is_file().then_some(file)
}

pub fn content_type_of(file: &Path) -> &'static str {
    match file.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Request, Refusal> {
        read_request(&mut text.as_bytes())
    }

    fn call(host: &str, extra: &str) -> Request {
        parse(&format!(
            "POST /rpc?x=1 HTTP/1.1\r\nHost: {host}\r\n{extra}Content-Length: 2\r\n\r\n{{}}"
        ))
        .unwrap()
    }

    #[test]
    fn a_request_is_read_with_its_headers_and_body_and_no_query() {
        let request = call("127.0.0.1:9", "Content-Type: application/json\r\n");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/rpc");
        assert_eq!(request.header("content-type"), Some("application/json"));
        assert_eq!(request.body, b"{}");
    }

    #[test]
    fn a_request_that_is_not_http_or_is_too_big_is_refused() {
        assert_eq!(parse("nonsense\r\n\r\n").err(), Some(Refusal::BadRequest));
        assert_eq!(
            parse("GET / HTTP/1.1\r\nHost").err(),
            Some(Refusal::BadRequest)
        );
        let big = format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY + 1
        );
        assert_eq!(parse(&big).err(), Some(Refusal::BadRequest));
    }

    #[test]
    fn only_the_loopback_name_and_port_of_the_bridge_are_a_host() {
        let policy = Policy::new(9, "t".into());
        assert_eq!(policy.check_origin(&call("127.0.0.1:9", "")), Ok(()));
        assert_eq!(policy.check_origin(&call("localhost:9", "")), Ok(()));
        for foreign in [
            "evil.example",
            "127.0.0.1:10",
            "127.0.0.1",
            "127.0.0.1:9.evil.example",
        ] {
            assert_eq!(
                policy.check_origin(&call(foreign, "")),
                Err(Refusal::Forbidden),
                "{foreign}"
            );
        }
    }

    #[test]
    fn an_origin_has_to_be_the_bridges_own() {
        let policy = Policy::new(9, "t".into());
        let own = call("127.0.0.1:9", "Origin: http://127.0.0.1:9\r\n");
        assert_eq!(policy.check_origin(&own), Ok(()));
        for foreign in [
            "http://evil.example",
            "http://127.0.0.1:1",
            "null",
            "https://127.0.0.1:9",
        ] {
            let request = call("127.0.0.1:9", &format!("Origin: {foreign}\r\n"));
            assert_eq!(
                policy.check_origin(&request),
                Err(Refusal::Forbidden),
                "{foreign}"
            );
        }
    }

    #[test]
    fn the_token_has_to_be_a_bearer_and_the_right_one() {
        let policy = Policy::new(9, "secret".into());
        let with = |value: &str| call("127.0.0.1:9", &format!("Authorization: {value}\r\n"));
        assert_eq!(policy.check_token(&with("Bearer secret")), Ok(()));
        for wrong in ["Bearer nope", "Bearer secre", "secret", "Basic secret"] {
            assert_eq!(
                policy.check_token(&with(wrong)),
                Err(Refusal::Unauthorized),
                "{wrong}"
            );
        }
        assert_eq!(
            policy.check_token(&call("127.0.0.1:9", "")),
            Err(Refusal::Unauthorized)
        );
    }

    #[test]
    fn a_body_has_to_say_it_is_json() {
        let json = call(
            "127.0.0.1:9",
            "Content-Type: application/json; charset=utf-8\r\n",
        );
        assert_eq!(check_json(&json), Ok(()));
        let text = call("127.0.0.1:9", "Content-Type: text/plain\r\n");
        assert_eq!(check_json(&text), Err(Refusal::UnsupportedMediaType));
        assert_eq!(
            check_json(&call("127.0.0.1:9", "")),
            Err(Refusal::UnsupportedMediaType)
        );
    }

    #[test]
    fn a_path_never_leaves_the_folder_of_the_page() {
        let dir = std::env::temp_dir().join(format!("collide-bridge-http-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("bridge.html"), "page").unwrap();
        std::fs::write(dir.join("assets/a.js"), "js").unwrap();
        assert_eq!(file_for(&dir, "/"), Some(dir.join("bridge.html")));
        assert_eq!(
            file_for(&dir, "/assets/a.js"),
            Some(dir.join("assets/a.js"))
        );
        for refused in [
            "/../x",
            "/assets/../bridge.html",
            "//etc/passwd",
            "/%2e%2e/x",
            "/assets\\a.js",
            "/missing",
        ] {
            assert_eq!(file_for(&dir, refused), None, "{refused}");
        }
        std::fs::remove_file(dir.join("bridge.html")).unwrap();
        std::fs::remove_file(dir.join("assets/a.js")).unwrap();
        std::fs::remove_dir(dir.join("assets")).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
