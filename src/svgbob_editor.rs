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

mod canvas;
#[cfg(feature = "perf-workloads")]
pub(crate) use canvas::perf_canvas_workload;
use canvas::*;

type EditSnapshot = (egui::text::CCursorRange, String);
type EditUndoer = egui::util::undoer::Undoer<EditSnapshot>;

#[derive(Clone, Copy)]
enum HistoryDirection {
    Undo,
    Redo,
}

fn record_semantic_edit(
    undoer: &mut EditUndoer,
    time: f64,
    before: &EditSnapshot,
    after: &EditSnapshot,
) {
    if before == after {
        return;
    }

    undoer.add_undo(before);
    // Feeding the changed state clears any redo branch before the new
    // checkpoint is added.
    undoer.feed_state(time, after);
    undoer.add_undo(after);
}

fn step_semantic_history(
    undoer: &mut EditUndoer,
    current: &EditSnapshot,
    direction: HistoryDirection,
) -> Option<EditSnapshot> {
    let current_content = trimmed_canvas(&current.1);
    let mut candidate = current.clone();
    let mut moved = false;

    loop {
        let Some(next) = (match direction {
            HistoryDirection::Undo => undoer.undo(&candidate),
            HistoryDirection::Redo => undoer.redo(&candidate),
        })
        .cloned() else {
            return moved.then_some(candidate);
        };
        moved = true;
        candidate = next;

        if trimmed_canvas(&candidate.1) != current_content {
            return Some(candidate);
        }
    }
}

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
    /// Cell where the previous character was typed. Used to continue a
    /// connected row or column when typing into the canvas.
    #[serde(skip)]
    last_input_position: Option<(usize, usize)>,
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
            last_input_position: None,
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

        let before = (range, self.content.clone());
        let (content, cursor, last_input_position) =
            replace_text(self.content.clone(), range, &text, self.last_input_position);
        let content_changed = content != self.content;
        let cursor_range = egui::text::CCursorRange::one(egui::text::CCursor::new(cursor));
        if content_changed {
            let mut undoer = state.undoer();
            record_semantic_edit(
                &mut undoer,
                ui.input(|input| input.time),
                &before,
                &(cursor_range, content.clone()),
            );
            state.set_undoer(undoer);
        }
        self.content = content;
        self.last_input_position = last_input_position;
        state.cursor.set_char_range(Some(cursor_range));
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

        let before = (range, self.content.clone());
        let Some((content, cursor)) = replace_backspace(&self.content, range.primary.index) else {
            return false;
        };
        let content_changed = content != self.content;
        let cursor_range = egui::text::CCursorRange::one(egui::text::CCursor::new(cursor));
        if content_changed {
            let mut undoer = state.undoer();
            record_semantic_edit(
                &mut undoer,
                ui.input(|input| input.time),
                &before,
                &(cursor_range, content.clone()),
            );
            state.set_undoer(undoer);
        }
        self.content = content;
        state.cursor.set_char_range(Some(cursor_range));
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

        let before = (range, self.content.clone());
        let primary_row = grid_position(&self.content, range.primary.index).0;
        let (content, cursor) = replace_rectangle(&self.content, bounds, primary_row, &replacement);
        let content_changed = content != self.content;
        let cursor_range = egui::text::CCursorRange::one(egui::text::CCursor::new(cursor));
        if content_changed {
            let mut undoer = state.undoer();
            record_semantic_edit(
                &mut undoer,
                ui.input(|input| input.time),
                &before,
                &(cursor_range, content.clone()),
            );
            state.set_undoer(undoer);
        }
        self.content = content;
        self.rectangle_selection = false;
        self.inclusive_rectangle = false;
        state.cursor.set_char_range(Some(cursor_range));
        state.store(ctx, editor_id);
        content_changed
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

        let before = (range, self.content.clone());
        let (content, cursor) = paste_at_column(&self.content, range.primary.index, &paste);
        let content_changed = content != self.content;
        let cursor_range = egui::text::CCursorRange::one(egui::text::CCursor::new(cursor));
        if content_changed {
            let mut undoer = state.undoer();
            record_semantic_edit(
                &mut undoer,
                ui.input(|input| input.time),
                &before,
                &(cursor_range, content.clone()),
            );
            state.set_undoer(undoer);
        }
        self.content = content;
        state.cursor.set_char_range(Some(cursor_range));
        state.store(ctx, editor_id);
        content_changed
    }

    fn handle_undo_redo(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) -> bool {
        if !ui.memory(|memory| memory.has_focus(editor_id)) {
            return false;
        }

        let direction = ui.input(|input| {
            if input.key_pressed(egui::Key::Z)
                && input
                    .modifiers
                    .matches_logically(egui::Modifiers::SHIFT | egui::Modifiers::COMMAND)
                || input.key_pressed(egui::Key::Y)
                    && input.modifiers.matches_logically(egui::Modifiers::COMMAND)
            {
                Some(HistoryDirection::Redo)
            } else if input.key_pressed(egui::Key::Z)
                && input.modifiers.matches_logically(egui::Modifiers::COMMAND)
            {
                Some(HistoryDirection::Undo)
            } else {
                None
            }
        });
        let Some(direction) = direction else {
            return false;
        };

        ui.input_mut(|input| match direction {
            HistoryDirection::Undo => {
                input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z);
            },
            HistoryDirection::Redo => {
                input.consume_key(
                    egui::Modifiers::SHIFT | egui::Modifiers::COMMAND,
                    egui::Key::Z,
                );
                input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y);
            },
        });

        let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) else {
            return false;
        };
        let Some(range) = state.cursor.char_range() else {
            return false;
        };
        let current = (range, self.content.clone());
        let mut undoer = state.undoer();
        let Some((cursor_range, content)) = step_semantic_history(&mut undoer, &current, direction)
        else {
            return false;
        };
        let content_changed = trimmed_canvas(&content) != trimmed_canvas(&self.content);

        self.content = content;
        self.rectangle_selection = false;
        self.inclusive_rectangle = false;
        self.last_input_position = None;
        state.cursor.set_char_range(Some(cursor_range));
        state.set_undoer(undoer);
        state.store(ctx, editor_id);
        content_changed
    }
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

            let history_changed = self.handle_undo_redo(&ctx, ui, editor_id);
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

            if history_changed
                || rectangle_changed
                || paste_changed
                || backspace_changed
                || replace_changed
            {
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
        let before = egui::TextEdit::load_state(ctx, editor_id).and_then(|state| {
            state
                .cursor
                .char_range()
                .map(|range| (range, self.content.clone()))
        });
        self.handle_indent(ctx, ui, editor_id, |_| String::new());
        let Some(before) = before else {
            return;
        };
        let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) else {
            return;
        };
        let Some(range) = state.cursor.char_range() else {
            return;
        };
        let after = (range, self.content.clone());
        let mut undoer = state.undoer();
        record_semantic_edit(&mut undoer, ui.input(|input| input.time), &before, &after);
        state.set_undoer(undoer);
        state.store(ctx, editor_id);
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
#[path = "svgbob_editor/tests.rs"]
mod tests;
