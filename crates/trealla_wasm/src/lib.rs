use anyhow::{Context, Result, anyhow};
use std::{fmt::Write, sync::OnceLock};
use wasmtime::{Linker, Module, Store, Trap};
use wasmtime_wasi::{
    DirPerms, FilePerms, WasiCtxBuilder,
    p1::{self, WasiP1Ctx},
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

/// Maximum Wasmtime fuel available to each Prolog execution by default.
pub const DEFAULT_PROLOG_FUEL: u64 = 1_000_000_000;
#[cfg(feature = "sync")]
static RUNTIME_SYNC: OnceLock<PrologRuntime> = OnceLock::new();
#[cfg(feature = "async")]
static RUNTIME_ASYNC: OnceLock<PrologRuntime> = OnceLock::new();

static TPL_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tpl.bin"));

type WasiCtxWithCtx = (
    wasmtime_wasi::p1::WasiP1Ctx,
    wasmtime_wasi::p2::pipe::MemoryOutputPipe,
    wasmtime_wasi::p2::pipe::MemoryOutputPipe,
);

pub(crate) struct LinkerState {
    pub wasi: WasiP1Ctx,
}

pub(crate) struct PrologRuntime {
    pub engine: wasmtime::Engine,
    pub module: Module,
    pub linker: Linker<LinkerState>,
}

macro_rules! get_runtime_impl {
    (
        runtime: $runtime:ident,
        linker_fn: $linker_fn:ident


    ) => {
        fn get_runtime() -> &'static PrologRuntime {
            $runtime.get_or_init(|| {
                let mut config = wasmtime::Config::new();
                config.consume_fuel(true);
                let engine = wasmtime::Engine::new(&config).expect("Failed to create async engine");
                let module = if cfg!(precompiled_wasm) {
                    unsafe { Module::deserialize(&engine, TPL_BYTES) }.unwrap_or_else(|e| {
                        eprintln!("AOT load failed ({}), recompiling...", e);
                        Module::new(&engine, TPL_BYTES).expect("Final fallback failed")
                    })
                } else {
                    Module::new(&engine, TPL_BYTES).expect("Failed to compile raw WASM")
                };

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

macro_rules! run_prolog_impl {
        (
            asyncness: $($async_kw:ident)?,
            instantiate_fn: $inst_fn:ident,
            call_fn: $call_fn:ident,
            await_token: $($await:tt)*
        ) => {
            		/// Runs goal and specific input.
            		///
            		/// Note that input has to include specific goal for function to run.
            		///
            		/// Input doesn't have to be a query - it can include modules etc.,
            		/// all of it is fed through --consult flat to WASM tpl binary through STDIN.
            		///
                pub $($async_kw)? fn run_prolog_with_fuel(
                    goal: &str,
                    input: &str,
                    fuel: u64,
                ) -> Result<String> {
                    // At this point runtime should be initialized
                    let runtime = Self::get_runtime();

                    let (wasi, stdout, stderr) = build_wasi(goal, input)?;
                    let mut store = Store::new(&runtime.engine, LinkerState { wasi });
                    store.set_fuel(fuel)?;

                    let instance = runtime
                        .linker
                        .$inst_fn(&mut store, &runtime.module)
                        $($await)*
                        ?;

                    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

                    start
                        .$call_fn(&mut store, ())
                        $($await)*
                        .map_err(|error| {
                            if error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
                                anyhow!("Prolog execution fuel exhausted")
                            } else {
                                error.into()
                            }
                        })?;

                    process_output(stdout, stderr)
                }

                pub $($async_kw)? fn run_prolog(goal: &str, input: &str) -> Result<String> {
                    Self::run_prolog_with_fuel(goal, input, DEFAULT_PROLOG_FUEL)
                        $($await)*
                }
            }
}
#[cfg(feature = "async")]
pub struct EngineAsync;
#[cfg(feature = "sync")]
pub struct Engine;

#[cfg(feature = "sync")]
impl Engine {
    get_runtime_impl!(
        runtime: RUNTIME_SYNC,
        linker_fn: add_to_linker_sync
    );

    /// Initialize engine
    pub fn init() {
        Self::get_runtime();
    }

    run_prolog_impl!(
        asyncness: ,
        instantiate_fn: instantiate,
        call_fn: call,
        await_token:
    );
}

#[cfg(feature = "async")]
impl EngineAsync {
    get_runtime_impl!(
        runtime: RUNTIME_ASYNC,
        linker_fn: add_to_linker_async
    );
    /// Initialize engine
    pub fn init() {
        Self::get_runtime();
    }
    run_prolog_impl!(
        asyncness: async,
        instantiate_fn: instantiate_async,
        call_fn: call_async,
        await_token: .await
    );
}

fn build_wasi(goal: &str, input: &str) -> Result<WasiCtxWithCtx> {
    let mut sb = String::new();
    writeln!(sb, "{}", input)?;
    let goal = format!("{}, halt", goal);

    let stdin = MemoryInputPipe::new(sb);
    let stdout = MemoryOutputPipe::new(65535);
    let stderr = MemoryOutputPipe::new(65535);

    let ctx = WasiCtxBuilder::new()
        .stdin(stdin)
        .stdout(stdout.clone())
        .stderr(stdout.clone())
        .args(&["tpl", "-q", "--consult", "-g", &goal])
        .preopened_dir(".", "/", DirPerms::READ, FilePerms::READ)
        .expect("Can't open current dir as root")
        .env("PWD", "/")
        .build_p1();
    Ok((ctx, stdout, stderr))
}

pub(crate) fn process_output(stdout: MemoryOutputPipe, stderr: MemoryOutputPipe) -> Result<String> {
    let output_bytes = stdout.contents();
    let output_str =
        String::from_utf8(output_bytes.to_vec()).context("Prolog output invalid UTF-8")?;

    let err_bytes = stderr.contents();
    let err_str = String::from_utf8(err_bytes.to_vec()).context("Prolog output invalid UTF-8")?;

    if !err_str.is_empty() {
        return Err(anyhow!("Stderr: {}", err_str));
    }
    if output_str.trim().starts_with("error(") {
        return Err(anyhow!(output_str));
    }
    if output_str.starts_with("Error:") {
        return Err(anyhow!(output_str));
    }
    Ok(output_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FUEL: u64 = 10_000_000;

    #[cfg(feature = "sync")]
    #[test]
    fn sync_default_fuel_allows_terminating_prolog() {
        assert_eq!(Engine::run_prolog("true", "").unwrap(), "");
    }

    #[cfg(feature = "sync")]
    #[test]
    fn sync_fuel_stops_non_terminating_prolog() {
        let error = Engine::run_prolog_with_fuel("loop", "loop :- loop.", TEST_FUEL)
            .expect_err("recursive Prolog should exhaust its fuel");

        assert_eq!(error.to_string(), "Prolog execution fuel exhausted");
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_default_fuel_allows_terminating_prolog() {
        assert_eq!(EngineAsync::run_prolog("true", "").await.unwrap(), "");
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_fuel_stops_non_terminating_prolog() {
        let error = EngineAsync::run_prolog_with_fuel("loop", "loop :- loop.", TEST_FUEL)
            .await
            .expect_err("recursive Prolog should exhaust its fuel");

        assert_eq!(error.to_string(), "Prolog execution fuel exhausted");
    }
}
