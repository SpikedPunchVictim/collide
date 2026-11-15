//! The OTLP receiver (0.2, The OTLP receiver; 0.13, Claude Code's own telemetry; R5) as a declaration. It is part
//! of the daemon: Claude Code exports its logs and metrics (and, behind a beta flag, traces) over OTLP, and the
//! receiver takes them for interactive sessions and `agentWork` phases. Nothing listens here. What it must refuse is
//! written as constants and types, so that Stage 9b fills in a server that already has its limits.
//!
//! The limits, all four required (the Stage 9b criterion names each):
//!
//! | limit | declared as |
//! |---|---|
//! | binds loopback only, on a port the daemon picks | [`BIND_ADDRESS`] |
//! | refuses a foreign `Host` header | [`ALLOWED_HOST_NAMES`], [`OtlpRefusal::ForeignHost`] |
//! | wants the run's token as a bearer header | [`RunToken`], [`OtlpRefusal::MissingToken`], [`OtlpRefusal::WrongToken`] |
//! | caps a request body | [`MAX_BODY_BYTES`], [`OtlpRefusal::BodyTooLarge`] |
//!
//! The token is made per run and handed to that run's child in `OTEL_EXPORTER_OTLP_HEADERS`, so it is in the child's
//! environment but is good for this receiver only. Records are attributed by `session.id` and the resource
//! attributes `collide.run` and `collide.phase`, and the account's identity is dropped on arrival, before anything is
//! stored ([`IDENTITY_ATTRIBUTES`]).
//!
//! The module takes the request as already-read values ([`OtlpRequest`]), not as a type of an HTTP crate, because the
//! crate arrives with Stage 9b and the checks must not depend on it.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `otlp: receiver` | [`Receiver::bind`] | binds [`BIND_ADDRESS`] on a port the daemon picks and serves |
//! | `otlp: run token` | [`RunToken::mint`], [`Receiver::register_run`] | a token per run, and the run it stands for |
//! | `otlp: request check` | [`Receiver::check`] | the `Host`, token and body-size checks, in that order |
//! | `otlp: drop identity` | [`drop_identity`] | removes the account's identity attributes from a record |

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::net::Ipv4Addr;

/// The only address the receiver binds (0.2): the loopback interface. `lsof` shows no other.
pub const BIND_ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;

/// The `Host` names the receiver accepts, each with the port it is bound to. A request for any other name is
/// [`OtlpRefusal::ForeignHost`], which closes the way a web page has to reach the receiver through a name it
/// controls (DNS rebinding).
pub const ALLOWED_HOST_NAMES: [&str; 2] = ["127.0.0.1", "localhost"];

/// The largest request body the receiver reads, in bytes. The plan says the receiver "caps a request body" and gives
/// no number; this is OTLP's own default limit for a request.
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// The authorization scheme of the token.
pub const BEARER_SCHEME: &str = "Bearer";

/// The attributes the receiver drops from every record on arrival (0.13): the account's identity, which every
/// Claude Code record carries (measured S2). Stage 9b checks these keys against S2's capture.
pub const IDENTITY_ATTRIBUTES: [&str; 4] = [
    "user.email",
    "user.id",
    "user.account_uuid",
    "organization.id",
];

/// A run's token for the receiver. Its `Debug` and `Display` never show the secret, so a log line cannot leak it.
#[derive(Clone, PartialEq, Eq)]
pub struct RunToken(String);

impl RunToken {
    /// Makes a fresh, unguessable token.
    pub fn mint() -> RunToken {
        todo!("otlp: run token")
    }

    /// The token's text, for the header handed to the run's child and for nothing else.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RunToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RunToken(<redacted>)")
    }
}

impl fmt::Display for RunToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// What the receiver reads of a request before it reads the body: the `Host` header, the `Authorization` header and
/// the declared body length. Each is `None` when the header is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OtlpRequest<'a> {
    pub host: Option<&'a str>,
    pub authorization: Option<&'a str>,
    pub body_len: usize,
}

/// Why a request is refused. The checks run in the order of the variants, so a request with a foreign `Host` is
/// refused as that, whatever else is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OtlpRefusal {
    /// The `Host` header is absent or names something other than [`ALLOWED_HOST_NAMES`] at the receiver's port.
    ForeignHost { host: Option<String> },
    /// There is no `Authorization: Bearer <token>` header.
    MissingToken,
    /// The token is not one the receiver made for a live run.
    WrongToken,
    /// The body is over [`MAX_BODY_BYTES`].
    BodyTooLarge { len: usize, max: usize },
}

impl fmt::Display for OtlpRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OtlpRefusal::ForeignHost { host: Some(host) } => {
                write!(f, "the Host `{host}` is not this receiver's own address")
            }
            OtlpRefusal::ForeignHost { host: None } => {
                f.write_str("the request has no Host header")
            }
            OtlpRefusal::MissingToken => f.write_str("the request has no bearer token"),
            OtlpRefusal::WrongToken => f.write_str("the bearer token is not a live run's"),
            OtlpRefusal::BodyTooLarge { len, max } => {
                write!(f, "the body is {len} bytes, over the cap of {max}")
            }
        }
    }
}

impl std::error::Error for OtlpRefusal {}

/// The receiver: the port it is bound to and the token of each run that may send to it.
#[derive(Debug)]
pub struct Receiver {
    /// The port the daemon picked.
    pub port: u16,
    /// Run id to its token.
    pub tokens: BTreeMap<String, RunToken>,
}

impl Receiver {
    /// Binds [`BIND_ADDRESS`] on a port the OS picks, and starts serving.
    pub fn bind() -> io::Result<Receiver> {
        todo!("otlp: receiver")
    }

    /// Makes the token for `run` and remembers it. The caller puts it in that run's child's telemetry settings.
    pub fn register_run(&mut self, run: &str) -> RunToken {
        let _ = run;
        todo!("otlp: run token")
    }

    /// Checks a request, and returns the id of the run its token belongs to.
    pub fn check(&self, request: &OtlpRequest<'_>) -> Result<String, OtlpRefusal> {
        let _ = request;
        todo!("otlp: request check")
    }
}

/// Removes [`IDENTITY_ATTRIBUTES`] from a record's attributes, in place, before the record is stored.
pub fn drop_identity(attributes: &mut BTreeMap<String, String>) {
    let _ = attributes;
    todo!("otlp: drop identity")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_receiver_binds_loopback_only() {
        assert!(BIND_ADDRESS.is_loopback());
        assert_eq!(BIND_ADDRESS.to_string(), "127.0.0.1");
    }

    #[test]
    fn a_body_cap_is_declared_and_is_not_zero() {
        const { assert!(MAX_BODY_BYTES > 0) };
    }

    #[test]
    fn the_allowed_host_names_are_loopback_names_only() {
        assert_eq!(ALLOWED_HOST_NAMES, ["127.0.0.1", "localhost"]);
    }

    #[test]
    fn a_token_never_shows_its_secret_in_debug_or_display() {
        let token = RunToken("s3cret".to_string());
        assert!(!format!("{token:?}").contains("s3cret"));
        assert!(!token.to_string().contains("s3cret"));
        assert_eq!(token.reveal(), "s3cret");
    }

    #[test]
    fn a_refusal_says_what_was_wrong() {
        let over = OtlpRefusal::BodyTooLarge { len: 9, max: 4 }.to_string();
        assert!(over.contains('9') && over.contains('4'), "{over}");
        let host = OtlpRefusal::ForeignHost {
            host: Some("evil.test".to_string()),
        }
        .to_string();
        assert!(host.contains("evil.test"), "{host}");
    }

    #[test]
    #[should_panic(expected = "otlp: receiver")]
    fn binding_is_a_stub_tagged_for_stage_9b() {
        let _ = Receiver::bind();
    }
}
