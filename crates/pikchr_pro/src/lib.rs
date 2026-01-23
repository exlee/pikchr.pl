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

use anyhow::Result;

use crate::prolog::{RenderError, engine};

pub mod pikchr;
pub mod prolog;
pub mod types;

pub fn prolog_to_svg_string(input: String) -> Result<String, RenderError> {
    engine::trealla::Engine::init();
    let result = engine::trealla::Engine::process_diagram(vec![input])?;
    let svg = pikchr::render_pikchr(result)?;
    Ok(svg.into_inner())
}
