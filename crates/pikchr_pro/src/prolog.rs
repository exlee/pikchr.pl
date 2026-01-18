use std::{fmt::Write, sync::OnceLock};

use crate::types::*;

use anyhow::{Context, Result, anyhow};
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::{
    WasiCtxBuilder,
    p1::{self, WasiP1Ctx},
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

static TPL_WASM: &[u8] = include_bytes!("../native/tpl/tpl.wasm");
static PROLOG_INIT: &str = include_str!("../native/prolog/init.pl");

pub(crate) struct LinkerState {
    pub wasi: WasiP1Ctx,
}
pub(crate) struct PrologRuntime {
    pub engine: Engine,
    pub module: Module,
    pub linker: Linker<LinkerState>,
}

#[cfg(feature = "async")]
pub mod asynch;
#[cfg(feature = "sync")]
pub mod sync;
