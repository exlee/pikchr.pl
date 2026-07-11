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
    let cells = visible_cells(100.0, 4.0, 18.0) as f32;
    assert!(cells * 18.0 + 4.0 <= 100.0);

    assert_eq!(visible_cells(100.0, 8.0, 8.0), 11);
    let cells = visible_cells(100.0, 8.0, 8.0) as f32;
    assert!(cells * 8.0 + 8.0 <= 100.0);
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

    let (content, cursor, _) = replace_text("abcd".to_owned(), range, "XY", None);

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

    let (content, cursor, _) = replace_text("ab".to_owned(), range, "X", None);

    assert_eq!(content, "abX");
    assert_eq!(cursor, 3);
}

#[test]
fn replace_mode_continues_the_relation_between_adjacent_inputs() {
    let first = egui::text::CCursorRange::one(egui::text::CCursor::new(0));
    let (content, _, last) = replace_text("1234\n5678\n90XY".to_owned(), first, "A", None);
    let second = egui::text::CCursorRange::one(egui::text::CCursor::new(6));
    let (content, cursor, last) = replace_text(content, second, "B", last);

    assert_eq!(grid_position(&content, cursor), (2, 2));

    let moved = egui::text::CCursorRange::one(egui::text::CCursor::new(7));
    let (_, cursor, _) = replace_text(content, moved, "C", last);
    assert_eq!(grid_position("1234\n5678\n90XY", cursor), (1, 3));
}

#[test]
fn replace_mode_continues_the_exact_input_vector() {
    let range = egui::text::CCursorRange::one(egui::text::CCursor::new(6));
    let (_, _, last) = replace_text("1234\n5678\n90XY".to_owned(), range, "A", None);

    let diagonal = egui::text::CCursorRange::one(egui::text::CCursor::new(12));
    let (content, cursor, _) = replace_text("1234\n5678\n90XY".to_owned(), diagonal, "B", last);

    assert_eq!(grid_position(&content, cursor), (3, 3));
}

fn snapshot(content: &str, cursor: usize) -> EditSnapshot {
    (
        egui::text::CCursorRange::one(egui::text::CCursor::new(cursor)),
        content.to_owned(),
    )
}

#[test]
fn custom_edits_are_distinct_undo_and_redo_steps() {
    let first = snapshot("A", 1);
    let second = snapshot("AB", 2);
    let third = snapshot("ABC", 3);
    let mut undoer = EditUndoer::default();

    record_semantic_edit(&mut undoer, 0.0, &first, &second);
    record_semantic_edit(&mut undoer, 0.1, &second, &third);

    let second_again = step_semantic_history(&mut undoer, &third, HistoryDirection::Undo).unwrap();
    assert_eq!(second_again, second);

    let first_again =
        step_semantic_history(&mut undoer, &second_again, HistoryDirection::Undo).unwrap();
    assert_eq!(first_again, first);

    let second_redone =
        step_semantic_history(&mut undoer, &first_again, HistoryDirection::Redo).unwrap();
    assert_eq!(second_redone, second);

    let third_redone =
        step_semantic_history(&mut undoer, &second_redone, HistoryDirection::Redo).unwrap();
    assert_eq!(third_redone, third);
}

#[test]
fn custom_edit_after_undo_clears_the_redo_branch() {
    let first = snapshot("A", 1);
    let discarded = snapshot("AB", 2);
    let replacement = snapshot("AC", 2);
    let mut undoer = EditUndoer::default();

    record_semantic_edit(&mut undoer, 0.0, &first, &discarded);
    let first_again =
        step_semantic_history(&mut undoer, &discarded, HistoryDirection::Undo).unwrap();
    record_semantic_edit(&mut undoer, 0.1, &first_again, &replacement);

    assert!(step_semantic_history(&mut undoer, &replacement, HistoryDirection::Redo,).is_none());
}

#[test]
fn undo_skips_padding_only_canvas_states() {
    let first = snapshot("A", 1);
    let second = snapshot("AB ", 2);
    let second_with_more_padding = snapshot("AB   \n     ", 2);
    let mut undoer = EditUndoer::default();
    undoer.add_undo(&first);
    undoer.add_undo(&second);
    undoer.add_undo(&second_with_more_padding);

    let undone = step_semantic_history(
        &mut undoer,
        &second_with_more_padding,
        HistoryDirection::Undo,
    )
    .unwrap();

    assert_eq!(trimmed_canvas(&undone.1), "A");
}

#[test]
fn replace_mode_undoes_each_rapid_input_and_redoes_it() {
    use egui_kittest::Harness;

    let editor_id = egui::Id::new("undo_editor");
    let mut editor = SvgbobEditor::new(egui::Id::new("svgbob_undo"), egui::Id::new("render"));
    editor.content = "ABC".to_owned();
    editor.mode = SvgbobEditMode::Replace;
    let mut harness = Harness::new_ui_state(
        move |ui, editor| {
            editor.editor_spec(editor_id, ui);
        },
        editor,
    );
    harness.run();

    let mut state = egui::TextEdit::load_state(&harness.ctx, editor_id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(1),
        )));
    state.store(&harness.ctx, editor_id);
    harness
        .ctx
        .memory_mut(|memory| memory.request_focus(editor_id));

    harness.event(egui::Event::Text("X".to_owned()));
    harness.run();
    harness.event(egui::Event::Text("Y".to_owned()));
    harness.run();
    assert_eq!(trimmed_canvas(&harness.state().content), "AXY");

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();
    assert_eq!(trimmed_canvas(&harness.state().content), "AXC");

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();
    assert_eq!(trimmed_canvas(&harness.state().content), "ABC");

    harness.key_press_modifiers(
        egui::Modifiers::SHIFT | egui::Modifiers::COMMAND,
        egui::Key::Z,
    );
    harness.run();
    assert_eq!(trimmed_canvas(&harness.state().content), "AXC");
}
