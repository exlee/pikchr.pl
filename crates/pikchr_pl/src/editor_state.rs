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

use std::{collections::HashMap, path::PathBuf};

use iced::{
    keyboard::Modifiers,
    widget::{pane_grid, svg, text_editor},
};
use pikchr_pro::types::PikchrCode;
use tokio::sync::watch;

use crate::{OperatingMode, PaneContent, constants, prolog_modules, undo::UndoStack};

pub const INITIAL_CONTENT: &str = r#"diagram -->
  box("Hello").
"#;

pub struct Editor {
    pub modules: prolog_modules::PrologModules,
    pub pikchr_input_tx:        watch::Sender<PikchrCode>,
    pub pikchr_input_rx:        watch::Receiver<PikchrCode>,
    pub prolog_input_tx:        watch::Sender<String>,
    pub prolog_input_rx:        watch::Receiver<String>,
    pub content:         text_editor::Content,
    pub svg_handle:      Option<svg::Handle>,
    pub is_compiling:    bool,
    pub last_successful: bool,
    pub operating_mode:  OperatingMode,
    pub modifiers:       Modifiers,
    pub last_error:      Buffered<String>,
    pub current_file:    Option<PathBuf>,
    pub show_debug:      bool,
    pub pikchr_code:     Option<PikchrCode>,
    pub dirty:           bool,
    pub undo_stack:      UndoStack,
    pub panes: pane_grid::State<PaneContent>,
    pub file_watch_mode: bool,
}

impl Default for Editor {
    fn default() -> Self {
        let (piktx, pikrx) = watch::channel(PikchrCode::new(""));
        let (prtx, prrx) = watch::channel(String::new());
        let content = text_editor::Content::with_text(INITIAL_CONTENT);

        let (mut pane_state, main_pane) = pane_grid::State::new(PaneContent::Editor);
        pane_state.split(
            pane_grid::Axis::Vertical,
            main_pane,
            PaneContent::Preview
        );

        Self {
            modules: prolog_modules::PrologModules::new(),
            undo_stack: UndoStack::new(content.clone()),
            pikchr_input_tx: piktx,
            pikchr_input_rx: pikrx,
            prolog_input_tx: prtx,
            prolog_input_rx: prrx,
            svg_handle: None,
            is_compiling: false,
            last_successful: false,
            operating_mode: OperatingMode::PrologMode,
            modifiers: Modifiers::default(),
            last_error: Buffered::new(String::new()),
            current_file: None,
            dirty: true,
            show_debug: false,
            pikchr_code: None,
            file_watch_mode: false,
            content,
            panes: pane_state
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
