use std::sync::Arc;

use allocative::Allocative;
use dice::DiceError;
use dupe::Dupe;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OtlErr {
    #[error("unknown error")]
    Unknown,
    #[error("Dice failure {0}")]
    DiceFail(DiceError),
    #[error("IoError {0}")]
    IoError(#[from] std::io::Error),
}

impl Allocative for OtlErr {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut allocative::Visitor<'b>) {
        let vis = visitor.enter_self(&self);
        vis.exit();
    }
}
