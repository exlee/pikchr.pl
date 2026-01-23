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

pub trait StringExt {
    fn trim_last_lines(self, lines: usize) -> Self;
    fn trim_last_chars(self, chars: usize) -> Self;
}

impl StringExt for String {
    fn trim_last_lines(self, lines: usize) -> Self {
        let mut reversed = Vec::new();
        for (idx, line) in self.lines().rev().enumerate() {
            if idx >= lines {
                break;
            }
            reversed.push(line);
        }
        reversed.into_iter().rev().collect::<Vec<_>>().join("\n")
    }

    fn trim_last_chars(self, chars: usize) -> Self {
        let mut reversed = Vec::new();
        for (idx, c) in self.chars().rev().enumerate() {
            if idx >= chars {
                break;
            }
            reversed.push(c)
        }
        let mut new_string = String::new();
        for c in reversed.into_iter().rev() {
            new_string.push(c)
        }
        new_string
    }
}
