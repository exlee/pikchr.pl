use std::{fmt::Write, sync::OnceLock};

use anyhow::{Context, Result, anyhow};
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::{
    WasiCtxBuilder,
    p1::{self, WasiP1Ctx},
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

use crate::{
    prolog::{
        LinkerState,
        PROLOG_INIT,
        PrologRuntime,
        RenderError,
        TPL_WASM,
        build_wasi,
        process_output,
    },
    types::{PikchrCode, *},
};

static RUNTIME_ASYNC: OnceLock<PrologRuntime> = OnceLock::new();

pub fn init() {
    get_runtime();
}
fn get_runtime() -> &'static PrologRuntime {
    RUNTIME_ASYNC.get_or_init(|| {
        let mut config = wasmtime::Config::new();
        config.async_support(true);
        let engine = Engine::new(&config).expect("Failed to create async engine");
        let module = Module::new(&engine, TPL_WASM).expect("Failed to compile WASM");

        // Build Linker once
        let mut linker = Linker::new(&engine);
        p1::add_to_linker_async(&mut linker, |s: &mut LinkerState| &mut s.wasi)
            .expect("Failed to link WASI");

        PrologRuntime {
            engine,
            module,
            linker,
        }
    })
}
pub async fn run_prolog(input: &str) -> Result<String, RenderError> {
    let runtime = get_runtime();

    let (wasi, stdout, stderr) = build_wasi(input)?;

    let mut store = Store::new(&runtime.engine, LinkerState { wasi });

    let instance = runtime
        .linker
        .instantiate_async(&mut store, &runtime.module)
        .await?;

    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
    start.call_async(&mut store, ()).await?;

    process_output(stdout, stderr)
}

pub async fn process_diagram(input: String) -> Result<PikchrCode, RenderError> {
    let mut data = String::new();
    let _ = writeln!(data, "{}", PROLOG_INIT);
    let _ = writeln!(data, "run :- phrase(diagram, Out), format(\"~s\", [Out]).");
    let _ = writeln!(data, "{}", input);
    run_prolog(&data)
        .await
        .map_err(|e| RenderError::PrologError(format!("{}", e)))
        .map(|s| PikchrCode::new(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    macro_rules! prolog_test {
        ($name: ident, $inp: literal, $out: literal) => {
            #[tokio::test]
            async fn $name() {
                let input = $inp;
                let expectation = $out;
                let got = process_diagram(String::from(input))
                    .await
                    .unwrap()
                    .into_inner();
                assert_eq!(got, expectation.trim());
            }
        };
    }
    prolog_test!(async_test_1, r#"diagram --> circle."#, "circle;");
    prolog_test!(
        async_test_2,
        r#"diagram --> circle("Test")."#,
        r#"circle "Test";"#
    );
    prolog_test!(
        async_test_3,
        r#"diagram --> circle("Test", fill("red"))."#,
        r#"circle "Test" fill red;"#
    );
    prolog_test!(
        async_test_4,
        r#"diagram --> circle("Test", "fill red")."#,
        r#"circle "Test" fill red;"#
    );
    prolog_test!(
        async_test_5,
        r#"diagram --> text("Test", small)."#,
        r#"text "Test" small;"#
    );
    #[tokio::test]
    async fn can_process_basic_string() {
        let input = String::from("run :- Value = 1, write(Value).");
        let expectation = "1";
        let got = run_prolog(input.as_str()).await.unwrap();

        assert_eq!(got, expectation);
    }

    #[tokio::test]
    async fn can_create_basic_diagram() {
        let input = String::from("diagram --> \"box;\", \"arrow;\", \"box\".");
        let got = process_diagram(input).await.unwrap().into_inner();
        let expectation = "box;arrow;box";

        assert_eq!(got, expectation);
    }
}
