//! The structural checks of a workflow (0.7, A workflow is a declared transition table), as pure functions over
//! [`Workflow`]. [`validate`] is what a pack load runs: every refusal in one pass, in the order of the table.
//! [`check_launch`] is what a launch runs, for what a table cannot tell until a run is asked for (two identical
//! fan-out instances). [`check_adoption`] checks a start at a named phase and round.

use crate::duration::Duration;
use crate::workflow::{
    ASK_HUMAN, Adoption, Decision, Fanout, Instance, MIN_WAIT_INTERVAL, Phase, Target, Then,
    Workflow,
};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    NoPhases,
    /// A phase name that is empty or is `ask_human`.
    BadPhaseName {
        name: String,
    },
    UnknownStart {
        start: String,
    },
    /// A transition that names a phase the workflow does not declare. `at` says where it was written.
    DanglingThen {
        phase: String,
        at: String,
        target: String,
    },
    /// A decision whose `on:` maps a token that its `tokens:` does not declare.
    UndeclaredToken {
        phase: String,
        token: String,
    },
    /// A decision that declares a token in `tokens:` and does not map it in `on:`.
    UnhandledToken {
        phase: String,
        token: String,
    },
    /// A decision or a human phase that maps no token at all.
    NoTokens {
        phase: String,
    },
    /// An `end:` with no outcome.
    EmptyOutcome {
        phase: String,
        at: String,
    },
    /// A fan-out with both `models:` and `instances:`, or with neither.
    FanoutSource {
        phase: String,
    },
    /// A fan-out instance that names none of a model, a definition, a profile and a prompt.
    EmptyInstance {
        phase: String,
        index: usize,
    },
    ZeroTimeout {
        phase: String,
    },
    IntervalTooShort {
        phase: String,
        interval: Duration,
    },
    /// A `workspace:` or `integrate:` that is not `pack/name`.
    BadImplementationName {
        key: &'static str,
        name: String,
    },
}

impl fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkflowError::NoPhases => write!(f, "the workflow declares no phases"),
            WorkflowError::BadPhaseName { name } => {
                write!(
                    f,
                    "`{name}` cannot be a phase name (it is empty or reserved: `{ASK_HUMAN}`)"
                )
            }
            WorkflowError::UnknownStart { start } => {
                write!(
                    f,
                    "`start:` names `{start}`, which is not a phase of this workflow"
                )
            }
            WorkflowError::DanglingThen { phase, at, target } => write!(
                f,
                "phase `{phase}`: `{at}` goes to `{target}`, which is not a phase of this workflow"
            ),
            WorkflowError::UndeclaredToken { phase, token } => write!(
                f,
                "phase `{phase}`: `on:` maps the token `{token}`, which `tokens:` does not declare"
            ),
            WorkflowError::UnhandledToken { phase, token } => write!(
                f,
                "phase `{phase}`: the token `{token}` is declared in `tokens:` and has no entry in `on:`"
            ),
            WorkflowError::NoTokens { phase } => {
                write!(
                    f,
                    "phase `{phase}`: `on:` maps no token, so nothing can follow it"
                )
            }
            WorkflowError::EmptyOutcome { phase, at } => {
                write!(
                    f,
                    "phase `{phase}`: `{at}` ends the run with an empty outcome"
                )
            }
            WorkflowError::FanoutSource { phase } => write!(
                f,
                "phase `{phase}`: `fanout:` needs `models:` or `instances:`, and not both"
            ),
            WorkflowError::EmptyInstance { phase, index } => write!(
                f,
                "phase `{phase}`: fan-out instance {index} names no model, definition, profile or prompt"
            ),
            WorkflowError::ZeroTimeout { phase } => {
                write!(
                    f,
                    "phase `{phase}`: `timeout:` is zero, so the wait ends at once"
                )
            }
            WorkflowError::IntervalTooShort { phase, interval } => write!(
                f,
                "phase `{phase}`: `interval:` is {interval}, and a wait polls at most once a second"
            ),
            WorkflowError::BadImplementationName { key, name } => {
                write!(
                    f,
                    "`{key}:` is `{name}`; an implementation is named `pack/name`"
                )
            }
        }
    }
}

impl std::error::Error for WorkflowError {}

/// `pack/name`: two non-empty parts and one slash.
fn is_implementation_name(name: &str) -> bool {
    matches!(name.split_once('/'), Some((pack, local)) if !pack.is_empty()
        && !local.is_empty() && !local.contains('/'))
}

fn check_target(
    workflow: &Workflow,
    phase: &str,
    at: &str,
    target: &Target,
    errors: &mut Vec<WorkflowError>,
) {
    match target {
        Target::Name(name) if name == ASK_HUMAN => {}
        Target::Name(name) if !workflow.phases.contains(name) => {
            errors.push(WorkflowError::DanglingThen {
                phase: phase.to_string(),
                at: at.to_string(),
                target: name.clone(),
            });
        }
        Target::Name(_) => {}
        Target::End(end) if end.end.is_empty() => errors.push(WorkflowError::EmptyOutcome {
            phase: phase.to_string(),
            at: at.to_string(),
        }),
        Target::End(_) => {}
    }
}

fn check_decision(phase: &str, decision: &Decision, errors: &mut Vec<WorkflowError>) {
    if decision.on.is_empty() {
        errors.push(WorkflowError::NoTokens {
            phase: phase.to_string(),
        });
    }
    if decision.tokens.is_empty() {
        return;
    }
    for token in decision.on.keys() {
        if !decision.tokens.contains(token) {
            errors.push(WorkflowError::UndeclaredToken {
                phase: phase.to_string(),
                token: token.clone(),
            });
        }
    }
    for token in &decision.tokens {
        if !decision.on.contains_key(token) {
            errors.push(WorkflowError::UnhandledToken {
                phase: phase.to_string(),
                token: token.clone(),
            });
        }
    }
}

fn check_fanout(phase: &str, fanout: &Fanout, errors: &mut Vec<WorkflowError>) {
    if fanout.models.is_empty() == fanout.instances.is_empty() {
        errors.push(WorkflowError::FanoutSource {
            phase: phase.to_string(),
        });
    }
    for (index, instance) in fanout.instances.iter().enumerate() {
        let empty = instance.model.is_none()
            && instance.definition.is_none()
            && instance.profile.is_none()
            && instance.prompt.is_none();
        if empty {
            errors.push(WorkflowError::EmptyInstance {
                phase: phase.to_string(),
                index,
            });
        }
    }
}

fn check_phase(workflow: &Workflow, name: &str, phase: &Phase, errors: &mut Vec<WorkflowError>) {
    if name.is_empty() || name == ASK_HUMAN {
        errors.push(WorkflowError::BadPhaseName {
            name: name.to_string(),
        });
    }
    for (at, target) in phase.targets() {
        check_target(workflow, name, &at, target, errors);
    }
    if let Some(Then::Decision(decision)) = phase.then() {
        check_decision(name, decision, errors);
    }
    match phase {
        Phase::Agent(agent) => {
            if let Some(fanout) = &agent.fanout {
                check_fanout(name, fanout, errors);
            }
        }
        Phase::Wait(wait) => {
            if wait.timeout.as_secs() == 0 {
                errors.push(WorkflowError::ZeroTimeout {
                    phase: name.to_string(),
                });
            }
            if wait.interval < MIN_WAIT_INTERVAL {
                errors.push(WorkflowError::IntervalTooShort {
                    phase: name.to_string(),
                    interval: wait.interval,
                });
            }
        }
        Phase::Human(human) => {
            if human.on.is_empty() {
                errors.push(WorkflowError::NoTokens {
                    phase: name.to_string(),
                });
            }
        }
        Phase::Script(_) => {}
    }
}

/// Checks a workflow's table: the start, every transition's target, every decision's tokens, fan-out sources,
/// wait timings and implementation names. Returns every refusal, in the table's order.
pub fn validate(workflow: &Workflow) -> Result<(), Vec<WorkflowError>> {
    let mut errors = Vec::new();
    if workflow.phases.is_empty() {
        errors.push(WorkflowError::NoPhases);
    }
    if let Some(start) = &workflow.start
        && !workflow.phases.contains(start)
    {
        errors.push(WorkflowError::UnknownStart {
            start: start.clone(),
        });
    }
    for (key, name) in [
        ("workspace", &workflow.workspace),
        ("integrate", &workflow.integrate),
    ] {
        if let Some(name) = name
            && !is_implementation_name(name)
        {
            errors.push(WorkflowError::BadImplementationName {
                key,
                name: name.clone(),
            });
        }
    }
    for (name, phase) in workflow.phases.iter() {
        check_phase(workflow, name, phase, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// What a launch refuses and a load cannot (0.3, 0.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchRefusal {
    /// Two instances of a fan-out that are the same, so the second would repeat the first (0.7: "Two identical
    /// instances are refused at launch"). `first` and `second` are their places in `instances:`.
    IdenticalInstances {
        phase: String,
        first: usize,
        second: usize,
    },
    /// A model listed twice in `models:`.
    RepeatedModel { phase: String, model: String },
}

impl fmt::Display for LaunchRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchRefusal::IdenticalInstances {
                phase,
                first,
                second,
            } => write!(
                f,
                "phase `{phase}`: fan-out instances {first} and {second} are identical"
            ),
            LaunchRefusal::RepeatedModel { phase, model } => {
                write!(
                    f,
                    "phase `{phase}`: fan-out lists the model `{model}` twice"
                )
            }
        }
    }
}

impl std::error::Error for LaunchRefusal {}

/// The pairs of instances that are identical, as places in the list, each pair once and in order.
pub fn identical_instances(instances: &[Instance]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for (second, instance) in instances.iter().enumerate() {
        if let Some(first) = instances[..second]
            .iter()
            .position(|other| other == instance)
        {
            pairs.push((first, second));
        }
    }
    pairs
}

/// Marks a workflow for refusal at launch when a fan-out would run the same thing twice. A fan-out whose model
/// aliases resolve to one model is the launch's own check, because only the user's mapping knows.
pub fn check_launch(workflow: &Workflow) -> Result<(), Vec<LaunchRefusal>> {
    let mut refusals = Vec::new();
    for (name, phase) in workflow.phases.iter() {
        let Phase::Agent(agent) = phase else { continue };
        let Some(fanout) = &agent.fanout else {
            continue;
        };
        for (first, second) in identical_instances(&fanout.instances) {
            refusals.push(LaunchRefusal::IdenticalInstances {
                phase: name.to_string(),
                first,
                second,
            });
        }
        let mut seen = BTreeSet::new();
        for model in &fanout.models {
            if !seen.insert(model) {
                refusals.push(LaunchRefusal::RepeatedModel {
                    phase: name.to_string(),
                    model: model.clone(),
                });
            }
        }
    }
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(refusals)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdoptionError {
    UnknownPhase {
        phase: String,
    },
    /// Rounds start at 1.
    RoundZero,
}

impl fmt::Display for AdoptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdoptionError::UnknownPhase { phase } => {
                write!(
                    f,
                    "cannot adopt at `{phase}`, which is not a phase of this workflow"
                )
            }
            AdoptionError::RoundZero => write!(f, "cannot adopt at round 0; rounds start at 1"),
        }
    }
}

impl std::error::Error for AdoptionError {}

/// Checks that a run can start at `adoption`: the phase is in the table and the round is 1 or more.
pub fn check_adoption(workflow: &Workflow, adoption: &Adoption) -> Result<(), AdoptionError> {
    if !workflow.phases.contains(&adoption.phase) {
        return Err(AdoptionError::UnknownPhase {
            phase: adoption.phase.clone(),
        });
    }
    if adoption.round == 0 {
        return Err(AdoptionError::RoundZero);
    }
    Ok(())
}
