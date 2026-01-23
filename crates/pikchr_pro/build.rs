// This file is part of pikchr.pl.
//
// pikchr.pl is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// pikchr.pl is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with pikchr.pl. If not, see <https://www.gnu.org/licenses/>.

use std::{env, fs, path::PathBuf};
use wasmtime::{Config, Engine};

fn main() {
    build_pikchr();
    build_optimized_wasm();
}


fn build_pikchr() {
    cc::Build::new()
        .file("native/pikchr/pikchr.c")
        .compile("pikchr");

    println!("cargo:rerun-if-changed=native/pikchr/pikchr.c");
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
