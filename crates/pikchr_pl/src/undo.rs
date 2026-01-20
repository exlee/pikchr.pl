use iced::widget::text_editor;

#[derive(Default)]
pub struct UndoStack {
    pub redo: Vec<text_editor::Content>,
    pub undo: Vec<text_editor::Content>,
}

impl UndoStack {
    pub fn new(initial_content: text_editor::Content) -> Self {
        Self {
            undo: vec![initial_content],
            redo: vec![],
        }
    }
    pub fn push(&mut self, content: &text_editor::Content) {
        self.redo.clear();
        self.undo.push(content.clone())
    }
    pub fn undo_into(&mut self, content: &mut text_editor::Content) {
        if self.undo.is_empty() {
            return;
        }
        if let Some(undo_layer) = self.undo.pop() {
            self.redo.push(undo_layer);
        }

        let previous_state: text_editor::Content = self
            .undo
            .last()
            .expect("Guaranteed by length check above")
            .to_owned();

        *content = previous_state;
    }
    pub fn redo_into(&mut self, content: &mut text_editor::Content) {
        if self.undo.len() < 2 {
            return;
        }
        if let Some(redo_layer) = self.redo.pop() {
            self.undo.push(redo_layer);
        }

        let previous_state: text_editor::Content = self
            .undo
            .last()
            .expect("Guaranteed by length check above")
            .to_owned();

        *content = previous_state;
    }
}
