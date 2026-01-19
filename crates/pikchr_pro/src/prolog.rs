use std::{fmt::Write, fs::File, sync::OnceLock};

use anyhow::{Context, Result, anyhow};
use thiserror::Error;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::{
    DirPerms,
    FilePerms,
    WasiCtxBuilder,
    filesystem::Dir,
    p1::{self, WasiP1Ctx},
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

use crate::types::*;

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

#[derive(Debug, Error, Clone)]
pub enum RenderError {
    #[error("Prolog error: {0}")]
    PrologError(String),
    #[error("Pikchr error: {0}")]
    PikchrError(String),
    #[error("Anyhow: {0}")]
    AnyhowError(String),
    #[error("Fmt error: {0}")]
    FormatError(#[from] std::fmt::Error),
}

impl From<anyhow::Error> for RenderError {
    fn from(value: anyhow::Error) -> Self {
        RenderError::AnyhowError(value.to_string())
    }
}

#[cfg(feature = "async")]
pub mod asynch;
#[cfg(feature = "sync")]
pub mod sync;

type WasiCtxWithCtx = (
    wasmtime_wasi::p1::WasiP1Ctx,
    wasmtime_wasi::p2::pipe::MemoryOutputPipe,
    wasmtime_wasi::p2::pipe::MemoryOutputPipe,
);
fn build_wasi(input: &str) -> Result<WasiCtxWithCtx, RenderError> {
    let mut sb = String::new();
    writeln!(sb, "{}", PROLOG_INIT)?;
    writeln!(sb, "{}", input)?;

    let stdin = MemoryInputPipe::new(sb);
    let stdout = MemoryOutputPipe::new(65535);
    let stderr = MemoryOutputPipe::new(65535);

    let ctx = WasiCtxBuilder::new()
        .stdin(stdin)
        .stdout(stdout.clone())
        .stderr(stdout.clone())
        .args(&["tpl", "-q", "--consult", "-g", "run, halt"])
        .preopened_dir(".", "/", DirPerms::READ, FilePerms::READ)
        .expect("Can't open current dir as root")
        .env("PWD", "/")
        .build_p1();
    Ok((ctx, stdout, stderr))
}
fn process_output(
    stdout: MemoryOutputPipe,
    stderr: MemoryOutputPipe,
) -> Result<String, RenderError> {
    let output_bytes = stdout.contents();
    let output_str =
        String::from_utf8(output_bytes.to_vec()).context("Prolog output invalid UTF-8")?;

    let err_bytes = stderr.contents();
    let err_str = String::from_utf8(err_bytes.to_vec()).context("Prolog output invalid UTF-8")?;

    if !err_str.is_empty() {
        return Err(RenderError::PrologError(err_str));
    }
    if output_str.trim().starts_with("error(") {
        return Err(RenderError::PrologError(output_str));
    }
    if output_str.starts_with("Error:") {
        return Err(RenderError::PrologError(output_str));
    }
    Ok(output_str)
}
