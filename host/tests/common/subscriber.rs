//! A client of the daemon's socket that reads notices (ts-0076-cw). `OwnerClient` answers one call at a time and has no
//! way to read what the daemon sends unasked, so the tests of the event stream use this one: it speaks lines, keeps the
//! notices that arrive while it waits for an answer, and says when the daemon closed the connection.

use collide_core::owner::{
    DaemonSubscribeParams, DaemonSubscribeResult, Notice, Notification, OWNER_PROTOCOL,
};
use collide_host::daemon::DaemonPaths;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

/// What waiting for the next notice found.
#[derive(Debug)]
pub enum Next {
    Notice(Notice),
    /// Nothing came within the wait.
    Quiet,
    /// The daemon closed the connection.
    Closed,
}

pub struct Stream {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: u64,
    notices: VecDeque<Notice>,
    /// The start of a line that a timed-out read left. The tests send ASCII, so the start is always valid text.
    partial: String,
}

impl Stream {
    /// Connects and does `initialize` with the token file of `paths`.
    pub fn connect(paths: &DaemonPaths) -> Stream {
        let stream = UnixStream::connect(&paths.socket).expect("the daemon answers");
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut client = Stream {
            stream,
            reader,
            next_id: 1,
            notices: VecDeque::new(),
            partial: String::new(),
        };
        let token = fs::read_to_string(&paths.token_file).unwrap();
        let _: Value = client.call(
            "initialize",
            json!({"protocol": OWNER_PROTOCOL, "token": token.trim()}),
        );
        client
    }

    /// One call, answered with its result. A notice that comes before the answer is kept for [`Stream::next`].
    pub fn call<R: DeserializeOwned>(&mut self, method: &str, params: Value) -> R {
        match self.call_raw(method, params) {
            Ok(result) => {
                serde_json::from_value(result).unwrap_or_else(|e| panic!("{method}: {e}"))
            }
            Err(error) => panic!("{method} failed: {error}"),
        }
    }

    /// One call, answered with the result or the error object.
    pub fn call_raw(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.stream
            .write_all(format!("{request}\n").as_bytes())
            .unwrap();
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let left = until.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "{method} was not answered");
            match self.read_line(left) {
                Line::Text(value) if value.get("id").is_some() => {
                    return match value.get("error") {
                        Some(error) => Err(error.clone()),
                        None => Ok(value["result"].clone()),
                    };
                }
                Line::Text(value) => self.keep(value),
                Line::Quiet => panic!("{method} was not answered"),
                Line::Closed => {
                    panic!("the daemon closed the connection before it answered {method}")
                }
            }
        }
    }

    pub fn subscribe(&mut self, since: Option<u64>) -> DaemonSubscribeResult {
        self.call(
            "daemon.subscribe",
            serde_json::to_value(DaemonSubscribeParams { since }).unwrap(),
        )
    }

    /// The next notice, waiting up to `wait` for one.
    pub fn next(&mut self, wait: Duration) -> Next {
        if let Some(notice) = self.notices.pop_front() {
            return Next::Notice(notice);
        }
        match self.read_line(wait) {
            Line::Text(value) => {
                self.keep(value);
                Next::Notice(self.notices.pop_front().expect("a notice was kept"))
            }
            Line::Quiet => Next::Quiet,
            Line::Closed => Next::Closed,
        }
    }

    /// The `seq` of each `daemon.event` that comes, until `count` have, or the wait runs out for one. Other notices
    /// are skipped.
    pub fn event_seqs(&mut self, count: usize, wait: Duration) -> Vec<u64> {
        let mut seqs = Vec::new();
        while seqs.len() < count {
            match self.next(wait) {
                Next::Notice(Notice::DaemonEvent(event)) => seqs.push(event.seq),
                Next::Notice(_) => {}
                Next::Quiet | Next::Closed => break,
            }
        }
        seqs
    }

    fn keep(&mut self, value: Value) {
        let notification: Notification =
            serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{e}: {value}"));
        let notice = Notice::from_notification(&notification)
            .unwrap_or_else(|e| panic!("{e}: not a notice: {value}"));
        self.notices.push_back(notice);
    }

    fn read_line(&mut self, wait: Duration) -> Line {
        self.stream
            .set_read_timeout(Some(wait.max(Duration::from_millis(1))))
            .unwrap();
        match self.reader.read_line(&mut self.partial) {
            Ok(0) => Line::Closed,
            Ok(_) => {
                let line = std::mem::take(&mut self.partial);
                Line::Text(serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}")))
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                Line::Quiet
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => Line::Closed,
            Err(e) => panic!("reading from the daemon: {e}"),
        }
    }
}

enum Line {
    Text(Value),
    Quiet,
    Closed,
}
