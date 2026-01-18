use std::{fmt::Write, sync::OnceLock};

use anyhow::{Context, Result, anyhow};
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::{
    WasiCtxBuilder, p1,
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

use crate::{
    prolog::{LinkerState, PROLOG_INIT, PrologRuntime, TPL_WASM},
    types::PikchrCode,
};

static RUNTIME_SYNC: OnceLock<PrologRuntime> = OnceLock::new();

pub fn init() {
    get_runtime();
}

pub fn get_runtime() -> &'static PrologRuntime {
    RUNTIME_SYNC.get_or_init(|| {
        let mut config = wasmtime::Config::new();
        config.async_support(false);
        let engine = Engine::new(&config).expect("Failed to create sync engine");
        let module = Module::new(&engine, TPL_WASM).expect("Failed to compile WASM");

        // Build Linker once
        let mut linker = Linker::new(&engine);
        p1::add_to_linker_sync(&mut linker, |s: &mut LinkerState| &mut s.wasi)
            .expect("Failed to link WASI");

        PrologRuntime {
            engine,
            module,
            linker,
        }
    })
}

pub fn run_prolog(input: &str) -> Result<String> {
    use crate::prolog::PROLOG_INIT;

    let runtime = get_runtime();
    let mut sb = String::new();
    writeln!(sb, "{}", PROLOG_INIT)?;
    writeln!(sb, "{}", input)?;

    let stdin = MemoryInputPipe::new(sb);
    let stdout = MemoryOutputPipe::new(65535);

    let wasi = WasiCtxBuilder::new()
        .stdin(stdin)
        .stdout(stdout.clone())
        .args(&["tpl", "-q", "--consult", "-g", "run, halt"])
        .inherit_stderr()
        .build_p1();

    let mut store = Store::new(&runtime.engine, LinkerState { wasi });

    let instance = runtime.linker.instantiate(&mut store, &runtime.module)?;

    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
    start.call(&mut store, ())?;

    let output_bytes = stdout.contents();
    let output_str =
        String::from_utf8(output_bytes.to_vec()).context("Prolog output invalid UTF-8")?;

    if output_str.contains("Error: ") {
        Err(anyhow!(output_str))
    } else {
        Ok(output_str)
    }
}
pub fn process_diagram(input: String) -> Result<PikchrCode, String> {
    let mut data = String::new();
    let _ = writeln!(data, "{}", PROLOG_INIT);
    let _ = writeln!(data, "run :- phrase(diagram, Out), format(\"~s\", [Out]).");
    let _ = writeln!(data, "{}", input);
    run_prolog(&data)
        .map_err(|e| format!("{}", e))
        .map(|s| PikchrCode::new(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! prolog_test {
        ($name: ident, $inp: literal, $out: literal) => {
            #[test]
            fn $name() {
                let input = $inp;
                let expectation = $out;
                let got = process_diagram(String::from(input)).unwrap().into_inner();
                assert_eq!(got, expectation.trim());
            }
        };
    }
    prolog_test!(sync_test_1, r#"diagram --> circle."#, "circle;");
    prolog_test!(
        sync_test_2,
        r#"diagram --> circle("Test")."#,
        r#"circle "Test";"#
    );
    prolog_test!(
        sync_test_3,
        r#"diagram --> circle("Test", fill("red"))."#,
        r#"circle "Test" fill red;"#
    );
    prolog_test!(
        sync_test_4,
        r#"diagram --> circle("Test", "fill red")."#,
        r#"circle "Test" fill red;"#
    );
    prolog_test!(
        sync_test_5,
        r#"diagram --> text("Test", small)."#,
        r#"text "Test" small;"#
    );
}
