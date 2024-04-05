use allocative::Allocative;
use derive_more::Display;
use dice::{CancellationContext, DiceComputations, DiceTransactionUpdater, Key};
use dupe::Dupe;
use futures::{
    future::{self, BoxFuture},
    TryFutureExt,
};

use dice::InjectedKey;
use futures::FutureExt;
use std::{error::Error, sync::Arc};

use crate::{
    error::OtlErr,
    parser::{execute_command, Command, CommandOutput},
};
use async_trait::async_trait;

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct CommandRef(Arc<Command>);

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct LookupCommand(Arc<String>);

impl LookupCommand {
    fn from_str_ref(strref: &String) -> Self {
        Self(Arc::new(strref.clone()))
    }
}

impl InjectedKey for LookupCommand {
    type Value = Arc<Command>;
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

//async fn lookup_unit(ctx: &mut DiceComputations<'_>, var: &Var) -> anyhow::Result<Arc<Equation>> {
//    Ok(ctx.compute(&LookupVar(var.clone())).await?)
//}

async fn get_command_deps(
    ctx: &mut DiceComputations<'_>,
    var: &[String],
) -> Result<Vec<CommandRef>, OtlErr> {
    //let futs = ctx.compute_many(units.iter().map(|unit| {
    //    DiceComputations::declare_closure(
    //        move |ctx: &mut DiceComputations| -> BoxFuture<Result<i64, Arc<anyhow::Error>>> {
    //            match unit {
    //                Unit::Var(var) => ctx.eval(var.clone()).boxed(),
    //                Unit::Literal(lit) => futures::future::ready(Ok(*lit)).boxed(),
    //            }
    //        },
    //    )
    //}));

    let futs = ctx.compute_many(var.into_iter().map(|val| {
        DiceComputations::declare_closure(
            move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandRef, OtlErr>> {
                let val = LookupCommand::from_str_ref(val);
                ctx.compute(&val)
                    .map_ok(|val| CommandRef(val))
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

//impl CommandSetter for DiceTransactionUpdater {
//    fn add_command(&mut self, command: CommandRef) -> Result<(), OtlErr> {
//
//    }
//}

fn execute_commands(commands: Vec<Command>) -> Result<(), OtlErr> {
    Ok(())
}
