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
    p1,
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

use crate::{
    get_runtime_impl,
    process_diagram_impl,
    prolog::{
        self,
        LinkerState,
        PrologRuntime,
        Queries,
        RenderError,
        TPL_WASM,
        build_wasi,
        process_output,
    },
    run_prolog_impl,
    types::PikchrCode,
};

static RUNTIME_SYNC: OnceLock<PrologRuntime> = OnceLock::new();

pub fn init() {
    get_runtime();
}

get_runtime_impl!(
    runtime: RUNTIME_SYNC,
    async_support: false,
    linker_fn: add_to_linker_sync
);
process_diagram_impl!(
    async_: ,
    await_:
);
run_prolog_impl!(
    asyncness: ,
    instantiate_fn: instantiate,
    call_fn: call,
    await_token:
);
