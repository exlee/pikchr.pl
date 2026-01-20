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

pub use crate::pikchr::PikchrCode;
macro_rules! string_newtype {
    ($name: ident) => {
        pub struct $name(String);
        impl $name {
            pub fn new<T>(input: T) -> Self
            where
                T: AsRef<str>,
            {
                Self(String::from(input.as_ref()))
            }
            pub fn as_inner(&self) -> &String {
                &self.0
            }
            pub fn into_inner(self) -> String {
                self.0
            }
        }
    };
}
string_newtype!(SvgString);
string_newtype!(PrologCode);
