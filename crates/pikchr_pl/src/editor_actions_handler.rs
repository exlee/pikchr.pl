use std::cmp::{max, min};

use iced::{
    Task,
    widget::text_editor::{Action, Cursor, Edit, Motion, Position},
};

use crate::{
    editor_state::Editor,
    messages::{EditorAction, Message},
};

const INDENT_SPACES: usize = 4;

pub fn handle(editor: &mut Editor, msg: EditorAction) -> Task<Message> {
    match msg {
        EditorAction::Dedent => {
            let cursor = editor.content.cursor();

            let line = editor
                .content
                .line(cursor.position.line)
                .expect("Line on cursor should exist.");
            let text = String::from(line.text);
            let removals = min(INDENT_SPACES, get_prefix_spaces(&text));

            editor.content.perform(Action::Move(Motion::Home));
            for _ in 0..removals {
                editor.content.perform(Action::Edit(Edit::Delete));
            }

            editor
                .content
                .move_to(shift_cursor_cols(cursor, -(removals as isize)));
            Task::none()
        },
        EditorAction::Indent => {
            let orig_cursor = editor.content.cursor();
            editor.content.perform(Action::Move(Motion::Home));
            for _ in 0..INDENT_SPACES {
                editor.content.perform(Action::Edit(Edit::Insert(' ')));
            }
            let new_cursor = shift_cursor_cols(orig_cursor, INDENT_SPACES as isize);

            editor.content.move_to(new_cursor);

            Task::none()
        },
        EditorAction::NewlineIndent => {
            let cursor = editor.content.cursor();

            let line = editor
                .content
                .line(cursor.position.line)
                .expect("Line on cursor should exist.");
            let text = String::from(line.text);
            let mut indent_to = get_prefix_spaces(&text);
            if let Some(arrow_index) = text.find("--> ") {
                indent_to = indent_to.max(arrow_index + 4);
            }
            if text.contains("-->") {
                indent_to = indent_to.max(2);
            }
            if cursor.position.column == 0 {
                indent_to = 0
            }
            editor.content.perform(Action::Edit(Edit::Enter));

            for _ in 0..indent_to{
                editor.content.perform(Action::Edit(Edit::Insert(' ')))
            };

            Task::none()

        },
    }
}

fn get_prefix_spaces(text: &str) -> usize {
    let mut spaces = 0;
    for c in text.chars() {
        if !c.is_whitespace() {
            break;
        }
        spaces += 1;
    }
    spaces
}
fn shift_cursor_cols(orig: Cursor, by: isize) -> Cursor {
    let new_column = max(0, orig.position.column as isize + by) as usize;

    Cursor {
        position: Position {
            line: orig.position.line,
            column: new_column,
        },
        selection: None,
    }
}
