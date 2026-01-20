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

use std::path::PathBuf;

use iced::{
    keyboard::Modifiers,
    widget::{svg, text_editor},
};
use pikchr_pro::types::PikchrCode;
use tokio::sync::watch;

use crate::OperatingMode;

pub const INITIAL_CONTENT: &str = r#"diagram -->
  box("Hello").
"#;

pub struct Editor {
    pub input_tx:        watch::Sender<PikchrCode>,
    pub input_rx:        watch::Receiver<PikchrCode>,
    pub content:         text_editor::Content,
    pub svg_handle:      Option<svg::Handle>,
    pub is_compiling:    bool,
    pub last_successful: bool,
    pub operating_mode:  OperatingMode,
    pub modifiers:       Modifiers,
    pub last_error:      Buffered<String>,
    pub current_file:    Option<PathBuf>,
    pub dirty:           bool,
}

impl Default for Editor {
    fn default() -> Self {
        let (tx, rx) = watch::channel(PikchrCode::new(""));
        Self {
            input_tx:        tx,
            input_rx:        rx,
            content:         text_editor::Content::with_text(INITIAL_CONTENT),
            svg_handle:      None,
            is_compiling:    false,
            last_successful: false,
            operating_mode:  OperatingMode::PrologMode,
            modifiers:       Modifiers::default(),
            last_error:      Buffered::new(String::new()),
            current_file:    None,
            dirty:           true,
        }
    }
}

pub struct Buffered<T: Clone> {
    current: T,
    cached:  T,
}

impl<T: Clone> Buffered<T> {
    pub fn new(init: T) -> Self {
        Self {
            cached:  init.clone(),
            current: init,
        }
    }
    pub fn set(&mut self, value: T) {
        self.current = value;
    }
    pub fn get(&self) -> T {
        self.cached.clone()
    }
    pub fn commit(&mut self) {
        self.cached = self.current.clone();
    }
}
