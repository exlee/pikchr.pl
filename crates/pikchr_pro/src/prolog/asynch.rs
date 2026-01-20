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

use std::{fmt::Write, sync::OnceLock};

use anyhow::{Context, Result, anyhow};
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::{
    WasiCtxBuilder,
    p1::{self, WasiP1Ctx},
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

use crate::{
    get_runtime_impl,
    process_diagram_impl,
    prolog::{
        LinkerState,
        PrologRuntime,
        Queries,
        RenderError,
        TPL_WASM,
        build_wasi,
        process_output,
    },
    run_prolog_impl,
    types::{PikchrCode, *},
};

static RUNTIME_ASYNC: OnceLock<PrologRuntime> = OnceLock::new();

pub fn init() {
    get_runtime();
}

get_runtime_impl!(
    runtime: RUNTIME_ASYNC,
    async_support: true,
    linker_fn: add_to_linker_async
);
run_prolog_impl!(
    asyncness: async,
    instantiate_fn: instantiate_async,
    call_fn: call_async,
    await_token: .await
);

process_diagram_impl!(
    async_: async,
    await_: .await
);
