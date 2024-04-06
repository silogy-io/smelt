use allocative::Allocative;
use derive_more::Display;
use dice::{
    CancellationContext, DetectCycles, Dice, DiceComputations, DiceTransactionUpdater, Key,
};
use dupe::Dupe;
use futures::{
    future::{self, BoxFuture},
    TryFutureExt,
};

use dice::InjectedKey;
use futures::FutureExt;
use std::{collections::BTreeMap, error::Error, sync::Arc};

use crate::{
    command::{
        execute_command, Command, CommandOutput, CommandScript, CommandScriptInner, TargetType,
    },
    error::OtlErr,
};
use async_trait::async_trait;

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct CommandRef(Arc<Command>);

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct QueryCommandRef(Arc<Command>);

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct LookupCommand(Arc<String>);

impl From<QueryCommandRef> for CommandRef {
    fn from(value: QueryCommandRef) -> Self {
        Self(value.0)
    }
}

impl From<CommandRef> for QueryCommandRef {
    fn from(value: CommandRef) -> Self {
        Self(value.0)
    }
}

impl LookupCommand {
    fn from_str_ref(strref: &String) -> Self {
        Self(Arc::new(strref.clone()))
    }
}

impl InjectedKey for LookupCommand {
    type Value = CommandRef;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for CommandRef {
    type Value = Result<CommandOutput, Arc<OtlErr>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> Self::Value {
        let deps = self.0.script.as_slice();
        let command_deps = get_command_deps(ctx, deps).await?;

        let futs = ctx.compute_many(command_deps.into_iter().map(|val| {
            DiceComputations::declare_closure(
                move |ctx: &mut DiceComputations| -> BoxFuture<Self::Value> {
                    ctx.compute(&val)
                        .map(|computed_val| match computed_val {
                            Ok(val) => val,
                            Err(err) => Err(Arc::new(OtlErr::DiceFail(err))),
                        })
                        .boxed()
                },
            )
        }));
        let val: Vec<Self::Value> = future::join_all(futs).await.into_iter().collect();
        //Currently, we do nothing with this. What we _should_ do is check if these guys fail --
        //specifically, if build targets fail -- this would be Bad and should cause an abort
        execute_command(self.0.as_ref())
            .await
            .map_err(|err| Arc::new(err))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        false
    }
}

async fn get_command_deps(
    ctx: &mut DiceComputations<'_>,
    var: &[String],
) -> Result<Vec<CommandRef>, OtlErr> {
    let futs = ctx.compute_many(var.into_iter().map(|val| {
        DiceComputations::declare_closure(
            move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandRef, OtlErr>> {
                let val = LookupCommand::from_str_ref(val);
                ctx.compute(&val)
                    .map_err(|err| OtlErr::DiceFail(err))
                    .boxed()
            },
        )
    }));
    let rv = future::join_all(futs).await.into_iter().collect();
    rv
}

#[derive(Clone)]
pub enum GraphKey {
    Target(String),
    File(std::path::PathBuf),
}

pub trait CommandSetter {
    fn add_command(&mut self, command: CommandRef) -> Result<(), OtlErr>;
    fn add_commands(
        &mut self,
        equations: impl IntoIterator<Item = CommandRef>,
    ) -> Result<(), OtlErr>;
}

#[async_trait]
pub trait CommandExecutor {
    async fn execute_command(
        &mut self,
        command_name: &CommandRef,
    ) -> Result<CommandOutput, Arc<OtlErr>>;

    async fn execute_commands(
        &mut self,
        command_name: Vec<CommandRef>,
    ) -> Vec<Result<CommandOutput, Arc<OtlErr>>>;
}

#[async_trait]
impl CommandExecutor for DiceComputations<'_> {
    async fn execute_command(
        &mut self,
        command: &CommandRef,
    ) -> Result<CommandOutput, Arc<OtlErr>> {
        match self.compute(command).await {
            Ok(val) => val,
            Err(dicey) => Err(Arc::new(OtlErr::DiceFail(dicey))),
        }
    }
    async fn execute_commands(
        &mut self,
        commands: Vec<CommandRef>,
    ) -> Vec<Result<CommandOutput, Arc<OtlErr>>> {
        let futs = self.compute_many(commands.into_iter().map(|val| {
            DiceComputations::declare_closure(
                move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandOutput, Arc<OtlErr>>> {
                    ctx.compute(&val)
                        .map(|computed_val| match computed_val {
                            Ok(val) => val,
                            Err(err) => Err(Arc::new(OtlErr::DiceFail(err))),
                        })
                        .boxed()
                },
            )
        }));
        let val = future::join_all(futs).await;
        val
    }
}

impl CommandSetter for DiceTransactionUpdater {
    fn add_command(&mut self, command: CommandRef) -> Result<(), OtlErr> {
        let lookup = LookupCommand::from_str_ref(&command.0.name);

        self.changed_to(vec![(lookup, command)])?;
        Ok(())
    }

    fn add_commands(
        &mut self,
        commands: impl IntoIterator<Item = CommandRef>,
    ) -> Result<(), OtlErr> {
        for command in commands {
            self.add_command(command)?;
        }
        Ok(())
    }
}

pub struct CommandGraph {
    dice: Arc<Dice>,
    all_commands: Vec<CommandRef>,
}
impl CommandGraph {
    async fn create_command_graph(commands: Vec<Command>) -> Result<Self, OtlErr> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut ctx = dice.updater();
        let commands: Vec<CommandRef> = commands
            .into_iter()
            .map(|val| CommandRef(Arc::new(val)))
            .collect();
        ctx.add_commands(commands.iter().cloned())?;

        let ctx = ctx.commit().await;
        let graph = CommandGraph {
            dice,
            all_commands: commands,
        };
        Ok(graph)
    }
    async fn run_all_tests(&self) -> Vec<Result<CommandOutput, Arc<OtlErr>>> {
        let refs = self
            .all_commands
            .iter()
            .cloned()
            .filter(|val| val.0.target_type == TargetType::Test)
            .collect();

        let mut ctx = self.dice.updater();
        let mut tx = ctx.commit().await;
        tx.execute_commands(refs).await
    }
    async fn run_one_test(
        &self,
        test_name: impl Into<String>,
    ) -> Result<CommandOutput, Arc<OtlErr>> {
        let mut ctx = self.dice.updater();
        let mut tx = ctx.commit().await;
        let command = tx
            .compute(&LookupCommand(Arc::new(test_name.into())))
            .await
            .map_err(|val| Arc::new(OtlErr::DiceFail(val)))?;
        tx.execute_command(&command).await
    }
}
