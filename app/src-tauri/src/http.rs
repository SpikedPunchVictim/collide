//! Just enough HTTP/1.1 for the loopback listeners of the app: the UI-tools endpoint ([`crate::endpoint`]) and the
//! test bridge. One request a connection, `Connection: close`, no chunked bodies, no keep-alive. Nothing but `std` is
//! used, so the app adds no dependency (the host chose no HTTP crate either: `host/src/http_get.rs`).
//!
//! This module reads a request and writes a response. What a request may do (the `Host`, the `Origin`, the token) is
//! each listener's own policy.

use std::io::{self, Read, Write};

/// The most a request's head may hold, in bytes.
pub const MAX_HEAD: usize = 16 * 1024;

/// A request, as read. Header names are lower case.
pub struct Request {
    pub method: String,
    /// The target without its query.
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// The first value of a header, by its lower-case name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Why a request could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    /// It is not HTTP/1.x, its head is too long, or the connection ended before it was whole.
    Malformed,
    /// Its `Content-Length` is over what the listener reads. No body was read.
    TooLarge,
}

/// Reads one request. The head is read up to its blank line, and the body up to its `Content-Length`, which may not be
/// over `max_body`.
pub fn read_request(stream: &mut impl Read, max_body: usize) -> Result<Request, ReadError> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = find(&bytes, b"\r\n\r\n") {
            break at;
        }
        if bytes.len() > MAX_HEAD {
            return Err(ReadError::Malformed);
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(ReadError::Malformed),
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
        }
    };
    let head = std::str::from_utf8(&bytes[..head_end]).map_err(|_| ReadError::Malformed)?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (first.next(), first.next(), first.next(), first.next())
    else {
        return Err(ReadError::Malformed);
    };
    if !version.starts_with("HTTP/1.") {
        return Err(ReadError::Malformed);
    }
    let headers: Vec<(String, String)> = lines
        .map(|line| {
            let (name, value) = line.split_once(':').ok_or(ReadError::Malformed)?;
            Ok((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect::<Result<_, ReadError>>()?;
    let length = match headers.iter().find(|(name, _)| name == "content-length") {
        None => 0,
        Some((_, value)) => value.parse::<usize>().map_err(|_| ReadError::Malformed)?,
    };
    if length > max_body {
        return Err(ReadError::TooLarge);
    }
    let mut body = bytes[head_end + 4..].to_vec();
    while body.len() < length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(ReadError::Malformed),
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

/// Writes one response. `extra` are headers added to the ones every response has (a name and a value, with no line
/// break in either).
pub fn write_response(
    stream: &mut impl Write,
    status: (u16, &str),
    content_type: &str,
    extra: &[(&str, &str)],
    body: &[u8],
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         Connection: close\r\n",
        status.0,
        status.1,
        body.len()
    )?;
    for (name, value) in extra {
        write!(stream, "{name}: {value}\r\n")?;
    }
    stream.write_all(b"\r\n")?;
    stream.write_all(body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: usize = 1024 * 1024;

    fn parse(text: &str) -> Result<Request, ReadError> {
        read_request(&mut text.as_bytes(), MAX)
    }

    #[test]
    fn a_request_is_read_with_its_headers_and_body_and_no_query() {
        let request = parse(
            "POST /rpc?x=1 HTTP/1.1\r\nHost: 127.0.0.1:9\r\nContent-Type: application/json\r\n\
             Content-Length: 2\r\n\r\n{}",
        )
        .unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/rpc");
        assert_eq!(request.header("content-type"), Some("application/json"));
        assert_eq!(request.body, b"{}");
    }

    #[test]
    fn a_header_sent_twice_is_kept_twice() {
        let request = parse("GET / HTTP/1.1\r\nHost: a\r\nHost: b\r\n\r\n").unwrap();
        let hosts: Vec<_> = request
            .headers
            .iter()
            .filter(|(n, _)| n == "host")
            .collect();
        assert_eq!(hosts.len(), 2);
    }

    #[test]
    fn a_request_that_is_not_http_is_malformed() {
        assert_eq!(parse("nonsense\r\n\r\n").err(), Some(ReadError::Malformed));
        assert_eq!(
            parse("GET / HTTP/1.1\r\nHost").err(),
            Some(ReadError::Malformed)
        );
        assert_eq!(
            parse("GET / SPDY/3\r\n\r\n").err(),
            Some(ReadError::Malformed)
        );
    }

    #[test]
    fn a_body_over_the_limit_is_too_large_and_one_at_it_is_read() {
        let head = |length: usize| format!("POST / HTTP/1.1\r\nContent-Length: {length}\r\n\r\n");
        assert_eq!(
            parse(&head(MAX + 1)).err(),
            Some(ReadError::TooLarge),
            "no body is read for a length over the limit"
        );
        let at = format!("{}{}", head(MAX), "x".repeat(MAX));
        assert_eq!(parse(&at).unwrap().body.len(), MAX);
    }

    #[test]
    fn a_body_that_ends_early_is_malformed() {
        let short = "POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nabc";
        assert_eq!(parse(short).err(), Some(ReadError::Malformed));
    }

    #[test]
    fn a_response_carries_the_extra_headers_it_is_given() {
        let mut bytes = Vec::new();
        let extra = [("Content-Security-Policy", "default-src 'none'")];
        write_response(&mut bytes, (200, "OK"), "text/html", &extra, b"page").unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(
            text.contains("\r\nContent-Security-Policy: default-src 'none'\r\n"),
            "{text}"
        );
        assert!(text.ends_with("\r\n\r\npage"), "{text}");
    }
}
