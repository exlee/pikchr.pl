use eframe::egui::{self, Context, Ui, text_edit::TextEditOutput};
use tokio::sync::{mpsc::Sender, watch};

use crate::{
    Msg, SvgbobEditMode,
    editor::{self, GenericEditor, HandleEnter as _},
    icons::{AppIcon, icon_button},
    impl_generated_content, impl_id, impl_indexable, impl_initialize, impl_initialize_tx,
    impl_target, impl_visible,
    mini_window::{
        self, EditorWindow, HasMenu, HasName as _, MiniWindow, RawContent, RenderToggle,
    },
    setter_getter_for_trait,
};

/// A dedicated ASCII-art editor. It is permanently rendered by Svgbob and has
/// its own binding hooks, separate from Pikchr and the generator editors.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SvgbobEditor {
    pub id: egui::Id,
    target_svg: egui::Id,
    pub(crate) visible: bool,
    pub(crate) content: String,
    pub(crate) index: usize,
    #[serde(skip_serializing, default)]
    initialized: bool,
    #[serde(skip)]
    watch_tx: Option<watch::Sender<(egui::Context, egui::Id, String)>>,
    error: Option<String>,
    name: String,
    #[serde(default = "default_render")]
    pub(crate) render: bool,
    #[serde(default)]
    mode: SvgbobEditMode,
}

fn default_render() -> bool {
    true
}

impl SvgbobEditor {
    pub fn new(id: egui::Id, target_svg: egui::Id) -> Self {
        Self {
            id,
            target_svg,
            visible: true,
            content: String::new(),
            index: 1,
            initialized: false,
            watch_tx: None,
            error: None,
            name: id.short_debug_format(),
            render: true,
            mode: SvgbobEditMode::default(),
        }
    }

    pub(crate) fn set_edit_mode(&mut self, mode: SvgbobEditMode) {
        self.mode = mode;
    }

    #[cfg(test)]
    pub(crate) fn edit_mode(&self) -> SvgbobEditMode {
        self.mode
    }

    fn move_canvas_cursor(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        editor_id: egui::Id,
        direction: CanvasDirection,
    ) -> bool {
        let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) else {
            return false;
        };
        let Some(range) = state.cursor.char_range() else {
            return false;
        };

        let (row, column) = grid_position(&self.content, range.primary.index);
        let (target_row, target_column) = match direction {
            CanvasDirection::Up => (row.saturating_sub(1), column),
            CanvasDirection::Down => (row + 1, column),
            CanvasDirection::Left => (row, column.saturating_sub(1)),
            CanvasDirection::Right => (row, column + 1),
        };
        let (content_changed, target) = move_to_grid(&mut self.content, target_row, target_column);

        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(target),
            )));
        state.store(ui.ctx(), editor_id);
        content_changed
    }

    fn replace_input(&self, ctx: &Context, ui: &Ui, editor_id: egui::Id) -> Option<ReplaceInput> {
        if self.mode != SvgbobEditMode::Replace || !ui.memory(|memory| memory.has_focus(editor_id))
        {
            return None;
        }
        let range = egui::TextEdit::load_state(ctx, editor_id)?
            .cursor
            .char_range()?;
        // Egui already has the correct semantics for replacing a selection:
        // remove only the selected text, then insert the new text.
        if !range.is_empty() {
            return None;
        }
        let text =
            ui.input(|input| {
                let mut text = String::new();
                for event in &input.events {
                    match event {
                        // TextEdit itself ignores Enter text events because it
                        // receives a distinct Key::Enter event.
                        egui::Event::Text(input) if input != "\n" && input != "\r" => {
                            text.push_str(input);
                        },
                        egui::Event::Paste(input) => text.push_str(input),
                        // IME maintains its own temporary selection. Let egui
                        // own it until a dedicated IME-aware Replace path exists.
                        egui::Event::Ime(_) => return None,
                        egui::Event::Cut
                        | egui::Event::Key {
                            key:
                                egui::Key::Backspace
                                | egui::Key::Delete
                                | egui::Key::Enter
                                | egui::Key::Tab,
                            pressed: true,
                            ..
                        } => return None,
                        _ => {},
                    }
                }
                (!text.is_empty()).then_some(text)
            })?;

        Some(ReplaceInput {
            content: self.content.clone(),
            range,
            text,
        })
    }
}

struct ReplaceInput {
    content: String,
    range: egui::text::CCursorRange,
    text: String,
}

#[derive(Clone, Copy)]
enum CanvasDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Return the row and column of a character cursor in the ASCII-art grid.
fn grid_position(content: &str, cursor: usize) -> (usize, usize) {
    let mut row = 0;
    let mut column = 0;
    for (index, character) in content.chars().enumerate() {
        if index == cursor {
            return (row, column);
        }
        if character == '\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    (row, column)
}

/// Move a text buffer's materialized grid boundary to `row`, `column`.
///
/// Canvas navigation is allowed beyond ragged lines and the final row. We
/// represent the newly reachable cells as spaces/newlines so egui's TextEdit
/// can continue to own selection, IME, clipboard, and text input handling.
fn move_to_grid(content: &mut String, row: usize, column: usize) -> (bool, usize) {
    let mut changed = false;
    let current_rows = content
        .chars()
        .filter(|character| *character == '\n')
        .count()
        + 1;
    for _ in current_rows..=row {
        content.push('\n');
        changed = true;
    }

    let mut line_start_byte = 0;
    let mut line_start_cursor = 0;
    for _ in 0..row {
        let next_newline = content[line_start_byte..]
            .find('\n')
            .expect("row was materialized above");
        let next_line_start = line_start_byte + next_newline + 1;
        line_start_cursor += content[line_start_byte..next_line_start].chars().count();
        line_start_byte = next_line_start;
    }

    let line_end_byte = content[line_start_byte..]
        .find('\n')
        .map(|offset| line_start_byte + offset)
        .unwrap_or(content.len());
    let line_length = content[line_start_byte..line_end_byte].chars().count();
    if column > line_length {
        for _ in line_length..column {
            content.insert(line_end_byte, ' ');
        }
        changed = true;
    }

    (changed, line_start_cursor + column)
}

fn byte_index(content: &str, character_index: usize) -> usize {
    content
        .char_indices()
        .nth(character_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(content.len())
}

/// Apply text in Replace mode: each non-newline character overwrites one
/// canvas cell, then the block cursor advances to the next cell.
fn replace_text(
    mut content: String,
    range: egui::text::CCursorRange,
    text: &str,
) -> (String, usize) {
    debug_assert!(
        range.is_empty(),
        "Replace mode handles selections in TextEdit"
    );
    let (mut row, mut column) = grid_position(&content, range.primary.index);
    for character in text.chars() {
        match character {
            '\r' => {},
            '\n' => {
                row += 1;
                column = 0;
            },
            character => {
                // Materialize the target cell, not just its preceding cursor
                // position, so replacing at an end-of-line grows the canvas.
                let (_, after_cell) = move_to_grid(&mut content, row, column + 1);
                let cell = after_cell - 1;
                let start_byte = byte_index(&content, cell);
                let end_byte = byte_index(&content, cell + 1);
                content.replace_range(start_byte..end_byte, &character.to_string());
                column += 1;
            },
        }
    }
    let (_, cursor) = move_to_grid(&mut content, row, column);
    (content, cursor)
}

impl EditorWindow for SvgbobEditor {
    fn get_editor_window(&self) -> mini_window::EditorWindowView<'_> {
        mini_window::EditorWindowView {
            index: &self.index,
            id: &self.id,
            content: self as &dyn mini_window::GeneratedContent,
            editor_type: self as &dyn mini_window::EditorType,
            mini_window: self as &dyn MiniWindow,
            name: &self.name,
        }
    }
}

impl HasMenu for SvgbobEditor {
    fn has_menu(&self) -> bool {
        true
    }

    fn menu(&self, ui: &mut Ui, tx: Sender<Msg>) {
        let icon = match self.mode {
            SvgbobEditMode::Insert => AppIcon::InsertMode,
            SvgbobEditMode::Replace => AppIcon::ReplaceMode,
        };
        let next_mode = self.mode.toggled();
        if icon_button(ui, icon)
            .on_hover_text(format!(
                "{} mode\nSwitch to {}",
                self.mode.label(),
                next_mode.label()
            ))
            .clicked()
        {
            let _ = tx.try_send(Msg::SetSvgbobEditMode(self.id, next_mode));
        }
    }
}

impl GenericEditor for SvgbobEditor {
    fn editor_spec(&mut self, editor_id: egui::Id, ui: &mut Ui) -> TextEditOutput {
        ui.scope(|ui| {
            // TextEdit only has a bar cursor. Hide it and paint a full
            // monospace cell after the widget so the dedicated canvas editor
            // has a conventional block cursor.
            ui.visuals_mut().text_cursor.stroke.color = egui::Color32::TRANSPARENT;
            ui.visuals_mut().text_cursor.blink = false;

            let replace_input = self.replace_input(ui.ctx(), ui, editor_id);

            let mut output = egui::TextEdit::multiline(&mut self.content)
                .code_editor()
                .desired_width(f32::INFINITY)
                .id(editor_id)
                .show(ui);

            if let Some(replace_input) = replace_input {
                let (content, cursor) = replace_text(
                    replace_input.content,
                    replace_input.range,
                    &replace_input.text,
                );
                self.content = content;
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), editor_id) {
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(cursor),
                        )));
                    state.store(ui.ctx(), editor_id);
                }
                output.response.mark_changed();
            }

            if ui.memory(|memory| memory.has_focus(editor_id))
                && let Some(cursor_range) = output.cursor_range
                && cursor_range.is_empty()
            {
                let cursor_rect = output
                    .galley
                    .pos_from_cursor(cursor_range.primary)
                    .translate(output.galley_pos.to_vec2());
                let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                let cell_width = ui.fonts_mut(|fonts| fonts.glyph_width(&font_id, ' '));
                let cell = egui::Rect::from_min_size(
                    cursor_rect.min,
                    egui::vec2(cell_width, cursor_rect.height()),
                );
                ui.painter_at(output.text_clip_rect).rect_filled(
                    cell,
                    0.0,
                    ui.visuals().selection.bg_fill,
                );
            }

            output
        })
        .inner
    }

    // ASCII art should not inherit indentation when adding a new line.
    fn handle_enter(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) {
        self.handle_indent(ctx, ui, editor_id, |_| String::new());
    }

    fn handle_tab_binding(&mut self, _ctx: &Context, ui: &mut Ui, editor_id: egui::Id) -> bool {
        if !ui.memory(|memory| memory.has_focus(editor_id)) {
            return false;
        }
        let tab_pressed =
            ui.input(|input| !input.modifiers.any() && input.key_pressed(egui::Key::Tab));
        if tab_pressed {
            ui.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
            });
            self.mode = self.mode.toggled();
        }

        // Switching modes does not change editor content.
        false
    }

    fn handle_navigation_binding(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        editor_id: egui::Id,
    ) -> bool {
        if !ui.memory(|memory| memory.has_focus(editor_id)) {
            return false;
        }

        let direction = ui.input(|input| {
            if input.modifiers.any() {
                return None;
            }
            [
                (egui::Key::ArrowUp, CanvasDirection::Up),
                (egui::Key::ArrowDown, CanvasDirection::Down),
                (egui::Key::ArrowLeft, CanvasDirection::Left),
                (egui::Key::ArrowRight, CanvasDirection::Right),
            ]
            .into_iter()
            .find_map(|(key, direction)| input.key_pressed(key).then_some((key, direction)))
        });

        let Some((key, direction)) = direction else {
            return false;
        };
        ui.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, key);
        });
        self.move_canvas_cursor(ctx, ui, editor_id, direction)
    }

    // Cmd/Ctrl-R is intentionally not reserved here; future Svgbob-specific
    // bindings can use it without conflicting with the generic rename binding.
    fn handle_command_bindings(&mut self, _ctx: &Context, _ui: &mut Ui, _tx: &Sender<Msg>) {}

    fn editor_on_changed(&self, _tx: Sender<Msg>, ctx: &Context) {
        let _ = self
            .watch_tx
            .as_ref()
            .expect("Should be initialized")
            .send((ctx.clone(), self.id, self.get_raw_content()));
    }

    fn initialize(&mut self, tx: Sender<Msg>) {
        mini_window::InitializeWatchTx::initialize(self, tx);
    }
}

impl MiniWindow for SvgbobEditor {
    fn get_title(&self) -> String {
        format!("Svgbob - {}", self.get_name())
    }

    fn help_topic(&self) -> crate::help::HelpTopic {
        crate::help::HelpTopic::Svgbob
    }

    fn can_save_to_library(&self) -> bool {
        true
    }
}

impl RenderToggle for SvgbobEditor {
    fn has_renderer(&self) -> bool {
        true
    }

    fn render_enabled(&self) -> bool {
        self.render
    }

    fn set_render_enabled(&mut self, on: bool) {
        self.render = on;
    }

    fn output_type(&self) -> crate::OutputType {
        crate::OutputType::Svgbob
    }

    fn has_output_selector(&self) -> bool {
        false
    }
}

impl editor::Editor for SvgbobEditor {}
impl crate::mini_window::EditorType for SvgbobEditor {
    fn get_editor_type(&self) -> crate::EditorType {
        crate::EditorType::Svgbob
    }
}

impl_id!(SvgbobEditor, id);
impl_target!(SvgbobEditor, target_svg);
impl_visible!(SvgbobEditor, visible);
impl_initialize!(SvgbobEditor, initialized);
impl_indexable!(SvgbobEditor);
impl_generated_content!(SvgbobEditor, content);
impl_initialize_tx!(
    SvgbobEditor, watch_tx,
    on_change: |(ctx,id,content)| Msg::UpdateRender(ctx, id, content),
    data: (Context,egui::Id, String),
    empty: (Context::default(),egui::Id::new(""), String::new())
);

setter_getter_for_trait! { (content => String | content.clone() => String) for SvgbobEditor as raw_content for mini_window::RawContent }
setter_getter_for_trait! { (error => Option<String> | error.clone() => Option<String>) for SvgbobEditor as error for mini_window::HasError }
setter_getter_for_trait! { (name => String | name.clone() => String) for SvgbobEditor as name for mini_window::HasName }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_permanently_a_svgbob_renderer_without_output_selector() {
        let editor = SvgbobEditor::new(egui::Id::new("svgbob"), egui::Id::new("render"));
        assert_eq!(editor.output_type(), crate::OutputType::Svgbob);
        assert!(!editor.has_output_selector());
        assert_eq!(editor.mode, SvgbobEditMode::Insert);
    }

    #[test]
    fn missing_persisted_mode_defaults_to_insert() {
        let editor = SvgbobEditor::new(egui::Id::new("svgbob"), egui::Id::new("render"));
        let mut persisted = serde_json::to_value(editor).unwrap();
        persisted.as_object_mut().unwrap().remove("mode");

        let restored: SvgbobEditor = serde_json::from_value(persisted).unwrap();

        assert_eq!(restored.mode, SvgbobEditMode::Insert);
    }

    #[test]
    fn canvas_navigation_materializes_ragged_cells() {
        let mut content = "ab\nx".to_owned();

        let (changed, cursor) = move_to_grid(&mut content, 1, 3);

        assert!(changed);
        assert_eq!(content, "ab\nx  ");
        assert_eq!(grid_position(&content, cursor), (1, 3));
    }

    #[test]
    fn canvas_navigation_materializes_rows_below_content() {
        let mut content = "ab".to_owned();

        let (changed, cursor) = move_to_grid(&mut content, 2, 2);

        assert!(changed);
        assert_eq!(content, "ab\n\n  ");
        assert_eq!(grid_position(&content, cursor), (2, 2));
    }

    #[test]
    fn canvas_navigation_keeps_existing_grid_cell_unchanged() {
        let mut content = "ab\ncd".to_owned();

        let (changed, cursor) = move_to_grid(&mut content, 1, 1);

        assert!(!changed);
        assert_eq!(content, "ab\ncd");
        assert_eq!(grid_position(&content, cursor), (1, 1));
    }

    #[test]
    fn replace_mode_overwrites_cells_and_advances() {
        let range = egui::text::CCursorRange::one(egui::text::CCursor::new(1));

        let (content, cursor) = replace_text("abcd".to_owned(), range, "XY");

        assert_eq!(content, "aXYd");
        assert_eq!(cursor, 3);
    }

    #[test]
    fn replace_mode_grows_the_canvas_at_end_of_line() {
        let range = egui::text::CCursorRange::one(egui::text::CCursor::new(2));

        let (content, cursor) = replace_text("ab".to_owned(), range, "X");

        assert_eq!(content, "abX");
        assert_eq!(cursor, 3);
    }
}
