use std::io::{self, Read};

use crate::prolog::process_diagram_sync;
use anyhow::Result;

pub mod pikchr;
pub mod prolog;
use pikchr_pro::prolog_to_svg_string;
mod types;

fn main() -> io::Result<()> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    match prolog_to_svg_string(buffer) {
        Err(e) => eprintln!("Error: {}", e),
        Ok(result) => println!("{}", result),
    }
    Ok(())
}
