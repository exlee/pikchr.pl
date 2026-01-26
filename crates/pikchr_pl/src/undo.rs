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

use iced::widget::text_editor::{self, Cursor, Position};

#[derive(Debug, Clone)]
struct UndoContent {
    content: text_editor::Content,
    position: Position,
}
#[derive(Default, Debug)]
pub struct UndoStack {
    pub redo: Vec<UndoContent>,
    pub undo: Vec<UndoContent>,
}

impl From<text_editor::Content> for UndoContent {
    fn from(value: text_editor::Content) -> Self {

        Self {
            position: value.cursor().position,
            content: value
        }
    }
}

impl From<&text_editor::Content> for UndoContent {
    fn from(value: &text_editor::Content) -> Self {

        Self {
            position: value.cursor().position,
            content: value.clone()
        }
    }
}

impl UndoStack {
    pub fn new(initial_content: text_editor::Content) -> Self {
        Self {
            undo: vec![initial_content.into()],
            redo: vec![],
        }
    }
    pub fn push(&mut self, content: &text_editor::Content) {
        self.redo.clear();
        self.undo.push(content.into())
    }
    pub fn undo_into(&mut self, content: &mut text_editor::Content) {
        if self.undo.len()  <= 1 {
            *content = self.undo.last().unwrap().content.clone();
            return;
        }
        if let Some(undo_layer) = self.undo.pop() {
            self.redo.push(undo_layer);
        }

        let previous_state: &UndoContent = self
            .undo
            .last()
            .expect("Guaranteed by length check above");

        *content = previous_state.content.clone();
        let new_cursor = Cursor {
            position: previous_state.position,
            selection: None
        };
        content.move_to(new_cursor);
    }
    pub fn redo_into(&mut self, content: &mut text_editor::Content) {
        if let Some(redo_layer) = self.redo.pop() {

            *content = redo_layer.content.clone();
            let new_cursor = Cursor {
                position: redo_layer.position,
                selection: None
            };
            self.undo.push(redo_layer);
            content.move_to(new_cursor);
        } 
    }
}
