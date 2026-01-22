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

use std::path::PathBuf;

use iced::{keyboard::Modifiers, widget::{pane_grid, text_editor}};
use pikchr_pro::types::PikchrCode;

use crate::{ApplicationError, OperatingMode};

#[derive(Debug, Clone, Copy)]
pub enum EditorAction {
    Dedent,
    Indent,
    NewlineIndent,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Destructive event coming from editor
    Edit(text_editor::Action),
    LoadFileSelected(Option<PathBuf>),
    LoadRequested,
    ModifiersChanged(Modifiers),
    NewRequested,
    PerformAction(text_editor::Action),
    PerformActions(bool, Vec<text_editor::Action>),
    PikchrFinished(Option<Result<String, ApplicationError>>),
    PrologFinished(Result<PikchrCode, ApplicationError>),
    RadioSelected(OperatingMode),
    RefreshTick,
    RunLogic,
    RunPikchr(PikchrCode),
    RunProlog(String),
    SaveFileSelected(Option<PathBuf>),
    SaveFinished,
    SaveRequested,
    SaveAsRequested,
    ShowError(ApplicationError),
    ToggleDebugOverlay,
    Undo,
    Redo,
    ToggleFileWatch,
    PaneResized(pane_grid::ResizeEvent),
    EditorAction(EditorAction),
    LoadedFileChanged
}
