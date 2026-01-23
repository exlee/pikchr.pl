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


use crate::{
    prolog::{DIAGRAM_INIT,  Queries, RenderError},
    types::PikchrCode,
};


#[macro_export]
macro_rules! process_diagram_impl {
    (
        func: $func:ident,
        async_: $($async_kw:ident)?,
        await_: $($await_token:tt)*
    ) => {
            pub $($async_kw)? fn process_diagram(input: Queries) -> Result<PikchrCode, RenderError> {
                let mut diagram_input = input.clone();
                diagram_input.insert(0, String::from(DIAGRAM_INIT));
                let diagram_input = diagram_input.iter().cloned().collect::<Vec<_>>().join("\n");


                trealla_wasm::$func::run_prolog("run", &diagram_input)
                $($await_token)*
                .map_err(|e| RenderError::PrologError(format!("{}", e)))
                .map(PikchrCode::new)
            }
    };
}

#[cfg(feature = "sync")]
pub struct Engine{}
#[cfg(feature = "async")]
pub struct EngineAsync{}

impl Engine {
    pub fn init() {
        trealla_wasm::Engine::init();
    }

    process_diagram_impl!(
        func: Engine,
        async_: ,
        await_:
    );
}
impl EngineAsync {
    pub fn init() {
        trealla_wasm::EngineAsync::init();
    }

    process_diagram_impl!(
        func: EngineAsync,
        async_: async,
        await_: .await
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! prolog_test {
        ($name: ident, $inp:literal, $out:literal) => {
            mod $name {
                use super::*;
                prolog_test!(@common test,,[ ],Engine,sync_version,$inp,$out);
                prolog_test!(@common tokio::test,async,[.await],EngineAsync,async_version, $inp, $out);
            }
        };
        (@common $test_type:meta,$($async_kw:ident)?,[$($await_token:tt)*],$module:ident,$name:ident, $inp: literal, $out: literal) => {
            #[$test_type]
            $($async_kw)? fn $name() {
                let input = $inp;
                let expectation = $out;
                let got = $module::process_diagram(vec![String::from(input)])
                    $($await_token)*
                    .unwrap()
                    .into_inner();
                assert_eq!(got, expectation.trim());
            }
        }
    }
    prolog_test!(
        test_1,
        r#"
circle --> "circle;".
diagram --> circle.
    "#,
        "circle;"
    );
    prolog_test!(
        test_2,
        r#"
circle(Name) --> "circle", " \"", Name, "\";".
diagram --> circle("Test").
    "#,
        r#"circle "Test";"#
    );
    prolog_test!(
        test_3,
        r#"
fill(C) --> "fill ", C.
circle(N,A) --> "circle \"", N, "\" ", A, ";".
diagram --> circle("Test", fill("red")).

    "#,
        r#"circle "Test" fill red;"#
    );
    prolog_test!(
        test_4,
        r#"
circle(N,A) --> "circle \"", N, "\" ", A, ";".
diagram --> circle("Test", "fill red").
    "#,
        r#"circle "Test" fill red;"#
    );
    prolog_test!(
        test_5,
        r#"
small --> "small".
text(N,A) --> "text \"", N, "\" ", A, ";".
diagram --> text("Test", small).
    "#,
        r#"text "Test" small;"#
    );
    prolog_test!(
        test_6,
        r#"diagram --> "box;", "arrow;", "box"."#,
        "box;arrow;box"
    );
}
