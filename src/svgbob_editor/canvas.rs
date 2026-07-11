use super::*;

pub(super) fn default_render() -> bool {
    true
}

pub(super) fn trimmed_canvas(content: &str) -> String {
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

pub(super) fn serialize_trimmed_canvas<S>(content: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&trimmed_canvas(content))
}

pub(super) fn fitted_canvas(content: &str, minimum_rows: usize, minimum_columns: usize) -> String {
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

pub(super) fn visible_cells(space: f32, margin: f32, cell_size: f32) -> usize {
    let cells = ((space - margin).max(cell_size) / cell_size).floor();
    if cells.is_finite() && cells <= 10_000.0 {
        cells as usize
    } else {
        0
    }
}

pub(super) fn take_replacement_text(events: &mut Vec<egui::Event>) -> Option<String> {
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
pub(super) enum CanvasDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Return the row and column of a character cursor in the ASCII-art grid.
pub(super) fn grid_position(content: &str, cursor: usize) -> (usize, usize) {
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
pub(super) fn move_to_grid(content: &mut String, row: usize, column: usize) -> (bool, usize) {
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

pub(super) fn byte_index(content: &str, character_index: usize) -> usize {
    content
        .char_indices()
        .nth(character_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(content.len())
}

pub(super) fn line_bounds(content: &str) -> Vec<(usize, usize)> {
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

pub(super) fn rectangle_ranges(
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

pub(super) fn rectangle_bounds(
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

pub(super) fn rectangle_text(content: &str, bounds: (usize, usize, usize, usize)) -> String {
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

pub(super) fn replace_rectangle(
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

pub(super) fn paste_at_column(content: &str, cursor: usize, text: &str) -> (String, usize) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let (row, column) = grid_position(content, cursor);
    let last_row = row + text.split('\n').count() - 1;
    replace_rectangle(content, (row, last_row, column, column), last_row, &text)
}

/// Apply text in Replace mode: each non-newline character overwrites one
/// canvas cell, then the block cursor advances to the next cell.
pub(super) fn replace_text(
    mut content: String,
    range: egui::text::CCursorRange,
    text: &str,
    mut last_input_position: Option<(usize, usize)>,
) -> (String, usize, Option<(usize, usize)>) {
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
                last_input_position = None;
            },
            character => {
                // Materialize the target cell, not just its preceding cursor
                // position, so replacing at an end-of-line grows the canvas.
                let (_, after_cell) = move_to_grid(&mut content, row, column + 1);
                let cell = after_cell - 1;
                let start_byte = byte_index(&content, cell);
                let end_byte = byte_index(&content, cell + 1);
                content.replace_range(start_byte..end_byte, &character.to_string());
                let input_position = (row, column);
                let next = match last_input_position {
                    Some((previous_row, previous_column))
                        if row.abs_diff(previous_row) <= 1
                            && column.abs_diff(previous_column) <= 1
                            && (row, column) != (previous_row, previous_column) =>
                    {
                        (
                            row.saturating_add_signed(row as isize - previous_row as isize),
                            column
                                .saturating_add_signed(column as isize - previous_column as isize),
                        )
                    },
                    _ => (row, column + 1),
                };
                last_input_position = Some(input_position);
                row = next.0;
                column = next.1;
            },
        }
    }
    let (_, cursor) = move_to_grid(&mut content, row, column);
    (content, cursor, last_input_position)
}

pub(super) fn replace_backspace(content: &str, cursor: usize) -> Option<(String, usize)> {
    let (row, column) = grid_position(content, cursor);
    let target_column = column.checked_sub(1)?;
    let mut content = content.to_owned();
    let (_, target) = move_to_grid(&mut content, row, target_column);
    let range = egui::text::CCursorRange::one(egui::text::CCursor::new(target));
    let (content, _, _) = replace_text(content, range, " ", None);
    Some((content, target))
}

#[cfg(feature = "perf-workloads")]
pub(crate) fn perf_canvas_workload(rows: usize, columns: usize) -> usize {
    let source = (0..rows)
        .map(|row| {
            (0..columns)
                .map(|column| if (row + column) % 17 == 0 { '+' } else { ' ' })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut content = fitted_canvas(&source, rows + 8, columns + 8);
    let (_, cursor) = move_to_grid(&mut content, rows / 2, columns / 2);
    let range = egui::text::CCursorRange::one(egui::text::CCursor::new(cursor));
    let (content, cursor, _) = replace_text(content, range, "DiagramIDE", None);
    let (row, column) = grid_position(&content, cursor);
    let bounds = (
        row.saturating_sub(4),
        (row + 4).min(rows.saturating_sub(1)),
        column.saturating_sub(4),
        column + 4,
    );
    let selected = rectangle_text(&content, bounds);
    let (content, _) = replace_rectangle(&content, bounds, row, &selected);
    trimmed_canvas(&content).len()
}
