// This file is part of pikchr.pl.
//
// pikchr.pl is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// pikchr.pl is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with pikchr.pl. If not, see <https://www.gnu.org/licenses/>.

use thiserror::Error;

use crate::types::PikchrCode;

pub mod engine;

pub(crate) static PROLOG_INIT: &str = include_str!("../native/prolog/init.pl");

type Queries = Vec<String>;

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

macro_rules! future_type {
    ($T:ty) => {
			impl std::future::Future<Output = $T> + Send
    }
}
pub trait PrologEngine {
    fn process_diagram(input: Queries) -> Result<PikchrCode, RenderError>;
    fn run_prolog(input: Queries) -> Result<String, RenderError>;
}

pub trait PrologEngineAsync {
    fn process_diagram(input: Queries) -> future_type!(Result<PikchrCode,RenderError>);
    fn run_prolog(input: Queries) -> future_type!(Result<String, RenderError>);
}

pub trait PrologInit {
    fn init(init_data: Option<String>);
}
