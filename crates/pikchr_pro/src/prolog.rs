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

pub mod engine;

pub(crate) static DIAGRAM_INIT: &str = include_str!("../native/prolog/init.pl");

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

