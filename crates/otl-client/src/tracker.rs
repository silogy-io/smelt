use super::Subscriber;
use async_trait::async_trait;
use otl_data::{
    command_event::CommandVariant, event::Et, CommandFinished, CommandOutput, CommandStarted, Event,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::SystemTime};

pub enum InvocationTrackerError {
    InvalidStateTransition,
}

type CommandHandle = String;
#[async_trait]
impl Subscriber for InvocationTracker {
    type Error = InvocationTrackerError;
    async fn recv_event(&mut self, event: Arc<Event>) -> Result<(), Self::Error> {
        let ts: SystemTime = event.time.clone().unwrap().try_into().unwrap();
        if let Some(ref et) = event.et {
            match et {
                Et::Invoke(invoke) => Ok(()),
                Et::Command(command) => {
                    let name = command.command_ref.clone();

                    let slot = self.command_map.entry(name);

                    let mut rv = Ok(());
                    let state = match command.command_variant.as_ref().unwrap() {
                        CommandVariant::Started(_) => {
                            slot.and_modify(|val| rv = val.to_started(ts))
                        }
                        CommandVariant::Finished(CommandFinished {
                            out: Some(CommandOutput { status_code }),
                        }) => slot.and_modify(|val| rv = val.to_completed(ts, *status_code)),
                        CommandVariant::Cancelled(_) => {
                            slot.and_modify(|val| rv = val.to_cancelled(ts))
                        }
                        _ => slot,
                    };
                    Ok(())
                }
            }
        } else {
            Ok(())
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
    fn to_started(
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
    fn to_completed(
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
    fn to_cancelled(
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

struct InvokerMetaData {}

pub struct InvocationTracker {
    invoker: InvokerMetaData,
    command_map: HashMap<CommandHandle, ExecCommand>,
}
