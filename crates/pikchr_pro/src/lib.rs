use std::io::{self, Read};

use anyhow::Result;

use crate::prolog::{RenderError, sync::process_diagram};

pub mod pikchr;
pub mod prolog;
pub mod types;

pub fn prolog_to_svg_string(input: String) -> Result<String, RenderError> {
    let result = process_diagram(vec![input])?;
    let svg = pikchr::render_pikchr(result)?;
    Ok(svg.into_inner())
}
