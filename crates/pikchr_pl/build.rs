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
//
use std::{env, io::Write, path::PathBuf};

fn main() {
    integrate_prolog_modules();
}
fn integrate_prolog_modules() {
    let out_dir = env::var("OUT_DIR").map(PathBuf::from).unwrap();
    let out_path = out_dir.join("prolog_modules.rs");

    let modules_path = env::current_dir()
        .expect("Can't get CWD")
        .join("native/prolog");

    let mut output_file = std::fs::File::create(out_path).unwrap();
    writeln!(
        output_file,
        "pub static PROLOG_MODULES: &[(&str,&str)] = &["
    ).unwrap();
    if let Ok(entries) = std::fs::read_dir(modules_path) {
        for file in entries.flatten() {
            let path = file.path();
            if path.extension().is_some_and(|ext| ext == "pl") {
                let basename = path.file_stem().unwrap().to_str().unwrap();
                let file_path= path.canonicalize().unwrap().to_string_lossy().replace("\\", "/");
                writeln!(output_file, r#"  ("{}", include_str!("{}")),"#, basename, file_path).unwrap();
            }
        }
    }
    writeln!(output_file, "];").unwrap();
}
