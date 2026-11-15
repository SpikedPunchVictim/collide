//! The Claude session (0.3, Two core traits; The stream protocol): one per child, a sans-IO state machine. It holds what
//! a Claude child needs remembered: the permission requests waiting for an answer, whether a cancel was sent, how
//! the last turn ended, and, for the turn in flight, its calls without a result, its failed tool hooks and a blocked
//! prompt. `feed` is the stream parser measured by S1 and S9. The session reads no clock and opens no file;
//! the same input gives the same output, so a daemon restart rebuilds it from the shim's journal with `replay`.
//!
//! Every turn, the first included, is a `user` message on stdin, so `start` writes nothing: it reports the session id
//! at once, because the launch line passed it (`--session-id`, or `--resume`).
//!
//! What a line of the child's stdout becomes (`feed`; every event is stamped with the line's `wall_ns`):
//!
//! | the child writes | the session reads |
//! |---|---|
//! | `system/init`, `system/hook_started` | nothing: the id was bound at `start` and `init` comes on every turn; `hook_response` has all that `hook_started` has |
//! | `assistant` | a `Step` `Usage` for a response not seen before (input and cache tokens, once per `message.id`), a `Message` for each non-empty text block, a `ToolCall` for each `tool_use`; thinking is not an event; `error: "rate_limit"` is a limit signal |
//! | `rate_limit_event` | `rejected` is a limit signal and no event of its own; `allowed_warning` and any other status are an `Other` and pause nothing |
//! | `control_request` `can_use_tool` | a `PermissionRequest`, kept until `answer`; a hook's `ask` carries the hook's text as its `reason` |
//! | `system/hook_response` | a `HookCall`: name, event, outcome, exit code and the `decision` read from `stdout`; neither `stdout`, `stderr` nor `output` is kept |
//! | `system/informational` with `hook_no_verdict` | nothing after a `cancelled` `hook_response`, which it names the call of; else the `HookCall` itself |
//! | `system/informational` with `prompt_submit_outcome` | nothing: the turn's `result` ends `Blocked` with the hook's reason |
//! | `user` with `tool_result` | a `ToolResultSize` for each result; a `ToolRefused` for a call that did not run, by the `tool_result_meta` entry with its id (never its text); a `HookFailedOpen` for a failed tool hook whose tool ran |
//! | `system/compact_boundary` | a `Compaction` (trigger, tokens before, duration, mid-task); the turn's `result` then reads the compaction call from its model usage |
//! | `result` | a `FinalReport` from `result.result` (never for an error result), the `Usage` events below, a `Limited` for a limit, then a `TurnEnd`; a 429 error result is a limit signal |
//! | anything else | an `Other` named by the line's `type`, and its `subtype` when it has one; no content is kept |
//!
//! Usage, from every `result` (an error result's included), in this order:
//!
//! | event | what it holds |
//! |---|---|
//! | `Compaction`, or `OtherCalls` | what `model_usage_since_last_result` counts beyond `usage`, summed over the models: the call a compaction made (a `compact_boundary` came this turn, or `local_command` is `compact`), or any other model call. Its cost is given only when the turn used no tokens of its own |
//! | `Turn` | tokens from `usage`; cost as the running total less the one before it, less a compaction's cost given above; model time as `duration_api_ms` less the last result with turns; the context of the turn's last response and the window of its model |
//! | `RunningTotal` | `total_cost_usd` as reported, nothing else |
//!
//! `Step` usages are one per response, for context per response, and are never added to a `Turn`. A turn that was sent and
//! has no `result` when the child ends gives, at `finish`, a `Turn` usage of its responses' input and cache tokens
//! (`Partial`; `Unreported` with no response), with no output and no cost.
//!
//! A limit is one `Limited`: the signals of a turn (`rate_limit_event` `rejected`, an assistant `error: "rate_limit"`,
//! a 429 error result) are kept, and the event is given at the turn's `result`, or at `finish` if none came, with the
//! first reset time any of them had (`resetsAt`, epoch seconds, as milliseconds).
//!
//! `result.permission_denials` is never read: it lists hook, host, rule and auto denies alike, so it says nothing of
//! who refused a call.
//!
//! What the session writes (stdin), for the calls that write:
//!
//! | call | the message |
//! |---|---|
//! | `send_turn` | `{"type":"user","message":{"role":"user","content":[{"type":"text","text":...}]},...}` |
//! | `answer` | `{"type":"control_response","response":{"subtype":"success","request_id":...,"response":{"behavior":...}}}` |
//! | `cancel` | `{"type":"control_request","request_id":"collide-interrupt-N","request":{"subtype":"interrupt"}}` |
//!
//! A live write goes through [`Replayable::note_written`], the same code a replay runs over an `in` record, so a rebuilt
//! session and a live one reach the same state. A replay is [`replay_records`] over this session's [`Replayable`].

use collide_core::adapter::{
    AgentSession, ExitInfo, Finished, PhaseFailure, PhaseOutcome, ReplayError, StampedEvent, Step,
};
use collide_core::agent::{
    AgentEvent, CompactionTrigger, FinalReportSource, HookDecision, HookFailure, HookOutcome,
    Money, PermissionReply, PermissionRequest, RefusalCause, SessionStart, ToolKind, TurnEndReason,
    Usage, UsageQuality, UsageScope,
};
use collide_core::journal::JournalRecord;
use collide_core::replay::{Replayable, replay_records};
use serde_json::{Map, Value, json};

/// The message of a `deny` that carries none: Claude requires one.
const DENY_WITHOUT_MESSAGE: &str = "The host denied this tool call.";

/// The message of a `Cancel` answer. Claude has no cancel answer; this is a deny that says so, and the turn goes on.
/// `cancel()` is what interrupts it.
const CANCELLED_REQUEST: &str = "The host cancelled this request.";

/// The HTTP status of a usage limit (S1: a limit is an error result with 429).
const USAGE_LIMIT_STATUS: u64 = 429;

/// The `error` of an assistant message that is a usage or rate limit (S1, Q8).
const RATE_LIMIT_ERROR: &str = "rate_limit";

/// The exit code with which a hook blocks on purpose (S9).
const HOOK_BLOCK_EXIT_CODE: i32 = 2;

/// A permission request that has been asked and not answered. `input` is what an `allow` echoes back as
/// `updatedInput`, and is `None` when the request carried none.
#[derive(Debug, Clone, PartialEq)]
struct Pending {
    id: String,
    input: Option<Value>,
    /// The request as it came, for a restart that asks the person again.
    request: PermissionRequest,
}

/// A tool call that has been made and has no result yet. `tool` is what a hook's name is joined to.
#[derive(Debug, Clone, PartialEq)]
struct CallInFlight {
    id: String,
    tool: Option<String>,
}

/// A tool hook that failed without stopping its call, kept until the call's result says whether the tool ran.
/// `call_id` is known only when the stream named the call (`hook_no_verdict` does); otherwise the hook is joined to a
/// call by its tool's name.
#[derive(Debug, Clone, PartialEq)]
struct FailedHook {
    name: String,
    event: String,
    cause: HookFailure,
    call_id: Option<String>,
}

/// Token counts, each `None` when the agent did not report it (never 0). Read from a response's or a result's `usage`
/// (snake case), or from one model's entry of `model_usage_since_last_result` (camel case).
#[derive(Debug, Clone, Default, PartialEq)]
struct Tokens {
    input: Option<u64>,
    output: Option<u64>,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    cache_write_1h: Option<u64>,
    cache_write_5m: Option<u64>,
    thought: Option<u64>,
}

impl Tokens {
    fn from_usage(usage: &Value) -> Self {
        let count = |pointer: &str| usage.pointer(pointer).and_then(Value::as_u64);
        Self {
            input: count("/input_tokens"),
            output: count("/output_tokens"),
            cache_read: count("/cache_read_input_tokens"),
            cache_write: count("/cache_creation_input_tokens"),
            cache_write_1h: count("/cache_creation/ephemeral_1h_input_tokens"),
            cache_write_5m: count("/cache_creation/ephemeral_5m_input_tokens"),
            thought: count("/output_tokens_details/thinking_tokens"),
        }
    }

    fn from_model_usage(entry: &Value) -> Self {
        let count = |key: &str| entry.get(key).and_then(Value::as_u64);
        Self {
            input: count("inputTokens"),
            output: count("outputTokens"),
            cache_read: count("cacheReadInputTokens"),
            cache_write: count("cacheCreationInputTokens"),
            cache_write_1h: count("cacheCreationEphemeral1hInputTokens"),
            cache_write_5m: count("cacheCreationEphemeral5mInputTokens"),
            thought: count("thinkingTokens"),
        }
    }

    fn fields(&self) -> [&Option<u64>; 7] {
        [
            &self.input,
            &self.output,
            &self.cache_read,
            &self.cache_write,
            &self.cache_write_1h,
            &self.cache_write_5m,
            &self.thought,
        ]
    }

    /// The counts added: a count only one side reported is kept (a lower bound), and one neither did stays `None`.
    fn plus(&self, other: &Self) -> Self {
        let add = |a: Option<u64>, b: Option<u64>| match (a, b) {
            (Some(a), Some(b)) => Some(a.saturating_add(b)),
            (a, b) => a.or(b),
        };
        Self {
            input: add(self.input, other.input),
            output: add(self.output, other.output),
            cache_read: add(self.cache_read, other.cache_read),
            cache_write: add(self.cache_write, other.cache_write),
            cache_write_1h: add(self.cache_write_1h, other.cache_write_1h),
            cache_write_5m: add(self.cache_write_5m, other.cache_write_5m),
            thought: add(self.thought, other.thought),
        }
    }

    /// What `self` counts beyond `own`: a count is known only when both sides reported it.
    fn beyond(&self, own: &Self) -> Self {
        let take = |all: Option<u64>, own: Option<u64>| Some(all?.saturating_sub(own?));
        Self {
            input: take(self.input, own.input),
            output: take(self.output, own.output),
            cache_read: take(self.cache_read, own.cache_read),
            cache_write: take(self.cache_write, own.cache_write),
            cache_write_1h: take(self.cache_write_1h, own.cache_write_1h),
            cache_write_5m: take(self.cache_write_5m, own.cache_write_5m),
            thought: take(self.thought, own.thought),
        }
    }

    fn has_any(&self) -> bool {
        self.fields()
            .iter()
            .any(|count| matches!(count, Some(n) if *n > 0))
    }

    /// Input, output and the cache reads and writes are all reported, and all 0.
    fn is_all_zero(&self) -> bool {
        [self.input, self.output, self.cache_read, self.cache_write] == [Some(0); 4]
    }

    /// The tokens in the context of a response: input, cache reads and cache writes, when all three are reported.
    fn context(&self) -> Option<u64> {
        Some(self.input? + self.cache_read? + self.cache_write?)
    }

    /// `Exact` when input, output and both cache counts are reported, `Unreported` when none is, else `Partial`.
    fn quality(&self) -> UsageQuality {
        let reported = [self.input, self.output, self.cache_read, self.cache_write]
            .iter()
            .filter(|count| count.is_some())
            .count();
        match reported {
            4 => UsageQuality::Exact,
            0 => UsageQuality::Unreported,
            _ => UsageQuality::Partial,
        }
    }

    /// The counts as a `Usage` of `scope`, with no cost, time or context.
    fn into_usage(self, scope: UsageScope, quality: UsageQuality) -> Usage {
        Usage {
            scope,
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
            cache_write_1h: self.cache_write_1h,
            cache_write_5m: self.cache_write_5m,
            thought: self.thought,
            context_used: None,
            context_size: None,
            cost: None,
            model_time_ms: None,
            quality,
        }
    }
}

/// A model response seen in the turn in flight, by its `message.id`. The stream repeats one `usage` per content block, so
/// a response is kept once. `tokens` has no output, because a stream row's count is partial (S9); `context` is `None` for
/// a subagent's response, which is not the context of the session.
#[derive(Debug, Clone, PartialEq)]
struct Response {
    id: String,
    tokens: Tokens,
    context: Option<u64>,
}

/// One Claude child's conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeSession {
    start: SessionStart,
    pending: Vec<Pending>,
    cancel_requested: bool,
    interrupts_written: u64,
    /// Whether the turn in flight has given a message or a tool call.
    gave_output: bool,
    /// How the last turn ended; `None` until one does, and again once a new turn is sent.
    last_turn: Option<TurnEndReason>,
    /// The tool calls of the turn in flight that have no result yet.
    calls: Vec<CallInFlight>,
    /// The tool hooks of the turn in flight that failed, and whose call has no result yet.
    failed_hooks: Vec<FailedHook>,
    /// A hook blocked the prompt of the turn in flight: `Some(reason)`, the reason being the hook's own, if it gave one.
    blocked: Option<Option<String>>,
    /// A turn has been sent and has no result yet.
    turn_open: bool,
    /// The responses of the turn in flight, each once.
    responses: Vec<Response>,
    /// The model of the last response of the session itself, whose window is the context size.
    last_model: Option<String>,
    /// A `compact_boundary` came in the turn in flight.
    compacted: bool,
    /// A limit signal came in the turn in flight: `Some(reset time in ms, if a line gave one)`. One limit is one
    /// `Limited`, given at the turn's `result` (or at `finish`), so the first reset time any of its lines has is in it.
    limit: Option<Option<u64>>,
    /// The session's running cost as the last result reported it, and `duration_api_ms` as the last result with turns
    /// did. Both are differenced to give a turn's own. `None` when unknown: a process that resumes a session continues
    /// its total (S1), so its first result has nothing to be differenced from; and a result that lacked the value
    /// leaves nothing for the next to be differenced from.
    cost_anchor: Option<f64>,
    time_anchor: Option<u64>,
    /// The phase's settings carry the canary hook, so a turn that ends without its `hook_response` having been read is a
    /// phase that ran without Collide's settings. Off for a session that is not a phase's (the smoke check).
    expect_canary: bool,
    /// The canary hook's `hook_response` has been read.
    canary_seen: bool,
}

impl ClaudeSession {
    /// A session that starts as `start` says: Claude supports `Assign` (a UUID Collide picked) and `Resume`.
    pub fn new(start: SessionStart) -> Self {
        // A session Collide assigns starts every total at 0; a resumed one continues the earlier process's.
        let fresh = matches!(start, SessionStart::Assign { .. });
        Self {
            start,
            pending: Vec::new(),
            cancel_requested: false,
            interrupts_written: 0,
            gave_output: false,
            last_turn: None,
            calls: Vec::new(),
            failed_hooks: Vec::new(),
            blocked: None,
            turn_open: false,
            responses: Vec::new(),
            last_model: None,
            compacted: false,
            limit: None,
            cost_anchor: fresh.then_some(0.0),
            time_anchor: fresh.then_some(0),
            expect_canary: false,
            canary_seen: false,
        }
    }

    /// A session of a phase whose snapshot settings carry the canary hook ([`super::version::CANARY_MARKER`]): a turn that
    /// ends without the hook having answered is [`PhaseFailure::SettingsIgnored`].
    pub fn expecting_canary(mut self) -> Self {
        self.expect_canary = true;
        self
    }

    /// How the session started.
    pub fn session_start(&self) -> &SessionStart {
        &self.start
    }

    /// Learns a message as a replay would, then returns it as the bytes for the child.
    fn write(&mut self, message: Value) -> Vec<u8> {
        let bytes = json_line(&message);
        self.note_written(&bytes)
            .expect("a session reads its own writes");
        bytes
    }

    fn read_assistant(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        if value.get("error").and_then(Value::as_str) == Some(RATE_LIMIT_ERROR) {
            self.note_limit(value.pointer("/quotaLimits/resetsAt"));
        }
        let Some(blocks) = value.pointer("/message/content").and_then(Value::as_array) else {
            return vec![other("assistant", wall_ns)];
        };
        let mut events = Vec::new();
        if let Some(step) = self.note_response(value) {
            events.push(stamped(Some(wall_ns), step));
        }
        for block in blocks {
            let event = match block.get("type").and_then(Value::as_str) {
                Some("text") => match block.get("text").and_then(Value::as_str) {
                    Some(text) if !text.is_empty() => {
                        self.gave_output = true;
                        AgentEvent::Message { text: text.into() }
                    }
                    _ => continue,
                },
                Some("tool_use") => match tool_call(block) {
                    Some((event, call)) => {
                        self.gave_output = true;
                        self.calls.push(call);
                        event
                    }
                    None => other_event("assistant/tool_use"),
                },
                Some("thinking" | "redacted_thinking") => continue,
                Some(kind) => other_event(&format!("assistant/{kind}")),
                None => other_event("assistant"),
            };
            events.push(stamped(Some(wall_ns), event));
        }
        events
    }

    /// A response the session has not seen before: the `Step` usage of its input and cache tokens. A line with no
    /// `message.id` or no `usage` cannot be counted once, so it is not counted.
    fn note_response(&mut self, line: &Value) -> Option<AgentEvent> {
        let message = line.get("message")?;
        let id = message.get("id").and_then(Value::as_str)?;
        let usage = message.get("usage").filter(|usage| usage.is_object())?;
        if self.responses.iter().any(|response| response.id == id) {
            return None;
        }
        let of_the_session = line.get("parent_tool_use_id").is_none_or(Value::is_null);
        if of_the_session && let Some(model) = message.get("model").and_then(Value::as_str) {
            self.last_model = Some(model.into());
        }
        let tokens = Tokens {
            output: None,
            thought: None,
            ..Tokens::from_usage(usage)
        };
        let context = tokens.context().filter(|_| of_the_session);
        let mut step = tokens
            .clone()
            .into_usage(UsageScope::Step, UsageQuality::Partial);
        step.context_used = context;
        self.responses.push(Response {
            id: id.into(),
            tokens,
            context,
        });
        Some(AgentEvent::Usage { usage: step })
    }

    /// One limit signal. The first reset time any line of the limit has is kept, so a later line fills in a reset time
    /// an earlier one lacked, and a second line never gives a second limit. `resets_at` is epoch seconds.
    fn note_limit(&mut self, resets_at: Option<&Value>) {
        let reset_at_ms = resets_at
            .and_then(Value::as_u64)
            .and_then(|seconds| seconds.checked_mul(1000));
        match &mut self.limit {
            Some(known) => {
                if known.is_none() {
                    *known = reset_at_ms;
                }
            }
            None => self.limit = Some(reset_at_ms),
        }
    }

    fn read_rate_limit(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let info = value.get("rate_limit_info");
        match info
            .and_then(|info| info.get("status"))
            .and_then(Value::as_str)
        {
            // The limit is given once, at the turn's result (`note_limit`).
            Some("rejected") => {
                self.note_limit(info.and_then(|info| info.get("resetsAt")));
                Vec::new()
            }
            // An early warning is recorded and pauses nothing.
            Some("allowed_warning") => vec![other("rate_limit_event/allowed_warning", wall_ns)],
            _ => vec![other("rate_limit_event", wall_ns)],
        }
    }

    fn read_control_request(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let request = value.get("request");
        let subtype = request
            .and_then(|r| r.get("subtype"))
            .and_then(Value::as_str);
        let id = value.get("request_id").and_then(Value::as_str);
        let (Some("can_use_tool"), Some(id), Some(request)) = (subtype, id, request) else {
            let name = match subtype {
                Some(subtype) => format!("control_request/{subtype}"),
                None => "control_request".to_string(),
            };
            return vec![other(&name, wall_ns)];
        };
        let request = permission_request(id, request);
        self.pending.retain(|pending| pending.id != request.id);
        self.pending.push(Pending {
            id: request.id.clone(),
            input: request.input.clone(),
            request: request.clone(),
        });
        vec![stamped(
            Some(wall_ns),
            AgentEvent::PermissionRequest { request },
        )]
    }

    fn read_system(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        match subtype(value) {
            // The id was bound at `start`; `init` comes again on every turn.
            Some("init") => Vec::new(),
            // `hook_response` has the name, the outcome and the exit code that `hook_started` lacks.
            Some("hook_started") => Vec::new(),
            Some("hook_response") => self.read_hook_response(value, wall_ns),
            Some("informational") => self.read_informational(value, wall_ns),
            Some("compact_boundary") => {
                // The compaction's own model call is in this turn's `model_usage_since_last_result`, not in its usage.
                self.compacted = true;
                vec![self.read_compact_boundary(value, wall_ns)]
            }
            _ => vec![other(&type_name("system", value), wall_ns)],
        }
    }

    /// The `compact_metadata` of the line, where it gives one. `post_tokens` is left out on purpose: the size after is
    /// the context the next response reports. A figure the line lacks is `None`. A compaction is mid-task when a tool
    /// call has no result yet.
    fn read_compact_boundary(&self, value: &Value, wall_ns: u64) -> StampedEvent {
        let metadata = value.get("compact_metadata");
        let number = |key| metadata.and_then(|m| m.get(key)).and_then(Value::as_u64);
        let trigger = match metadata
            .and_then(|m| m.get("trigger"))
            .and_then(Value::as_str)
        {
            Some("manual") => Some(CompactionTrigger::Manual),
            Some("auto") => Some(CompactionTrigger::Auto),
            _ => None,
        };
        stamped(
            Some(wall_ns),
            AgentEvent::Compaction {
                trigger,
                context_before: number("pre_tokens"),
                duration_ms: number("duration_ms"),
                mid_task: !self.calls.is_empty(),
            },
        )
    }

    fn read_hook_response(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let text = |key| value.get(key).and_then(Value::as_str);
        let outcome = match text("outcome") {
            Some("success") => Some(HookOutcome::Success),
            Some("error") => Some(HookOutcome::Error),
            Some("cancelled") => Some(HookOutcome::Cancelled),
            _ => None,
        };
        let (Some(name), Some(event), Some(outcome)) =
            (text("hook_name"), text("hook_event"), outcome)
        else {
            return vec![other("system/hook_response", wall_ns)];
        };
        let exit_code = value
            .get("exit_code")
            .and_then(Value::as_i64)
            .and_then(|code| i32::try_from(code).ok());
        if super::version::canary_in(value) {
            self.canary_seen = true;
        }
        if let Some(cause) = hook_failure(outcome, exit_code) {
            self.note_failed_hook(name, event, cause, None);
        }
        // `stdout` is read for its decision and dropped; `stderr` and `output` are never read.
        let decision = text("stdout").and_then(hook_decision);
        vec![stamped(
            Some(wall_ns),
            AgentEvent::HookCall {
                name: name.into(),
                event: event.into(),
                outcome,
                exit_code,
                decision,
            },
        )]
    }

    /// `system/informational` is a prompt a hook blocked, a hook that gave no verdict, or something else.
    fn read_informational(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let blocked = value.get("prompt_submit_outcome");
        let kind = blocked
            .and_then(|outcome| outcome.get("kind"))
            .and_then(Value::as_str);
        if matches!(kind, Some("blocked_by_hook" | "stopped_by_hook")) {
            self.blocked = Some(
                blocked
                    .and_then(|outcome| outcome.get("reason"))
                    .and_then(Value::as_str)
                    .map(String::from),
            );
            return Vec::new();
        }
        if value.get("tag").and_then(Value::as_str) == Some("hook_no_verdict") {
            return self.read_no_verdict(value, wall_ns);
        }
        vec![other(&type_name("system", value), wall_ns)]
    }

    /// With `--include-hook-events` the timed-out hook has its own `hook_response` (`cancelled`) before this line, so
    /// the line only says which call it was about. Without the flag it is all there is, and it names the event and the
    /// call but not the hook.
    fn read_no_verdict(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let info = value.get("hook_no_verdict");
        let Some(event) = info
            .and_then(|info| info.get("hook_event"))
            .and_then(Value::as_str)
        else {
            return vec![other("system/informational", wall_ns)];
        };
        let call_id = info
            .and_then(|info| info.get("tool_use_id"))
            .and_then(Value::as_str);
        let known = self.failed_hooks.iter_mut().find(|failed| {
            failed.cause == HookFailure::NoVerdict
                && failed.event == event
                && failed.call_id.is_none()
        });
        if let Some(failed) = known {
            failed.call_id = call_id.map(String::from);
            return Vec::new();
        }
        self.note_failed_hook(event, event, HookFailure::NoVerdict, call_id);
        vec![stamped(
            Some(wall_ns),
            AgentEvent::HookCall {
                name: event.into(),
                event: event.into(),
                outcome: HookOutcome::Cancelled,
                exit_code: None,
                decision: None,
            },
        )]
    }

    /// Keeps a failed hook of a tool event until the call's result says whether the tool ran. Any other hook has no
    /// call to have failed open on.
    fn note_failed_hook(
        &mut self,
        name: &str,
        event: &str,
        cause: HookFailure,
        call_id: Option<&str>,
    ) {
        if matches!(event, "PreToolUse" | "PostToolUse") {
            self.failed_hooks.push(FailedHook {
                name: name.into(),
                event: event.into(),
                cause,
                call_id: call_id.map(String::from),
            });
        }
    }

    /// The `user` line that holds tool results: who refused a call that did not run, and whether a hook that failed
    /// left the call to run.
    fn read_user(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let results: Vec<&Value> = value
            .pointer("/message/content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|block| {
                        block.get("type").and_then(Value::as_str) == Some("tool_result")
                    })
                    .collect()
            })
            .unwrap_or_default();
        if results.is_empty() {
            return vec![other("user", wall_ns)];
        }
        let metas = value.get("tool_result_meta").and_then(Value::as_array);
        let mut events = Vec::new();
        for result in results {
            let Some(id) = result.get("tool_use_id").and_then(Value::as_str) else {
                continue;
            };
            let is_error = result
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let meta = metas.and_then(|metas| {
                metas
                    .iter()
                    .find(|meta| meta.get("id").and_then(Value::as_str) == Some(id))
            });
            if let Some(bytes) = result_bytes(result) {
                events.push(AgentEvent::ToolResultSize {
                    tool_call_id: id.into(),
                    bytes,
                });
            }
            let refusal = refusal_cause(meta, is_error);
            if let Some(cause) = refusal {
                events.push(AgentEvent::ToolRefused {
                    id: id.into(),
                    cause,
                });
            }
            events.extend(self.settle_hooks(id, refusal.is_none()));
        }
        events
            .into_iter()
            .map(|event| stamped(Some(wall_ns), event))
            .collect()
    }

    /// The call `id` has its result: each failed hook that belongs to it has failed open if the tool ran.
    fn settle_hooks(&mut self, id: &str, ran: bool) -> Vec<AgentEvent> {
        let tool = self
            .calls
            .iter()
            .position(|call| call.id == id)
            .and_then(|at| self.calls.remove(at).tool);
        let (mine, rest): (Vec<FailedHook>, Vec<FailedHook>) =
            std::mem::take(&mut self.failed_hooks)
                .into_iter()
                .partition(|failed| match &failed.call_id {
                    Some(call_id) => call_id == id,
                    None => tool.is_some() && tool.as_deref() == tool_of_hook(&failed.name),
                });
        self.failed_hooks = rest;
        if !ran {
            return Vec::new();
        }
        mine.into_iter()
            .map(|failed| AgentEvent::HookFailedOpen {
                name: failed.name,
                event: failed.event,
                tool_call_id: id.into(),
                cause: failed.cause,
            })
            .collect()
    }

    /// What the turn in flight remembered about its calls, its hooks, a block, its responses and a limit.
    fn forget_the_turn(&mut self) {
        self.calls.clear();
        self.failed_hooks.clear();
        self.blocked = None;
        self.responses.clear();
        self.compacted = false;
        self.limit = None;
    }

    /// The `Usage` events of a result, from the turn's own to the session's:
    /// - a compaction call, or any other model call, as what `model_usage_since_last_result` counts beyond `usage`
    ///   (S9: the compaction is in the first and not in the second), so the turn's own tokens never hold it;
    /// - the turn's: tokens from `usage` (the stream's output counts are partial), its cost and model time as
    ///   differences of the running totals, the context of its last response;
    /// - the session's running cost, as it was reported.
    fn usage_events(&mut self, result: &Value) -> Vec<AgentEvent> {
        let own = result
            .get("usage")
            .filter(|usage| usage.is_object())
            .map(Tokens::from_usage);
        let models = result
            .get("model_usage_since_last_result")
            .and_then(Value::as_object);
        let total = result.get("total_cost_usd").and_then(Value::as_f64);
        let has_turns = result.get("num_turns").and_then(Value::as_u64) != Some(0);
        let spend = self.spend(total, models);
        let model_time_ms = self.model_time(
            result.get("duration_api_ms").and_then(Value::as_u64),
            has_turns,
        );
        let mut events = Vec::new();
        let mut turn_cost = spend;
        if let (Some(own), Some(models)) = (&own, models) {
            let all = models
                .values()
                .map(Tokens::from_model_usage)
                .fold(Tokens::default(), |sum, next| sum.plus(&next));
            let extra = all.beyond(own);
            if extra.has_any() {
                let compaction = self.compacted
                    || result.get("local_command").and_then(Value::as_str) == Some("compact");
                let scope = if compaction {
                    UsageScope::Compaction
                } else {
                    UsageScope::OtherCalls
                };
                let quality = extra.quality();
                let mut usage = extra.into_usage(scope, quality);
                // The spend is the turn's and the call's together. It is the call's alone only when the turn used no
                // tokens of its own (a `/compact`); otherwise it stays in the turn's, and is not given twice.
                if own.is_all_zero() {
                    usage.cost = spend.map(usd);
                    turn_cost = spend.map(|_| 0.0);
                }
                events.push(AgentEvent::Usage { usage });
            }
        }
        let tokens = match own {
            Some(own) => own,
            None => self.responses_total(),
        };
        let quality = tokens.quality();
        let mut turn = tokens.into_usage(UsageScope::Turn, quality);
        turn.cost = turn_cost.map(usd);
        turn.model_time_ms = model_time_ms;
        turn.context_used = self
            .responses
            .iter()
            .rev()
            .find_map(|response| response.context);
        turn.context_size = self.context_size(models);
        events.push(AgentEvent::Usage { usage: turn });
        if let Some(amount) = total {
            let mut running =
                Tokens::default().into_usage(UsageScope::RunningTotal, UsageQuality::Exact);
            running.cost = Some(usd(amount));
            events.push(AgentEvent::Usage { usage: running });
        }
        events
    }

    /// The input and cache tokens of the turn's responses, each counted once. Nothing else is known of them.
    fn responses_total(&self) -> Tokens {
        self.responses
            .iter()
            .fold(Tokens::default(), |sum, response| {
                sum.plus(&response.tokens)
            })
    }

    /// The window of the model that answered, from `model_usage_since_last_result`.
    fn context_size(&self, models: Option<&Map<String, Value>>) -> Option<u64> {
        let entry = models?.get(self.last_model.as_deref()?)?;
        entry.get("contextWindow").and_then(Value::as_u64)
    }

    /// A turn's own cost: the running total less the one before it. With no total before it (a resumed process'
    /// first result), or a total that went backwards, the turn's own figure from `model_usage_since_last_result`.
    fn spend(&mut self, total: Option<f64>, models: Option<&Map<String, Value>>) -> Option<f64> {
        let reported = models.and_then(|models| {
            let costs: Vec<f64> = models
                .values()
                .filter_map(|entry| entry.get("costUSD").and_then(Value::as_f64))
                .collect();
            (!costs.is_empty()).then(|| costs.iter().sum())
        });
        let spend = match (total, self.cost_anchor) {
            (Some(total), Some(before)) if total >= before => Some(total - before),
            _ => reported,
        };
        self.cost_anchor = total;
        spend
    }

    /// A turn's model time: `duration_api_ms` less the one of the last result with turns. A result with no turns (a
    /// `/compact`, a blocked prompt) reports 0, so it has no model time and is not what the next is taken from. A
    /// result that lacks the value leaves no total for the next to be taken from.
    fn model_time(&mut self, duration: Option<u64>, has_turns: bool) -> Option<u64> {
        if !has_turns {
            return None;
        }
        let time = match (duration, self.time_anchor) {
            (Some(now), Some(before)) if now >= before => Some(now - before),
            _ => None,
        };
        self.time_anchor = duration;
        time
    }

    /// What a turn that was sent and never ended is known to have used: its responses' input and cache tokens.
    fn cut_usage(&self) -> AgentEvent {
        let tokens = self.responses_total();
        let quality = tokens.quality();
        let mut usage = tokens.into_usage(UsageScope::Turn, quality);
        usage.context_used = self
            .responses
            .iter()
            .rev()
            .find_map(|response| response.context);
        AgentEvent::Usage { usage }
    }

    fn read_result(&mut self, value: &Value, wall_ns: u64) -> Vec<StampedEvent> {
        let failed = is_failed_result(value);
        let blocked = self.blocked.take();
        let reason = if self.cancel_requested {
            TurnEndReason::Cancelled {}
        } else if let Some(reason) = &blocked {
            TurnEndReason::Blocked {
                reason: reason.clone(),
            }
        } else {
            turn_end_reason(value, failed)
        };
        let mut events = Vec::new();
        // An error result and a blocked prompt's result are never the phase's report: the first is an API failure and
        // the second repeats the hook's message and the prompt (S1, S9).
        let report = value
            .get("result")
            .and_then(Value::as_str)
            .filter(|text| !failed && blocked.is_none() && !text.is_empty());
        if let Some(text) = report {
            events.push(AgentEvent::FinalReport {
                text: text.into(),
                source: FinalReportSource::ResultField,
            });
        }
        events.extend(self.usage_events(value));
        if failed
            && value.get("api_error_status").and_then(Value::as_u64) == Some(USAGE_LIMIT_STATUS)
        {
            self.note_limit(None);
        }
        if let Some(reset_at_ms) = self.limit.take() {
            events.push(AgentEvent::Limited { reset_at_ms });
        }
        events.push(AgentEvent::TurnEnd {
            reason: reason.clone(),
            cancel_requested: self.cancel_requested,
            empty: !self.gave_output && usage_tokens(value) == 0,
        });
        self.gave_output = false;
        self.turn_open = false;
        self.last_turn = Some(reason);
        self.forget_the_turn();
        events
            .into_iter()
            .map(|event| stamped(Some(wall_ns), event))
            .collect()
    }
}

impl AgentSession for ClaudeSession {
    fn start(&mut self) -> Step {
        let id = match &self.start {
            SessionStart::Assign { uuid } => Some(uuid.clone()),
            SessionStart::Resume { id } => Some(id.clone()),
            // Claude cannot name its own session; launch refuses this start, so there is nothing to bind.
            SessionStart::AgentAssigned {} => None,
        };
        Step {
            events: id
                .map(|id| stamped(None, AgentEvent::SessionBound { id }))
                .into_iter()
                .collect(),
            writes: Vec::new(),
        }
    }

    fn feed(&mut self, line: &[u8], wall_ns: u64) -> Step {
        self.read_line(line, wall_ns)
    }

    fn send_turn(&mut self, text: &str) -> Step {
        let write = self.write(json!({
            "type": "user",
            "message": { "role": "user", "content": [{ "type": "text", "text": text }] },
            "parent_tool_use_id": null,
            "session_id": "",
        }));
        written(write)
    }

    fn answer(&mut self, request_id: &str, reply: PermissionReply) -> Step {
        let Some(pending) = self.pending.iter().find(|pending| pending.id == request_id) else {
            return Step::default();
        };
        let response = match reply {
            PermissionReply::Allow {} => {
                let mut allow = json!({ "behavior": "allow" });
                // An allow with an input replaces the call's input, so one that was never known is left out,
                // never sent as an empty object.
                if let Some(input) = &pending.input {
                    allow["updatedInput"] = input.clone();
                }
                allow
            }
            PermissionReply::Deny { message } => json!({
                "behavior": "deny",
                "message": message.as_deref().unwrap_or(DENY_WITHOUT_MESSAGE),
            }),
            PermissionReply::Cancel {} => {
                json!({ "behavior": "deny", "message": CANCELLED_REQUEST })
            }
        };
        let write = self.write(json!({
            "type": "control_response",
            "response": { "subtype": "success", "request_id": request_id, "response": response },
        }));
        written(write)
    }

    fn pending_permissions(&self) -> Vec<PermissionRequest> {
        self.pending
            .iter()
            .map(|pending| pending.request.clone())
            .collect()
    }

    fn cancel(&mut self) -> Step {
        let write = self.write(json!({
            "type": "control_request",
            "request_id": format!("collide-interrupt-{}", self.interrupts_written + 1),
            "request": { "subtype": "interrupt" },
        }));
        written(write)
    }

    fn finish(&mut self, exit: &ExitInfo) -> Finished {
        let outcome = match (&exit.record, &self.last_turn) {
            (None, _) => PhaseOutcome::Lost,
            (Some(_), None) => PhaseOutcome::Failed {
                cause: PhaseFailure::NoResult,
            },
            // The canary hook runs at session start, before the first turn: a turn that ended without it says the
            // settings were not read. A child that never ended a turn is `NoResult`, whatever the cause.
            (Some(_), Some(_)) if self.expect_canary && !self.canary_seen => PhaseOutcome::Failed {
                cause: PhaseFailure::SettingsIgnored,
            },
            (Some(_), Some(reason)) => outcome_of_turn(reason),
        };
        // What a turn that never ended is known to have used, and a limit no result followed.
        let mut events = Vec::new();
        if self.turn_open {
            events.push(self.cut_usage());
        }
        if let Some(reset_at_ms) = self.limit.take() {
            events.push(AgentEvent::Limited { reset_at_ms });
        }
        Finished {
            step: Step {
                events: events
                    .into_iter()
                    .map(|event| stamped(None, event))
                    .collect(),
                writes: Vec::new(),
            },
            outcome,
        }
    }

    fn replay(&mut self, records: &[JournalRecord]) -> Result<Step, ReplayError> {
        replay_records(self, records)
    }
}

impl Replayable for ClaudeSession {
    fn note_written(&mut self, bytes: &[u8]) -> Result<(), String> {
        let message: Value = serde_json::from_slice(bytes)
            .map_err(|reason| format!("not a JSON message: {reason}"))?;
        match message.get("type").and_then(Value::as_str) {
            // A new turn is a new thing to cancel, and has not ended.
            Some("user") => {
                self.cancel_requested = false;
                self.gave_output = false;
                self.turn_open = true;
                self.last_turn = None;
                self.forget_the_turn();
            }
            Some("control_response") => {
                let id = message
                    .pointer("/response/request_id")
                    .and_then(Value::as_str)
                    .ok_or("a control response with no `request_id`")?;
                self.pending.retain(|pending| pending.id != id);
            }
            Some("control_request")
                if message.pointer("/request/subtype").and_then(Value::as_str)
                    == Some("interrupt") =>
            {
                self.cancel_requested = true;
                self.interrupts_written += 1;
            }
            other => return Err(format!("not a message this session writes: type {other:?}")),
        }
        Ok(())
    }

    fn read_line(&mut self, line: &[u8], wall_ns: u64) -> Step {
        let events = match serde_json::from_slice::<Value>(line) {
            Ok(value) => match value.get("type").and_then(Value::as_str) {
                Some("assistant") => self.read_assistant(&value, wall_ns),
                Some("control_request") => self.read_control_request(&value, wall_ns),
                Some("result") => self.read_result(&value, wall_ns),
                Some("rate_limit_event") => self.read_rate_limit(&value, wall_ns),
                Some("system") => self.read_system(&value, wall_ns),
                Some("user") => self.read_user(&value, wall_ns),
                Some(kind) => vec![other(&type_name(kind, &value), wall_ns)],
                None => vec![other("unparsed_line", wall_ns)],
            },
            Err(_) => vec![other("unparsed_line", wall_ns)],
        };
        Step {
            events,
            writes: Vec::new(),
        }
    }
}

fn json_line(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("a JSON value serializes");
    bytes.push(b'\n');
    bytes
}

fn written(bytes: Vec<u8>) -> Step {
    Step {
        events: Vec::new(),
        writes: vec![bytes],
    }
}

fn stamped(wall_ns: Option<u64>, event: AgentEvent) -> StampedEvent {
    StampedEvent { wall_ns, event }
}

/// An `Other` that keeps no content: the log must not hold a hook's output or a prompt that the stream carried, and the
/// journal has the line whole.
fn other_event(raw_type: &str) -> AgentEvent {
    AgentEvent::Other {
        raw_type: raw_type.into(),
        raw: None,
    }
}

fn other(raw_type: &str, wall_ns: u64) -> StampedEvent {
    stamped(Some(wall_ns), other_event(raw_type))
}

fn subtype(value: &Value) -> Option<&str> {
    value.get("subtype").and_then(Value::as_str)
}

/// `type`, and `type/subtype` for a line that has a subtype (`system/status`).
fn type_name(kind: &str, value: &Value) -> String {
    match subtype(value) {
        Some(subtype) => format!("{kind}/{subtype}"),
        None => kind.to_string(),
    }
}

/// What a tool call does, by the tool's name: Claude's own tools are named for what they do, and an MCP tool or one
/// this version does not know is `Other`.
fn tool_kind(name: &str) -> ToolKind {
    match name {
        "Read" => ToolKind::Read,
        "Grep" | "Glob" => ToolKind::Search,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => ToolKind::Edit,
        "Bash" => ToolKind::Execute,
        "WebFetch" | "WebSearch" => ToolKind::Fetch,
        _ => ToolKind::Other,
    }
}

/// The event for a `tool_use` block, and the call as the session keeps it until its result.
fn tool_call(block: &Value) -> Option<(AgentEvent, CallInFlight)> {
    let id = block.get("id").and_then(Value::as_str)?;
    let tool = block.get("name").and_then(Value::as_str);
    let event = AgentEvent::ToolCall {
        id: id.into(),
        tool: tool.map(String::from),
        kind: tool.map_or(ToolKind::Other, tool_kind),
        input: block.get("input").filter(|input| !input.is_null()).cloned(),
    };
    let call = CallInFlight {
        id: id.into(),
        tool: tool.map(String::from),
    };
    Some((event, call))
}

/// The request as the host sees it. `input` is `None` when the request has none. `reason` is the hook's own text for
/// a hook that asked (S9); the agent's own options are none, so no answer offers an "always" (0.3, Permissions).
fn permission_request(id: &str, request: &Value) -> PermissionRequest {
    let tool = request.get("tool_name").and_then(Value::as_str);
    PermissionRequest {
        id: id.into(),
        tool_call_id: request
            .get("tool_use_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        tool: tool.map(String::from),
        kind: tool.map_or(ToolKind::Other, tool_kind),
        input: request
            .get("input")
            .filter(|input| !input.is_null())
            .cloned(),
        reason: request
            .get("decision_reason")
            .and_then(Value::as_str)
            .map(String::from),
        options: Vec::new(),
    }
}

/// Who refused a tool call, from the `user` event's metadata for it (`meta`, the `tool_result_meta` entry with the
/// call's id), never from the result's text (S9). `None` is a call that ran.
/// - `non_execution_kind`, or a `reject` decision, says the call did not run, and `permission_decision.source` says who
///   refused it: `hook`, `user_reject` (Collide's own deny) or `config` (a rule, or an auto-deny); any other source is
///   `Unknown`;
/// - an error with no metadata at all is a call to a tool the session does not have.
fn refusal_cause(meta: Option<&Value>, is_error: bool) -> Option<RefusalCause> {
    let Some(meta) = meta else {
        return is_error.then_some(RefusalCause::MissingTool);
    };
    let decision = meta.get("permission_decision");
    let rejected = decision
        .and_then(|decision| decision.get("decision"))
        .and_then(Value::as_str)
        == Some("reject");
    let did_not_run = meta
        .get("non_execution_kind")
        .is_some_and(|kind| !kind.is_null());
    if !rejected && !did_not_run {
        return None;
    }
    let source = decision
        .and_then(|decision| decision.get("source"))
        .and_then(Value::as_str);
    Some(match source {
        Some("hook") => RefusalCause::Hook,
        Some("user_reject") => RefusalCause::Host,
        Some("config") => RefusalCause::Rule,
        _ => RefusalCause::Unknown,
    })
}

/// How a hook that was meant to guard a call failed without stopping it, if it did (0.13). Exit 2 is the deliberate
/// block, and 0 is a pass; a timeout is `cancelled`; any other exit code is a failure that lets the call go on (S9
/// measured exit 1).
fn hook_failure(outcome: HookOutcome, exit_code: Option<i32>) -> Option<HookFailure> {
    match outcome {
        HookOutcome::Success => None,
        HookOutcome::Cancelled => Some(HookFailure::NoVerdict),
        HookOutcome::Error if exit_code == Some(HOOK_BLOCK_EXIT_CODE) => None,
        HookOutcome::Error => Some(HookFailure::ExitError),
    }
}

/// The tool a hook is for, from its name: `PreToolUse:Bash` is for `Bash`.
fn tool_of_hook(name: &str) -> Option<&str> {
    name.split_once(':').map(|(_, tool)| tool)
}

/// The `decision` a hook printed on stdout, as Claude's two JSON forms write it: `hookSpecificOutput.permissionDecision`
/// (`allow`, `deny`, `ask`) and the top-level `decision` (`approve`, `block`). Output that is not JSON, or says
/// something else, has none.
fn hook_decision(stdout: &str) -> Option<HookDecision> {
    let output: Value = serde_json::from_str(stdout.trim()).ok()?;
    let word = output
        .pointer("/hookSpecificOutput/permissionDecision")
        .or_else(|| output.get("decision"))?
        .as_str()?;
    match word {
        "allow" | "approve" => Some(HookDecision::Allow),
        "deny" => Some(HookDecision::Deny),
        "ask" => Some(HookDecision::Ask),
        "block" => Some(HookDecision::Block),
        _ => None,
    }
}

fn usd(amount: f64) -> Money {
    Money {
        amount,
        currency: "USD".into(),
    }
}

/// The size in bytes of a tool result's content as it enters the context: a string's length, or the sum over its blocks of
/// a text block's text and any other block's JSON. `None` when the result has no content.
fn result_bytes(result: &Value) -> Option<u64> {
    match result.get("content")? {
        Value::String(text) => Some(text.len() as u64),
        Value::Array(blocks) => Some(blocks.iter().map(block_bytes).sum()),
        _ => None,
    }
}

fn block_bytes(block: &Value) -> u64 {
    let text = match block.get("type").and_then(Value::as_str) {
        Some("text") => block.get("text").and_then(Value::as_str),
        _ => None,
    };
    match text {
        Some(text) => text.len() as u64,
        None => block.to_string().len() as u64,
    }
}

/// A result that is an error: `is_error`, or a subtype other than `success`. Its `result` is never the phase's report
/// (S1: a `result/success` with `is_error` is an API failure).
fn is_failed_result(result: &Value) -> bool {
    let error = result
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    error || subtype(result).is_some_and(|subtype| subtype != "success")
}

/// The agent's own reason for the turn's end, from the result alone. A blocked prompt is known from the line before
/// the result, and a usage limit from the status, so they are read in `read_result`.
fn turn_end_reason(result: &Value, failed: bool) -> TurnEndReason {
    if failed {
        return match subtype(result) {
            Some("error_max_turns") => TurnEndReason::MaxTurns {},
            Some("error_max_budget_usd") => TurnEndReason::Budget {},
            _ => TurnEndReason::Error {
                status: result
                    .get("api_error_status")
                    .and_then(Value::as_u64)
                    .and_then(|status| u16::try_from(status).ok()),
            },
        };
    }
    match result.get("stop_reason").and_then(Value::as_str) {
        Some("max_tokens") => TurnEndReason::MaxTokens {},
        Some("refusal") => TurnEndReason::Refusal {},
        _ => TurnEndReason::EndTurn {},
    }
}

/// Every token `result.usage` counts. A result with no `usage`, or with all of it 0, has none.
fn usage_tokens(result: &Value) -> u64 {
    [
        "input_tokens",
        "output_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ]
    .iter()
    .filter_map(|key| {
        result
            .pointer(&format!("/usage/{key}"))
            .and_then(Value::as_u64)
    })
    .sum()
}

/// What a phase came to, from how its last turn ended. An empty `end_turn` is not a failure here: Claude's protocol
/// has a failure flag, so only an adapter without one fails an empty turn (0.3, The stream protocol).
fn outcome_of_turn(reason: &TurnEndReason) -> PhaseOutcome {
    let failed = |cause| PhaseOutcome::Failed { cause };
    match reason {
        TurnEndReason::Cancelled {} => PhaseOutcome::Stopped,
        TurnEndReason::EndTurn {} => PhaseOutcome::Ended,
        TurnEndReason::MaxTokens {} => failed(PhaseFailure::MaxTokens),
        TurnEndReason::MaxTurns {} => failed(PhaseFailure::MaxTurns),
        TurnEndReason::Budget {} => failed(PhaseFailure::Budget),
        TurnEndReason::Refusal {} => failed(PhaseFailure::Refusal),
        TurnEndReason::Error { status } => failed(PhaseFailure::Api { status: *status }),
        TurnEndReason::Blocked { reason } => failed(PhaseFailure::Blocked {
            reason: reason.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_remembers_how_it_started() {
        let session = ClaudeSession::new(SessionStart::Resume { id: "abc".into() });
        assert_eq!(
            session.session_start(),
            &SessionStart::Resume { id: "abc".into() }
        );
    }

    #[test]
    fn replaying_an_empty_journal_gives_an_empty_step() {
        let mut session = ClaudeSession::new(SessionStart::Assign { uuid: "u".into() });
        assert_eq!(session.replay(&[]), Ok(Step::default()));
    }

    #[test]
    fn a_message_the_session_never_writes_is_not_readable_as_one_it_wrote() {
        let mut session = ClaudeSession::new(SessionStart::Assign { uuid: "u".into() });
        assert!(
            session
                .note_written(b"{\"type\":\"permission_reply\"}\n")
                .is_err()
        );
        assert!(session.note_written(b"not json\n").is_err());
        assert!(
            session
                .note_written(b"{\"type\":\"control_response\",\"response\":{}}\n")
                .is_err()
        );
        assert!(
            session
                .note_written(
                    b"{\"type\":\"control_request\",\"request\":{\"subtype\":\"initialize\"}}\n"
                )
                .is_err()
        );
    }

    #[test]
    fn usage_tokens_adds_the_four_counts_and_ignores_the_rest() {
        let result = json!({"usage": {"input_tokens": 1, "output_tokens": 2, "cache_read_input_tokens": 3,
                                      "cache_creation_input_tokens": 4, "service_tier": "standard"}});
        assert_eq!(usage_tokens(&result), 10);
        assert_eq!(usage_tokens(&json!({})), 0);
    }
}
