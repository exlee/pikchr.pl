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

use std::io::{self, Read};

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
