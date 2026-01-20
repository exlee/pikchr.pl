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

type Queries = Vec<String>;
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

fn build_wasi(input: Queries) -> Result<WasiCtxWithCtx, RenderError> {
    let mut sb = String::new();
    writeln!(sb, "{}", PROLOG_INIT)?;
    for query in input {
        writeln!(sb, "{}", query)?;
    }

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

pub trait PrologRunner {}

#[macro_export]
macro_rules! get_runtime_impl {
    (
        runtime: $runtime:ident,
        async_support: $async_support:literal,
        linker_fn: $linker_fn:ident


    ) => {
        fn get_runtime() -> &'static PrologRuntime {
            $runtime.get_or_init(|| {
                let mut config = wasmtime::Config::new();
                config.async_support($async_support);
                let engine = Engine::new(&config).expect("Failed to create async engine");
                let module = Module::new(&engine, TPL_WASM).expect("Failed to compile WASM");

                let mut linker = Linker::new(&engine);
                p1::$linker_fn(&mut linker, |s: &mut LinkerState| &mut s.wasi)
                    .expect("Failed to link WASI");

                PrologRuntime {
                    engine,
                    module,
                    linker,
                }
            })
        }
    };
}

#[macro_export]
macro_rules! process_diagram_impl {
    (
        async_: $($async_kw:ident)?,
        await_: $($await_token:tt)*
    ) => {
            pub $($async_kw)? fn process_diagram(input: Queries) -> Result<PikchrCode, RenderError> {
                run_prolog(input)
                $($await_token)*
                .map_err(|e| RenderError::PrologError(format!("{}", e)))
                .map(PikchrCode::new)
            }
    };
}
#[macro_export]
macro_rules! run_prolog_impl {
        (
            asyncness: $($async_kw:ident)?,
            instantiate_fn: $inst_fn:ident,
            call_fn: $call_fn:ident,
            await_token: $($await:tt)*
        ) => {
                pub $($async_kw)? fn run_prolog(input: Queries) -> Result<String, RenderError> {
                    let runtime = get_runtime();

                    let (wasi, stdout, stderr) = build_wasi(input)?;
                    let mut store = Store::new(&runtime.engine, LinkerState { wasi });

                    let instance = runtime
                        .linker
                        .$inst_fn(&mut store, &runtime.module)
                        $($await)*
                        ?;

                    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

                    start
                        .$call_fn(&mut store, ())
                        $($await)* ?;

                    process_output(stdout, stderr)
                }
            }
}

#[cfg(test)]
mod tests {
    use super::{asynch, sync};

    macro_rules! prolog_test {
        ($name: ident, $inp:literal, $out:literal) => {
            mod $name {
                use super::*;
                prolog_test!(@common test,,[ ],sync,sync_version,$inp,$out);
                prolog_test!(@common tokio::test,async,[.await],asynch,async_version, $inp, $out);
            }
        };
        (@common $test_type:meta,$($async_kw:ident)?,[$($await_token:tt)*],$module:ident,$name:ident, $inp: literal, $out: literal) => {
            #[$test_type]
            $($async_kw)? fn $name() {
                let input = $inp;
                let expectation = $out;
                let got = $module::process_diagram(vec![String::from(input)])
                    $($await_token)*
                    .unwrap()
                    .into_inner();
                assert_eq!(got, expectation.trim());
            }
        }
    }
    prolog_test!(
        test_1,
        r#"
circle --> "circle;".
diagram --> circle.
    "#,
        "circle;"
    );
    prolog_test!(
        test_2,
        r#"
circle(Name) --> "circle", " \"", Name, "\";".
diagram --> circle("Test").
    "#,
        r#"circle "Test";"#
    );
    prolog_test!(
        test_3,
        r#"
fill(C) --> "fill ", C.
circle(N,A) --> "circle \"", N, "\" ", A, ";".
diagram --> circle("Test", fill("red")).

    "#,
        r#"circle "Test" fill red;"#
    );
    prolog_test!(
        test_4,
        r#"
circle(N,A) --> "circle \"", N, "\" ", A, ";".
diagram --> circle("Test", "fill red").
    "#,
        r#"circle "Test" fill red;"#
    );
    prolog_test!(
        test_5,
        r#"
small --> "small".
text(N,A) --> "text \"", N, "\" ", A, ";".
diagram --> text("Test", small).
    "#,
        r#"text "Test" small;"#
    );
    prolog_test!(
        test_6,
        r#"diagram --> "box;", "arrow;", "box"."#,
        "box;arrow;box"
    );
}
