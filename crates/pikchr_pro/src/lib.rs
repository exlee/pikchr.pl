use std::io::{self, Read};

use crate::prolog::process_diagram_sync;
use anyhow::Result;

pub mod pikchr;
pub mod prolog;
pub mod types;

pub fn prolog_to_svg_string(input: String) -> Result<String, String> {
    let result = process_diagram_sync(input)?;
    let svg = pikchr::render_pikchr(result)?;
    Ok(svg.into_inner())
}
