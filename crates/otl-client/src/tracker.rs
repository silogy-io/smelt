use super::Subscriber;
use anyhow::anyhow;
use async_trait::async_trait;
use otl_data::{
    command_event::CommandVariant, event::Et, invoke_event::InvokeVariant, CommandFinished,
    CommandOutput, Event, ExecutionStart,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::SystemTime};
use thiserror::Error;

type InvocationUUID = String;

#[derive(Error, Debug)]
pub enum InvocationTrackerError {
    #[error("Invalid state transition for a command")]
    InvalidStateTransition,
    #[error("Invalid state transition for an invocation")]
    InvalidInvokeST,
    #[error("We don't handle a command variant")]
    UncoveredCommand(CommandVariant),
    #[error("We found a message with no payload")]
    EventMissing,
}

#[async_trait]
impl Subscriber for RunningInvocationTracker {
    async fn recv_event(&mut self, event: Arc<Event>) -> Result<(), anyhow::Error> {
        let trace_id = &event.trace_id;
        self.all_invocations
            .entry(trace_id.clone())
            .or_default()
            .recv_event(event)
            .await
    }
}

type CommandHandle = String;
#[async_trait]
impl Subscriber for SingleInvocationTracker {
    async fn recv_event(&mut self, event: Arc<Event>) -> Result<(), anyhow::Error> {
        let ts: SystemTime = event.time.clone().unwrap().try_into().unwrap();
        if let Some(ref et) = event.et {
            match et {
                Et::Invoke(invoke) => {
                    if let Some(ref var) = invoke.invoke_variant {
                        self.invoker.process(var)?;
                        Ok(())
                    } else {
                        Err(anyhow!(InvocationTrackerError::EventMissing))
                    }
                }
                Et::Command(command) => {
                    let name = command.command_ref.clone();

                    let slot = self.command_map.entry(name);

                    let mut rv = Ok(());
                    match command.command_variant.as_ref().unwrap() {
                        CommandVariant::Scheduled(_) => {
                            slot.or_insert(ExecCommand::scheduled(ts));
                        }
                        CommandVariant::Started(_) => {
                            slot.and_modify(|val| rv = val.started(ts));
                        }
                        CommandVariant::Finished(CommandFinished {
                            out: Some(CommandOutput { status_code }),
                        }) => {
                            slot.and_modify(|val| rv = val.completed(ts, *status_code));
                        }
                        CommandVariant::Cancelled(_) => {
                            slot.and_modify(|val| rv = val.cancelled(ts));
                        }
                        _ => {
                            return Err(InvocationTrackerError::UncoveredCommand(
                                command.command_variant.as_ref().unwrap().clone(),
                            )
                            .into())
                        }
                    };
                    Ok(())
                }
            }
        } else {
            Err(InvocationTrackerError::EventMissing.into())
        }
    }
}

enum OutputLookinThang {
    InMemory(String),
    OnDisk(PathBuf),
}

enum ExecCommandState {
    Scheduled {
        sched_time: SystemTime,
    },
    Running {
        sched_time: SystemTime,
        started_time: SystemTime,
    },
    Cancelled {
        sched_time: SystemTime,
        started_time: SystemTime,
        cancelled_time: SystemTime,
    },
    Completed {
        status_code: i32,
        sched_time: SystemTime,
        started_time: SystemTime,
        ended_time: SystemTime,
    },
}

struct ExecCommand {
    status: ExecCommandState,

    stdout: OutputLookinThang,
    stderr: OutputLookinThang,
}

impl ExecCommand {
    fn scheduled(started_time: impl Into<SystemTime>) -> Self {
        let sched_time = started_time.into();
        Self {
            status: ExecCommandState::Scheduled { sched_time },
            stderr: OutputLookinThang::InMemory(String::new()),
            stdout: OutputLookinThang::InMemory(String::new()),
        }
    }

    fn started(
        &mut self,
        started_time: impl Into<SystemTime>,
    ) -> Result<(), InvocationTrackerError> {
        let started_time = started_time.into();
        match self.status {
            ExecCommandState::Scheduled { sched_time } => {
                self.status = ExecCommandState::Running {
                    sched_time,
                    started_time,
                };
                Ok(())
            }
            _ => Err(InvocationTrackerError::InvalidStateTransition),
        }
    }
    fn completed(
        &mut self,
        completed_time: impl Into<SystemTime>,
        status_code: i32,
    ) -> Result<(), InvocationTrackerError> {
        let ended_time = completed_time.into();
        match self.status {
            ExecCommandState::Running {
                sched_time,
                started_time,
            } => {
                self.status = ExecCommandState::Completed {
                    ended_time,
                    sched_time,
                    started_time,
                    status_code,
                };
                Ok(())
            }
            _ => Err(InvocationTrackerError::InvalidStateTransition),
        }
    }
    fn cancelled(
        &mut self,
        cancelled_time: impl Into<SystemTime>,
    ) -> Result<(), InvocationTrackerError> {
        let cancelled_time = cancelled_time.into();
        match self.status {
            ExecCommandState::Running {
                sched_time,
                started_time,
            } => {
                self.status = ExecCommandState::Cancelled {
                    started_time,
                    sched_time,
                    cancelled_time,
                };
                Ok(())
            }
            _ => Err(InvocationTrackerError::InvalidStateTransition),
        }
    }
}

#[derive(Default)]
enum InvocationState {
    #[default]
    Uninit,
    Running(InvokerMetaData),
    Completed(InvokerMetaData),
}

#[derive(Default, Clone)]
struct InvokerMetaData {
    path: String,
    username: String,
    hostname: String,
}

impl InvocationState {
    fn process(&mut self, invoke: &InvokeVariant) -> Result<(), InvocationTrackerError> {
        match invoke {
            InvokeVariant::Start(started) => self.to_start(started.clone()),
            InvokeVariant::Done(_) => self.to_completed(),
        }
    }

    fn to_start(&mut self, var: ExecutionStart) -> Result<(), InvocationTrackerError> {
        match self {
            Self::Uninit => {
                *self = Self::Running(InvokerMetaData {
                    path: var.path,
                    username: var.username,
                    hostname: var.hostname,
                });
                Ok(())
            }
            _ => Err(InvocationTrackerError::InvalidInvokeST),
        }
    }
    fn to_completed(&mut self) -> Result<(), InvocationTrackerError> {
        match self {
            Self::Running(ref mut inner) => {
                *self = Self::Completed(inner.clone());
                Ok(())
            }
            _ => Err(InvocationTrackerError::InvalidInvokeST),
        }
    }
}

/// Tracks all the state within one ClientCommand -- e.g. if a client wants to run one test, this
/// struct will track all of the stuff that follows from that (building dependencies), etc
#[derive(Default)]
pub struct SingleInvocationTracker {
    invoker: InvocationState,
    command_map: HashMap<CommandHandle, ExecCommand>,
}

///RunningInvocationTracker tracks the data associated with each client submitted command.
///If the client says run one test that is an invocation, if the client says run all tests, that is another invocation
pub struct RunningInvocationTracker {
    all_invocations: HashMap<InvocationUUID, SingleInvocationTracker>,
}
