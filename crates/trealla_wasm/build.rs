use std::{env, fs, path::PathBuf};
use wasmtime::{Config, Engine};

fn main() {
    build_optimized_wasm();
}

fn build_optimized_wasm() {
    println!("cargo:rustc-check-cfg=cfg(precompiled_wasm)");
    println!("cargo:rerun-if-changed=native/tpl/tpl.wasm");

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(out_dir).join("tpl.bin");
    let wasm_path = PathBuf::from("native/tpl/tpl.wasm");
    let wasm_bytes = fs::read(&wasm_path).expect("Could not read WASM file");

    let target_triple = env::var("TARGET").unwrap();

    let mut config = Config::new();
    config.cranelift_opt_level(wasmtime::OptLevel::Speed);

    if let Err(e) = config.target(&target_triple) {
        println!(
            "cargo:warning=Wasmtime target '{}' not supported: {}. Falling back to raw WASM.",
            target_triple, e
        );
        fs::write(&out_path, &wasm_bytes).expect("Failed to write raw wasm");
        return;
    }

    let engine = Engine::new(&config).expect("Failed to create build-time engine");

    match engine.precompile_module(&wasm_bytes) {
        Ok(compiled) => {
            fs::write(&out_path, compiled).expect("Failed to write cwasm");
            println!("cargo:rustc-cfg=precompiled_wasm"); // Enable deserialization path
        },
        Err(e) => {
            println!(
                "cargo:warning=Precompilation failed for {}: {}. Falling back to raw WASM.",
                target_triple, e
            );
            fs::write(&out_path, &wasm_bytes).expect("Failed to write raw wasm");
        },
    }
}
