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
use std::sync::Arc;

use crate::{
    commands::{execute_command, Command, CommandOutput, TargetType},
    maybe_cache,
};
use async_trait::async_trait;
use otl_core::OtlErr;

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct CommandRef(Arc<Command>);

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct CommandVal {
    output: CommandOutput,
    command: CommandRef,
}

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
    type Value = Result<CommandVal, Arc<OtlErr>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let deps = self.0.dependencies.as_slice();
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
        let _val: Vec<Self::Value> = future::join_all(futs).await.into_iter().collect();
        //Currently, we do nothing with this. What we _should_ do is check if these guys fail --
        //specifically, if build targets fail -- this would be Bad and should cause an abort
        let output = match self.0.target_type {
            TargetType::Test => execute_command(self.0.as_ref()).await.map_err(Arc::new),
            _ => maybe_cache(self.0.as_ref()).await.map_err(Arc::new),
        }?;
        Ok(CommandVal {
            output,
            command: self.clone(),
        })
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        false
    }
}

async fn get_command_deps(
    ctx: &mut DiceComputations<'_>,
    var: &[String],
) -> Result<Vec<CommandRef>, OtlErr> {
    let futs = ctx.compute_many(var.iter().map(|val| {
        DiceComputations::declare_closure(
            move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandRef, OtlErr>> {
                let val = LookupCommand::from_str_ref(val);
                ctx.compute(&val).map_err(OtlErr::DiceFail).boxed()
            },
        )
    }));

    future::join_all(futs).await.into_iter().collect()
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
pub trait LocalCommandExecutor {
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
impl LocalCommandExecutor for DiceComputations<'_> {
    async fn execute_command(
        &mut self,
        command: &CommandRef,
    ) -> Result<CommandOutput, Arc<OtlErr>> {
        match self.compute(command).await {
            Ok(val) => val.map(|val| val.output),
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
                            Ok(val) => val.map(|val| val.output),
                            Err(err) => Err(Arc::new(OtlErr::DiceFail(err))),
                        })
                        .boxed()
                },
            )
        }));

        future::join_all(futs).await
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
    pub(crate) all_commands: Vec<CommandRef>,
}
impl CommandGraph {
    pub async fn from_commands_str(commands: impl AsRef<str>) -> Result<Self, OtlErr> {
        let commands: Vec<Command> = serde_yaml::from_str(commands.as_ref())?;
        Self::new(commands).await
    }

    pub async fn new(commands: Vec<Command>) -> Result<Self, OtlErr> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut ctx = dice.updater();
        let commands: Vec<CommandRef> = commands
            .into_iter()
            .map(|val| CommandRef(Arc::new(val)))
            .collect();
        ctx.add_commands(commands.iter().cloned())?;

        let _ctx = ctx.commit().await;
        let graph = CommandGraph {
            dice,
            all_commands: commands,
        };

        Ok(graph)
    }
    pub async fn run_all_tests(&self) -> Vec<Result<CommandOutput, Arc<OtlErr>>> {
        let refs = self
            .all_commands
            .iter()
            .filter(|&val| val.0.target_type == TargetType::Test)
            .cloned()
            .collect();

        let ctx = self.dice.updater();
        let mut tx = ctx.commit().await;
        tx.execute_commands(refs).await
    }

    pub async fn run_all_build(&self) -> Vec<Result<CommandOutput, Arc<OtlErr>>> {
        let refs = self
            .all_commands
            .iter()
            .filter(|&val| val.0.target_type == TargetType::Build)
            .cloned()
            .collect();

        let ctx = self.dice.updater();
        let mut tx = ctx.commit().await;
        tx.execute_commands(refs).await
    }

    pub async fn run_one_test(
        &self,
        test_name: impl Into<String>,
    ) -> Result<CommandOutput, Arc<OtlErr>> {
        let ctx = self.dice.updater();
        let mut tx = ctx.commit().await;
        let command = tx
            .compute(&LookupCommand(Arc::new(test_name.into())))
            .await
            .map_err(|val| Arc::new(OtlErr::DiceFail(val)))?;
        tx.execute_command(&command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    async fn execute_all_tests_in_file(yaml_data: &str) {
        let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data);

        let script = script.unwrap();
        let graph = CommandGraph::new(script).await.unwrap();
        let results = graph.run_all_tests().await;
        for result in results {
            match result {
                Ok(out) => {
                    assert_eq!(out.status_code, 0)
                }
                Err(err) => {
                    panic!("Seeing error {:?}", err);
                }
            }
        }
    }

    #[tokio::test]
    async fn dependency_less_exec() {
        let yaml_data = include_str!("../../../test_data/command_lists/cl1.yaml");
        execute_all_tests_in_file(yaml_data).await
    }

    #[tokio::test]
    async fn test_with_deps() {
        let yaml_data = include_str!("../../../test_data/command_lists/cl2.yaml");
        execute_all_tests_in_file(yaml_data).await
    }

    #[tokio::test]
    async fn test_with_intraphase_deps() {
        let yaml_data = include_str!("../../../test_data/command_lists/cl3.yaml");
        execute_all_tests_in_file(yaml_data).await
    }
}
