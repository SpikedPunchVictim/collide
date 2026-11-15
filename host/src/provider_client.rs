//! A client for a provider over its stdio (0.10, Protocol): newline-delimited JSON-RPC 2.0, one request at a time.
//! The CLI uses it until the daemon's provider supervisor exists. The provider's stderr goes to this process's own
//! stderr, so its log lines reach the person at the terminal.

use crate::builtin_packs::ProviderLaunch;
use collide_core::provider::{
    ApplyParams, ApplyResult, GetParams, GetResult, InitializeParams, InitializeResult, ListParams,
    ListResult, Task, TaskInvalid,
};
use serde_json::{Value, json};
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};

#[derive(Debug)]
pub enum ClientError {
    /// The provider process did not start.
    Spawn { program: String, error: io::Error },
    /// Writing a request or reading a reply failed.
    Io { method: String, error: io::Error },
    /// The provider closed its output before it answered.
    Closed {
        method: String,
        status: Option<String>,
    },
    /// A line of the provider's output is not JSON, or a reply is not what the method returns.
    Malformed { method: String, detail: String },
    /// The provider answered with a JSON-RPC error.
    Rpc {
        method: String,
        code: i64,
        message: String,
    },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { program, error } => {
                write!(f, "could not start the provider `{program}`: {error}")
            }
            Self::Io { method, error } => {
                write!(f, "talking to the provider ({method}) failed: {error}")
            }
            Self::Closed { method, status } => {
                let status = status
                    .as_ref()
                    .map(|status| format!(" ({status})"))
                    .unwrap_or_default();
                write!(
                    f,
                    "the provider exited{status} before it answered `{method}`; see its messages above"
                )
            }
            Self::Malformed { method, detail } => {
                write!(
                    f,
                    "the provider's answer to `{method}` is not valid: {detail}"
                )
            }
            Self::Rpc {
                method,
                code,
                message,
            } => write!(
                f,
                "the provider refused `{method}` (error {code}): {message}"
            ),
        }
    }
}

impl std::error::Error for ClientError {}

/// A running provider. Dropping it closes the provider's input, which ends it, and waits for it to exit.
pub struct ProviderClient {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    next_id: u64,
}

impl ProviderClient {
    /// Starts the provider with `cwd` as its working directory. It never reads the root from there (`initialize`
    /// carries it); the directory is only somewhere that exists.
    pub fn start(launch: &ProviderLaunch, cwd: &std::path::Path) -> Result<Self, ClientError> {
        let mut child = launch
            .command()
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| ClientError::Spawn {
                program: launch.program.display().to_string(),
                error,
            })?;
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().expect("stdout is piped"));
        Ok(Self {
            child,
            input,
            output,
            next_id: 1,
        })
    }

    /// Sends one request and returns its `result`. Lines without an `id` are notifications and are skipped.
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, ClientError> {
        let id = self.next_id;
        self.next_id += 1;
        let io_error = |error| ClientError::Io {
            method: method.to_string(),
            error,
        };
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let input = self.input.as_mut().ok_or_else(|| ClientError::Closed {
            method: method.to_string(),
            status: None,
        })?;
        writeln!(input, "{request}")
            .and_then(|()| input.flush())
            .map_err(io_error)?;
        loop {
            let mut line = String::new();
            if self.output.read_line(&mut line).map_err(io_error)? == 0 {
                self.input = None;
                let status = self.child.wait().ok().map(|status| status.to_string());
                return Err(ClientError::Closed {
                    method: method.to_string(),
                    status,
                });
            }
            let reply: Value =
                serde_json::from_str(line.trim()).map_err(|error| ClientError::Malformed {
                    method: method.to_string(),
                    detail: format!("`{}` is not JSON: {error}", line.trim()),
                })?;
            if reply.get("id").is_none_or(Value::is_null) {
                continue;
            }
            if reply["id"] != json!(id) {
                return Err(ClientError::Malformed {
                    method: method.to_string(),
                    detail: format!(
                        "a reply to request {} arrived while waiting for {id}",
                        reply["id"]
                    ),
                });
            }
            if let Some(error) = reply.get("error") {
                return Err(ClientError::Rpc {
                    method: method.to_string(),
                    code: error["code"].as_i64().unwrap_or(0),
                    message: error["message"]
                        .as_str()
                        .unwrap_or("no message")
                        .to_string(),
                });
            }
            return reply
                .get("result")
                .cloned()
                .ok_or_else(|| ClientError::Malformed {
                    method: method.to_string(),
                    detail: "the reply has neither result nor error".to_string(),
                });
        }
    }

    /// Calls `method` and returns the reply as a JSON value, for the caller to read as the method's result type.
    fn call(
        &mut self,
        method: &str,
        params: Result<Value, serde_json::Error>,
    ) -> Result<Value, ClientError> {
        let params = params.expect("protocol params are JSON");
        self.request(method, params)
    }

    pub fn initialize(
        &mut self,
        params: &InitializeParams,
    ) -> Result<InitializeResult, ClientError> {
        let result = self.call("initialize", serde_json::to_value(params))?;
        serde_json::from_value(result).map_err(|error| malformed("initialize", error))
    }

    /// Every task, following `nextCursor` to the end, and every item the source could not read. `checkout` names
    /// the checkout to read; none means the main checkout.
    pub fn list_all(
        &mut self,
        checkout: Option<&str>,
    ) -> Result<(Vec<Task>, Vec<TaskInvalid>), ClientError> {
        let mut params = ListParams {
            checkout: checkout.map(str::to_string),
            cursor: None,
        };
        let (mut tasks, mut invalid) = (Vec::new(), Vec::new());
        loop {
            let result = self.call("list", serde_json::to_value(&params))?;
            let page: ListResult =
                serde_json::from_value(result).map_err(|error| malformed("list", error))?;
            tasks.extend(page.tasks);
            invalid.extend(page.invalid);
            match page.next_cursor {
                Some(cursor) => params.cursor = Some(cursor),
                None => return Ok((tasks, invalid)),
            }
        }
    }

    pub fn get(&mut self, id: &str, checkout: Option<&str>) -> Result<Option<Task>, ClientError> {
        let params = GetParams {
            id: id.to_string(),
            checkout: checkout.map(str::to_string),
        };
        let result = self.call("get", serde_json::to_value(&params))?;
        let result: GetResult =
            serde_json::from_value(result).map_err(|error| malformed("get", error))?;
        Ok(result.task)
    }

    pub fn apply(&mut self, params: &ApplyParams) -> Result<ApplyResult, ClientError> {
        let result = self.call("apply", serde_json::to_value(params))?;
        serde_json::from_value(result).map_err(|error| malformed("apply", error))
    }
}

fn malformed(method: &str, error: serde_json::Error) -> ClientError {
    ClientError::Malformed {
        method: method.to_string(),
        detail: error.to_string(),
    }
}

impl Drop for ProviderClient {
    fn drop(&mut self) {
        self.input = None;
        let _ = self.child.wait();
    }
}
