use eframe::egui::{self, Context, Ui, text_edit::TextEditOutput};
use tokio::sync::{mpsc::Sender, watch};

use crate::{
    Msg, SvgbobEditMode,
    editor::{self, GenericEditor, HandleEnter as _},
    icons::{AppIcon, icon_button},
    impl_id, impl_indexable, impl_initialize, impl_initialize_tx, impl_target, impl_visible,
    mini_window::{
        self, EditorWindow, GeneratedContent, HasMenu, HasName as _, MiniWindow, RawContent,
        RenderToggle,
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
    #[serde(serialize_with = "serialize_trimmed_canvas")]
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
    #[serde(skip)]
    rectangle_selection: bool,
    #[serde(skip)]
    inclusive_rectangle: bool,
    /// Custom canvas bindings update egui's cursor before `TextEdit` runs, so
    /// remember its position and explicitly reveal it when it moves.
    #[serde(skip)]
    last_cursor_position: Option<usize>,
}

fn default_render() -> bool {
    true
}

fn trimmed_canvas(content: &str) -> String {
    let lines = content
        .split('\n')
        .map(|line| line.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let Some(last_row) = lines
        .iter()
        .rposition(|line| line.iter().any(|character| !character.is_whitespace()))
    else {
        return String::new();
    };
    let width = lines[..=last_row]
        .iter()
        .filter_map(|line| {
            line.iter()
                .rposition(|character| !character.is_whitespace())
        })
        .max()
        .expect("a non-blank row has a non-blank column")
        + 1;

    let mut trimmed = String::new();
    for (row, line) in lines[..=last_row].iter().enumerate() {
        if row > 0 {
            trimmed.push('\n');
        }
        trimmed.extend(line.iter().take(width));
    }
    trimmed
}

fn serialize_trimmed_canvas<S>(content: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&trimmed_canvas(content))
}

fn fitted_canvas(content: &str, minimum_rows: usize, minimum_columns: usize) -> String {
    let canonical = trimmed_canvas(content);
    let mut lines = if canonical.is_empty() {
        Vec::new()
    } else {
        canonical
            .split('\n')
            .map(|line| line.chars().collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    let columns = lines
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or_default()
        .max(minimum_columns);
    let rows = lines.len().max(minimum_rows);
    lines.resize_with(rows, Vec::new);
    for line in &mut lines {
        line.resize(columns, ' ');
    }

    let mut fitted = String::new();
    for (row, line) in lines.iter().enumerate() {
        if row > 0 {
            fitted.push('\n');
        }
        fitted.extend(line);
    }
    fitted
}

fn visible_cells(space: f32, margin: f32, cell_size: f32) -> usize {
    let cells = ((space - margin).max(cell_size) / cell_size).floor();
    if cells.is_finite() && cells <= 10_000.0 {
        cells as usize
    } else {
        0
    }
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
            rectangle_selection: false,
            inclusive_rectangle: false,
            last_cursor_position: None,
        }
    }

    pub(crate) fn set_edit_mode(&mut self, mode: SvgbobEditMode) {
        self.mode = mode;
    }

    #[cfg(test)]
    pub(crate) fn edit_mode(&self) -> SvgbobEditMode {
        self.mode
    }

    fn fit_editing_canvas(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) {
        const HORIZONTAL_MARGIN: f32 = 8.0;
        const VERTICAL_MARGIN: f32 = 4.0;

        let cursor = egui::TextEdit::load_state(ctx, editor_id).and_then(|state| {
            state.cursor.char_range().map(|range| {
                (
                    state,
                    grid_position(&self.content, range.primary.index),
                    grid_position(&self.content, range.secondary.index),
                    range.h_pos,
                )
            })
        });
        let font_id = egui::TextStyle::Monospace.resolve(ui.style());
        let (cell_width, row_height) =
            ui.fonts_mut(|fonts| (fonts.glyph_width(&font_id, ' '), fonts.row_height(&font_id)));
        let available = ui.available_size();
        let viewport_columns = visible_cells(available.x, HORIZONTAL_MARGIN, cell_width);
        let viewport_rows = visible_cells(available.y, VERTICAL_MARGIN, row_height);
        let cursor_columns = cursor.as_ref().map_or(0, |(_, primary, secondary, _)| {
            primary.1.max(secondary.1) + 1
        });
        let cursor_rows = cursor.as_ref().map_or(0, |(_, primary, secondary, _)| {
            primary.0.max(secondary.0) + 1
        });
        let fitted = fitted_canvas(
            &self.content,
            viewport_rows.max(cursor_rows),
            viewport_columns.max(cursor_columns),
        );
        if fitted == self.content {
            return;
        }

        self.content = fitted;
        if let Some((mut state, primary, secondary, h_pos)) = cursor {
            let lines = line_bounds(&self.content);
            let cursor_at = |(row, column): (usize, usize)| {
                let (start, length) = lines[row];
                egui::text::CCursor::new(start + column.min(length))
            };
            state.cursor.set_char_range(Some(egui::text::CCursorRange {
                primary: cursor_at(primary),
                secondary: cursor_at(secondary),
                h_pos,
            }));
            state.store(ctx, editor_id);
        }
    }

    fn move_canvas_cursor(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        editor_id: egui::Id,
        direction: CanvasDirection,
        extend_selection: bool,
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

        let target = egui::text::CCursor::new(target);
        let range = if extend_selection {
            egui::text::CCursorRange {
                primary: target,
                secondary: range.secondary,
                h_pos: None,
            }
        } else {
            egui::text::CCursorRange::one(target)
        };
        state.cursor.set_char_range(Some(range));
        state.store(ui.ctx(), editor_id);
        self.rectangle_selection = extend_selection;
        self.inclusive_rectangle = extend_selection;
        content_changed
    }

    fn handle_replace_input(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) -> bool {
        if self.mode != SvgbobEditMode::Replace || !ui.memory(|memory| memory.has_focus(editor_id))
        {
            return false;
        }
        let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) else {
            return false;
        };
        let Some(range) = state.cursor.char_range() else {
            return false;
        };
        // Egui already has the correct semantics for replacing a selection:
        // remove only the selected text, then insert the new text.
        if !range.is_empty() {
            return false;
        }
        let Some(text) = ui.input_mut(|input| take_replacement_text(&mut input.events)) else {
            return false;
        };

        let (content, cursor) = replace_text(self.content.clone(), range, &text);
        let content_changed = content != self.content;
        self.content = content;
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor),
            )));
        state.store(ctx, editor_id);
        content_changed
    }

    fn handle_replace_backspace(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        editor_id: egui::Id,
    ) -> bool {
        if self.mode != SvgbobEditMode::Replace || !ui.memory(|memory| memory.has_focus(editor_id))
        {
            return false;
        }
        let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) else {
            return false;
        };
        let Some(range) = state.cursor.char_range() else {
            return false;
        };
        if !range.is_empty() {
            return false;
        }

        let backspace = ui.input_mut(|input| {
            let index = input.events.iter().position(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Backspace,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.is_none()
                )
            })?;
            input.events.remove(index);
            Some(())
        });
        if backspace.is_none() {
            return false;
        }

        let Some((content, cursor)) = replace_backspace(&self.content, range.primary.index) else {
            return false;
        };
        let content_changed = content != self.content;
        self.content = content;
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor),
            )));
        state.store(ctx, editor_id);
        content_changed
    }

    fn handle_rectangle_input(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) -> bool {
        if !self.rectangle_selection || !ui.memory(|memory| memory.has_focus(editor_id)) {
            return false;
        }
        let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) else {
            return false;
        };
        let Some(range) = state.cursor.char_range() else {
            return false;
        };
        let bounds = rectangle_bounds(&self.content, range, self.inclusive_rectangle);

        let action = ui.input_mut(|input| {
            let index = input.events.iter().position(|event| {
                matches!(
                    event,
                    egui::Event::Copy
                        | egui::Event::Cut
                        | egui::Event::Paste(_)
                        | egui::Event::Text(_)
                        | egui::Event::Key {
                            key: egui::Key::Backspace | egui::Key::Delete,
                            pressed: true,
                            ..
                        }
                )
            })?;
            Some(input.events.remove(index))
        });
        let Some(action) = action else {
            return false;
        };

        if matches!(&action, egui::Event::Copy | egui::Event::Cut) {
            ctx.copy_text(rectangle_text(&self.content, bounds));
        }
        let replacement = match action {
            egui::Event::Copy => return false,
            egui::Event::Paste(text) | egui::Event::Text(text) => text,
            egui::Event::Cut | egui::Event::Key { .. } => String::new(),
            _ => unreachable!("rectangle input was filtered above"),
        };

        let primary_row = grid_position(&self.content, range.primary.index).0;
        let (content, cursor) = replace_rectangle(&self.content, bounds, primary_row, &replacement);
        self.content = content;
        self.rectangle_selection = false;
        self.inclusive_rectangle = false;
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor),
            )));
        state.store(ctx, editor_id);
        true
    }

    fn handle_column_paste(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) -> bool {
        if self.rectangle_selection || !ui.memory(|memory| memory.has_focus(editor_id)) {
            return false;
        }
        let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) else {
            return false;
        };
        let Some(range) = state.cursor.char_range().filter(|range| range.is_empty()) else {
            return false;
        };
        let paste = ui.input_mut(|input| {
            let index = input.events.iter().position(
                |event| matches!(event, egui::Event::Paste(text) if text.contains(['\n', '\r'])),
            )?;
            match input.events.remove(index) {
                egui::Event::Paste(text) => Some(text),
                _ => unreachable!("column paste was filtered above"),
            }
        });
        let Some(paste) = paste else {
            return false;
        };

        let (content, cursor) = paste_at_column(&self.content, range.primary.index, &paste);
        self.content = content;
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor),
            )));
        state.store(ctx, editor_id);
        true
    }
}

fn take_replacement_text(events: &mut Vec<egui::Event>) -> Option<String> {
    if events.iter().any(|event| {
        matches!(
            event,
            egui::Event::Ime(_)
                | egui::Event::Cut
                | egui::Event::Key {
                    key: egui::Key::Backspace
                        | egui::Key::Delete
                        | egui::Key::Enter
                        | egui::Key::Tab,
                    pressed: true,
                    ..
                }
        )
    }) {
        return None;
    }

    let mut text = String::new();
    let mut index = 0;
    while index < events.len() {
        match &events[index] {
            // TextEdit itself ignores Enter text events because it receives a
            // distinct Key::Enter event.
            egui::Event::Text(input) if input != "\n" && input != "\r" => {
                text.push_str(input);
                events.remove(index);
            },
            egui::Event::Paste(input) => {
                text.push_str(input);
                events.remove(index);
            },
            _ => index += 1,
        }
    }
    (!text.is_empty()).then_some(text)
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

fn line_bounds(content: &str) -> Vec<(usize, usize)> {
    let mut bounds = Vec::new();
    let mut start = 0;
    let mut length = 0;
    for character in content.chars() {
        if character == '\n' {
            bounds.push((start, length));
            start += length + 1;
            length = 0;
        } else {
            length += 1;
        }
    }
    bounds.push((start, length));
    bounds
}

fn rectangle_ranges(
    content: &str,
    range: egui::text::CCursorRange,
    inclusive: bool,
) -> Vec<egui::text::CCursorRange> {
    let (first_row, last_row, first_column, last_column) =
        rectangle_bounds(content, range, inclusive);
    if first_column == last_column {
        return Vec::new();
    }

    line_bounds(content)
        .into_iter()
        .enumerate()
        .filter(|(row, _)| (first_row..=last_row).contains(row))
        .filter_map(|(_, (start, length))| {
            let first = start + first_column.min(length);
            let last = start + last_column.min(length);
            (first != last).then(|| {
                egui::text::CCursorRange::two(
                    egui::text::CCursor::new(first),
                    egui::text::CCursor::new(last),
                )
            })
        })
        .collect()
}

fn rectangle_bounds(
    content: &str,
    range: egui::text::CCursorRange,
    inclusive: bool,
) -> (usize, usize, usize, usize) {
    let (primary_row, primary_column) = grid_position(content, range.primary.index);
    let (secondary_row, secondary_column) = grid_position(content, range.secondary.index);
    let (first_row, last_row) = if primary_row <= secondary_row {
        (primary_row, secondary_row)
    } else {
        (secondary_row, primary_row)
    };
    let (first_column, last_column) = if primary_column <= secondary_column {
        (primary_column, secondary_column)
    } else {
        (secondary_column, primary_column)
    };
    (
        first_row,
        last_row,
        first_column,
        last_column + usize::from(inclusive),
    )
}

fn rectangle_text(content: &str, bounds: (usize, usize, usize, usize)) -> String {
    let (first_row, last_row, first_column, last_column) = bounds;
    let rows = content
        .split('\n')
        .map(|line| line.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    (first_row..=last_row)
        .map(|row| {
            (first_column..last_column)
                .map(|column| {
                    rows.get(row)
                        .and_then(|line| line.get(column))
                        .copied()
                        .unwrap_or(' ')
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn replace_rectangle(
    content: &str,
    bounds: (usize, usize, usize, usize),
    primary_row: usize,
    replacement: &str,
) -> (String, usize) {
    let (first_row, last_row, first_column, last_column) = bounds;
    let mut rows = content
        .split('\n')
        .map(|line| line.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    if rows.len() <= last_row {
        rows.resize_with(last_row + 1, Vec::new);
    }

    let replacements = replacement
        .split('\n')
        .map(|line| line.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let repeat_replacement = replacements.len() == 1;
    let mut primary_replacement_length = 0;
    for (row, line) in rows
        .iter_mut()
        .enumerate()
        .take(last_row + 1)
        .skip(first_row)
    {
        line.resize(first_column.max(line.len()), ' ');
        let end = last_column.min(line.len());
        line.drain(first_column..end);
        let replacement: &[char] = if repeat_replacement {
            &replacements[0]
        } else {
            replacements
                .get(row - first_row)
                .map(Vec::as_slice)
                .unwrap_or(&[])
        };
        if row == primary_row {
            primary_replacement_length = replacement.len();
        }
        line.splice(first_column..first_column, replacement.iter().copied());
    }

    let mut content = rows
        .into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    let (_, cursor) = move_to_grid(
        &mut content,
        primary_row,
        first_column + primary_replacement_length,
    );
    (content, cursor)
}

fn paste_at_column(content: &str, cursor: usize, text: &str) -> (String, usize) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let (row, column) = grid_position(content, cursor);
    let last_row = row + text.split('\n').count() - 1;
    replace_rectangle(content, (row, last_row, column, column), last_row, &text)
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

fn replace_backspace(content: &str, cursor: usize) -> Option<(String, usize)> {
    let (row, column) = grid_position(content, cursor);
    let target_column = column.checked_sub(1)?;
    let mut content = content.to_owned();
    let (_, target) = move_to_grid(&mut content, row, target_column);
    let range = egui::text::CCursorRange::one(egui::text::CCursor::new(target));
    let (content, _) = replace_text(content, range, " ");
    Some((content, target))
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
            let ctx = ui.ctx().clone();
            self.fit_editing_canvas(&ctx, ui, editor_id);

            // TextEdit only has a bar cursor. Hide it and paint a full
            // monospace cell after the widget so the dedicated canvas editor
            // has a conventional block cursor.
            ui.visuals_mut().text_cursor.stroke.color = egui::Color32::TRANSPARENT;
            ui.visuals_mut().text_cursor.blink = false;

            // TextEdit owns pointer interaction and cursor state, but its
            // built-in selection paint is linear. Suppress that paint; the
            // canvas selection is painted row-by-row below.
            let selection_visuals = ui.visuals().clone();
            ui.visuals_mut().selection.bg_fill = egui::Color32::TRANSPARENT;
            ui.visuals_mut().selection.stroke.color = ui.visuals().text_color();

            let rectangle_changed = self.handle_rectangle_input(&ctx, ui, editor_id);
            let paste_changed = self.handle_column_paste(&ctx, ui, editor_id);
            let backspace_changed = self.handle_replace_backspace(&ctx, ui, editor_id);
            let replace_changed = self.handle_replace_input(&ctx, ui, editor_id);

            let mut output = egui::TextEdit::multiline(&mut self.content)
                .code_editor()
                .id(editor_id)
                .layouter(&mut |ui, text, _wrap_width| {
                    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                    let text_color = ui.visuals().text_color();
                    ui.fonts_mut(|fonts| {
                        fonts.layout_no_wrap(text.as_str().to_owned(), font_id, text_color)
                    })
                })
                .show(ui);

            let cursor_position = output
                .cursor_range
                .map(|cursor_range| cursor_range.primary.index);
            let cursor_moved = cursor_position != self.last_cursor_position;
            self.last_cursor_position = cursor_position;

            if cursor_moved && let Some(cursor_range) = output.cursor_range {
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
                ui.scroll_to_rect(cell, None);
            }

            if rectangle_changed || paste_changed || backspace_changed || replace_changed {
                output.response.mark_changed();
            }

            if output.response.dragged() {
                self.rectangle_selection = output
                    .cursor_range
                    .is_some_and(|cursor_range| !cursor_range.is_empty());
                self.inclusive_rectangle = false;
            } else if output.response.clicked() {
                self.rectangle_selection = false;
                self.inclusive_rectangle = false;
            }

            if ui.input(|input| input.key_pressed(egui::Key::A) && input.modifiers.command) {
                self.rectangle_selection = false;
                self.inclusive_rectangle = false;
            }

            ui.visuals_mut().selection = selection_visuals.selection;

            if let Some(cursor_range) = output.cursor_range
                && !cursor_range.is_empty()
            {
                let mut selected_galley = output.galley.clone();
                if self.rectangle_selection {
                    for row_range in
                        rectangle_ranges(&self.content, cursor_range, self.inclusive_rectangle)
                    {
                        egui::text_selection::visuals::paint_text_selection(
                            &mut selected_galley,
                            ui.visuals(),
                            &row_range,
                            None,
                        );
                    }

                    let (first_row, last_row, first_column, last_column) =
                        rectangle_bounds(&self.content, cursor_range, self.inclusive_rectangle);
                    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                    let cell_width = ui.fonts_mut(|fonts| fonts.glyph_width(&font_id, ' '));
                    let lines = line_bounds(&self.content);
                    let painter = ui.painter_at(output.text_clip_rect);
                    for row in first_row..=last_row {
                        let Some(placed_row) = output.galley.rows.get(row) else {
                            continue;
                        };
                        let line_length = lines.get(row).map_or(0, |(_, length)| *length);
                        let blank_start = first_column.max(line_length);
                        if blank_start < last_column {
                            let rect = egui::Rect::from_min_max(
                                output.galley_pos
                                    + egui::vec2(blank_start as f32 * cell_width, placed_row.pos.y),
                                output.galley_pos
                                    + egui::vec2(
                                        last_column as f32 * cell_width,
                                        placed_row.pos.y + placed_row.size.y,
                                    ),
                            );
                            painter.rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
                        }
                    }
                } else {
                    egui::text_selection::visuals::paint_text_selection(
                        &mut selected_galley,
                        ui.visuals(),
                        &cursor_range,
                        None,
                    );
                }
                ui.painter_at(output.text_clip_rect).galley(
                    output.galley_pos,
                    selected_galley.clone(),
                    ui.visuals().text_color(),
                );
                output.galley = selected_galley;
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
            let extend_selection = input.modifiers == egui::Modifiers::SHIFT;
            if !input.modifiers.is_none() && !extend_selection {
                return None;
            }
            [
                (egui::Key::ArrowUp, CanvasDirection::Up),
                (egui::Key::ArrowDown, CanvasDirection::Down),
                (egui::Key::ArrowLeft, CanvasDirection::Left),
                (egui::Key::ArrowRight, CanvasDirection::Right),
            ]
            .into_iter()
            .find_map(|(key, direction)| {
                input
                    .key_pressed(key)
                    .then_some((key, direction, extend_selection))
            })
        });

        let Some((key, direction, extend_selection)) = direction else {
            return false;
        };
        ui.input_mut(|input| {
            input.consume_key(
                if extend_selection {
                    egui::Modifiers::SHIFT
                } else {
                    egui::Modifiers::NONE
                },
                key,
            );
        });
        self.move_canvas_cursor(ctx, ui, editor_id, direction, extend_selection)
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
impl_initialize_tx!(
    SvgbobEditor, watch_tx,
    on_change: |(ctx,id,content)| Msg::UpdateRender(ctx, id, content),
    data: (Context,egui::Id, String),
    empty: (Context::default(),egui::Id::new(""), String::new())
);

impl GeneratedContent for SvgbobEditor {
    fn get_generated_content(&self) -> String {
        trimmed_canvas(&self.content)
    }

    fn set_generated_content(&mut self, value: String) {
        self.content = trimmed_canvas(&value);
    }
}

impl RawContent for SvgbobEditor {
    fn get_raw_content(&self) -> String {
        trimmed_canvas(&self.content)
    }

    fn set_raw_content(&mut self, value: String) {
        self.content = trimmed_canvas(&value);
    }
}

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
    fn editor_does_not_wrap_long_canvas_rows() {
        egui::__run_test_ui(|ui| {
            let mut editor = SvgbobEditor::new(egui::Id::new("svgbob"), egui::Id::new("render"));
            editor.content = "012345678901234567890123456789".to_owned();
            ui.set_max_width(40.0);
            let output = editor.editor_spec(egui::Id::new("editor"), ui);

            assert_eq!(output.galley.rows.len(), editor.content.lines().count());
        });
    }

    #[test]
    fn fitting_canvas_drops_padding_outside_viewport_and_cursor_bounds() {
        let padded = format!("X{}", " ".repeat(79));

        let viewport_fitted = fitted_canvas(&padded, 3, 60);
        assert_eq!(viewport_fitted.lines().count(), 3);
        assert!(
            viewport_fitted
                .lines()
                .all(|line| line.chars().count() == 60)
        );

        let cursor_fitted = fitted_canvas(&padded, 3, 76);
        assert!(cursor_fitted.lines().all(|line| line.chars().count() == 76));
        assert_eq!(trimmed_canvas(&cursor_fitted), "X");
    }

    #[test]
    fn visible_cells_never_overflows_the_viewport() {
        assert_eq!(visible_cells(100.0, 4.0, 18.0), 5);
        assert!(5.0 * 18.0 + 4.0 <= 100.0);

        assert_eq!(visible_cells(100.0, 8.0, 8.0), 11);
        assert!(11.0 * 8.0 + 8.0 <= 100.0);
    }

    #[test]
    fn long_row_does_not_set_the_window_width() {
        use std::{cell::Cell, rc::Rc};

        use egui_kittest::Harness;

        let window_id = egui::Id::new("svgbob_resize_window");
        let editor_id = egui::Id::new("editor");
        let text_rect = Rc::new(Cell::new(egui::Rect::NOTHING));
        let viewport_rect = Rc::new(Cell::new(egui::Rect::NOTHING));
        let shown_text_rect = text_rect.clone();
        let shown_viewport_rect = viewport_rect.clone();
        let mut editor = SvgbobEditor::new(egui::Id::new("svgbob"), egui::Id::new("render"));
        editor.content = "x".repeat(80);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1600.0, 900.0))
            .build(move |ctx| {
                egui::Window::new("resize test")
                    .id(window_id)
                    .default_size(egui::vec2(1000.0, 500.0))
                    .resizable(true)
                    .show(ctx, |ui| {
                        let output =
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.add_sized(ui.available_size(), |ui: &mut egui::Ui| {
                                        let output = editor.editor_spec(editor_id, ui);
                                        shown_text_rect.set(output.response.rect);
                                        output.response
                                    });
                                });
                        shown_viewport_rect.set(output.inner_rect);
                    });
            });

        let mut state = egui::text_edit::TextEditState::default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(80),
            )));
        state.store(&harness.ctx, editor_id);
        harness
            .ctx
            .memory_mut(|memory| memory.request_focus(editor_id));
        harness.run_steps(2);

        let initial = harness
            .ctx
            .memory(|memory| memory.area_rect(window_id).unwrap());
        let handle = initial.right_bottom();
        let target = handle - egui::vec2(700.0, 0.0);
        harness.hover_at(handle);
        harness.step();
        harness.drag_at(handle);
        harness.step();
        harness.hover_at(target);
        harness.run_steps(2);
        harness.drop_at(target);
        harness.run_steps(2);

        let resized = harness
            .ctx
            .memory(|memory| memory.area_rect(window_id).unwrap());
        assert!(resized.width() < 350.0, "resized window was {resized:?}");
        assert!(
            text_rect.get().right() >= viewport_rect.get().right() - 1.0,
            "text frame {:?} ended before viewport {:?}",
            text_rect.get(),
            viewport_rect.get()
        );
    }

    #[test]
    fn trims_blank_bottom_rows_and_columns_right_of_the_last_non_blank_cell() {
        assert_eq!(
            trimmed_canvas("A     X   \nB         \n          \n"),
            "A     X\nB      "
        );
        assert_eq!(trimmed_canvas("   \n\n"), "");
    }

    #[test]
    fn canonical_reads_do_not_modify_the_padded_editing_canvas() {
        let mut editor = SvgbobEditor::new(egui::Id::new("svgbob"), egui::Id::new("render"));
        editor.content = "A     X   \nB         \n          \n".to_owned();
        let padded = editor.content.clone();

        assert_eq!(editor.get_raw_content(), "A     X\nB      ");
        assert_eq!(editor.get_generated_content(), "A     X\nB      ");
        assert_eq!(editor.content, padded);
    }

    #[test]
    fn serialized_canvas_content_is_canonical() {
        let mut editor = SvgbobEditor::new(egui::Id::new("svgbob"), egui::Id::new("render"));
        editor.content = "A   \n    \n".to_owned();

        let value = serde_json::to_value(&editor).unwrap();
        assert_eq!(value["content"], "A");

        let restored: SvgbobEditor = serde_json::from_value(value).unwrap();
        assert_eq!(restored.content, "A");
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
    fn rectangle_selection_uses_the_same_columns_on_each_row() {
        let content = "abcd\nefgh\nijkl";
        let range = egui::text::CCursorRange {
            primary: egui::text::CCursor::new(13),
            secondary: egui::text::CCursor::new(1),
            h_pos: None,
        };

        let ranges = rectangle_ranges(content, range, false);
        let selected = ranges
            .iter()
            .map(|range| range.slice_str(content))
            .collect::<Vec<_>>();

        assert_eq!(selected, ["bc", "fg", "jk"]);
    }

    #[test]
    fn zero_width_rectangle_selects_no_text() {
        let content = "abcd\nefgh";
        let range = egui::text::CCursorRange {
            primary: egui::text::CCursor::new(7),
            secondary: egui::text::CCursor::new(2),
            h_pos: None,
        };

        assert!(rectangle_ranges(content, range, false).is_empty());
    }

    #[test]
    fn shift_right_selects_the_cursor_and_destination_cells() {
        let content = "abcd";
        let range = egui::text::CCursorRange {
            primary: egui::text::CCursor::new(2),
            secondary: egui::text::CCursor::new(1),
            h_pos: None,
        };

        let ranges = rectangle_ranges(content, range, true);

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].slice_str(content), "bc");
    }

    #[test]
    fn rectangle_copy_preserves_the_canvas_shape() {
        assert_eq!(rectangle_text("abcd\nef\nijkl", (0, 2, 1, 3)), "bc\nf \njk");
    }

    #[test]
    fn typing_replaces_each_selected_row() {
        let (content, cursor) = replace_rectangle("abcd\nefgh\nijkl", (0, 2, 1, 3), 2, "X");

        assert_eq!(content, "aXd\neXh\niXl");
        assert_eq!(grid_position(&content, cursor), (2, 2));
    }

    #[test]
    fn rectangle_edit_preserves_rows_after_the_selection() {
        let content = "line 1\nline 2\nline 3\nline 4\nline 5";

        let (content, _) = replace_rectangle(content, (2, 2, 0, 2), 2, "");

        assert_eq!(content, "line 1\nline 2\nne 3\nline 4\nline 5");
    }

    #[test]
    fn multiline_paste_maps_lines_into_the_rectangle() {
        let (content, cursor) = replace_rectangle("abcd\nefgh\nijkl", (0, 2, 1, 3), 2, "XY\nZ\n");

        assert_eq!(content, "aXYd\neZh\nil");
        assert_eq!(grid_position(&content, cursor), (2, 1));
    }

    #[test]
    fn multiline_paste_at_cursor_inserts_each_line_at_the_same_column() {
        let (content, cursor) = paste_at_column("abcd\nefgh", 2, "X\r\nYZ");

        assert_eq!(content, "abXcd\nefYZgh");
        assert_eq!(grid_position(&content, cursor), (1, 4));
    }

    #[test]
    fn replace_mode_overwrites_cells_and_advances() {
        let range = egui::text::CCursorRange::one(egui::text::CCursor::new(1));

        let (content, cursor) = replace_text("abcd".to_owned(), range, "XY");

        assert_eq!(content, "aXYd");
        assert_eq!(cursor, 3);
    }

    #[test]
    fn replace_mode_consumes_text_before_text_edit() {
        let mut events = vec![egui::Event::Text("XY".to_owned())];

        assert_eq!(take_replacement_text(&mut events).as_deref(), Some("XY"));
        assert!(events.is_empty());
    }

    #[test]
    fn replace_mode_treats_backspace_as_space() {
        let (content, cursor) = replace_backspace("abcd", 2).unwrap();

        assert_eq!(content, "a cd");
        assert_eq!(cursor, 1);
    }

    #[test]
    fn replace_mode_backspace_stops_at_start_of_line() {
        assert!(replace_backspace("ab\ncd", 3).is_none());
    }

    #[test]
    fn replace_mode_grows_the_canvas_at_end_of_line() {
        let range = egui::text::CCursorRange::one(egui::text::CCursor::new(2));

        let (content, cursor) = replace_text("ab".to_owned(), range, "X");

        assert_eq!(content, "abX");
        assert_eq!(cursor, 3);
    }
}
