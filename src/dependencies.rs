use eframe::egui;

use crate::{AppState, OutputType};

fn has_raw_dependency(content: &str, name: &str) -> bool {
    content.contains(&format!("!!{name}!!"))
}

fn has_generated_dependency(content: &str, name: &str) -> bool {
    content.contains(&format!("$${name}$$"))
}

fn has_svgbob_overlay_dependency(content: &str, name: &str) -> bool {
    content
        .lines()
        .map_while(svgbob_overlay_declaration)
        .any(|(_, editor_name)| editor_name == name)
}

pub(crate) fn clean_old_deps(state: &mut AppState) {
    let span = tracing::info_span!("clean_old_deps", deps_cleaned = tracing::field::Empty);
    let _enter = span.enter();
    let mut cleared_deps = 0;
    let dkeys: Vec<egui::Id> = state.editor_deps.keys().copied().collect();
    for dkey in dkeys {
        let editor_deps = &mut state.editor_deps;
        let Some(dname) = (|| {
            let v = state.windows.get(&dkey)?.as_name()?.get_name();
            Some(v)
        })() else {
            continue;
        };
        let ids = editor_deps.entry(dkey).or_default();
        for id in ids.clone().into_iter() {
            let generated_content = state
                .windows
                .get(&id)
                .and_then(|w| w.as_generated_content())
                .map(|pc| pc.get_generated_content())
                .unwrap_or_default();

            let raw_content = state
                .windows
                .get(&id)
                .and_then(|w| w.as_raw_content())
                .map(|pc| pc.get_raw_content())
                .unwrap_or_default();

            let raw_dependency = has_raw_dependency(&raw_content, &dname);
            let generated_dependency = state
                .windows
                .get(&dkey)
                .and_then(|w| w.as_render_toggle())
                .zip(state.windows.get(&id).and_then(|w| w.as_render_toggle()))
                .is_some_and(|(source, target)| {
                    source.output_type() == target.output_type()
                        && (has_generated_dependency(&generated_content, &dname)
                            || (target.output_type() == OutputType::Svgbob
                                && has_svgbob_overlay_dependency(&generated_content, &dname)))
                });
            let dep_count = usize::from(raw_dependency) + usize::from(generated_dependency);
            if dep_count == 0 {
                tracing::debug!(from = ?&dkey, to = ?&id, "removing dependency");

                slog_scope::debug!("removing dep"; "payload" => format!("{:?} -x- {:?}", &dkey, &id), "category" => "clean_old_deps");
                cleared_deps += 1;
                ids.remove(&id);
            }
        }
    }
    span.record("deps_cleaned", cleared_deps);
}
fn replace_raw_content(state: &mut AppState, id: egui::Id, content: &str) -> String {
    let editors: Vec<(egui::Id, String, String, String)> = state
        .windows
        .values()
        .filter_map(|window| {
            let editor_id = window.as_id()?.get_id();
            if editor_id == id {
                return None;
            }
            let name = window.as_name()?.get_name();
            let raw_content = window.as_raw_content()?.get_raw_content();
            Some((editor_id, name.clone(), format!("!!{name}!!"), raw_content))
        })
        .collect();
    let mut content = String::from(content);
    for (repl_id, name, _repl, _value) in &editors {
        let entry = state.editor_deps.entry(*repl_id).or_default();
        if has_raw_dependency(&content, name) {
            slog_scope::debug!("new dependency"; "type" => "raw", "payload" => format!("{:?} -> {:?}", repl_id, id));
            entry.insert(id);
        }
    }
    for _ in 1..=3 {
        for (_repl_id, _name, repl, value) in &editors {
            content = content.replace(repl, value);
        }
    }
    content
}
pub(crate) fn replace_content(
    state: &mut AppState,
    id: egui::Id,
    content: &str,
) -> Result<String, String> {
    let output_type = state
        .windows
        .get(&id)
        .and_then(|window| window.as_render_toggle())
        .map(|render| render.output_type())
        .unwrap_or_default();
    let content = replace_generated_content(state, id, content, output_type)?;
    Ok(replace_raw_content(state, id, &content))
}

fn svgbob_overlay_declaration(line: &str) -> Option<(char, &str)> {
    let (marker, name) = line.split_once(" = ")?;
    let mut marker_chars = marker.chars();
    let marker = marker_chars.next()?;
    if marker_chars.next().is_some() || name.is_empty() || name.trim() != name {
        return None;
    }
    Some((marker, name))
}

fn overlay_svgbob_at_marker(content: &mut Vec<Vec<char>>, marker: char, value: &str) {
    let positions: Vec<(usize, usize)> = content
        .iter()
        .enumerate()
        .flat_map(|(row, line)| {
            line.iter()
                .enumerate()
                .filter_map(move |(column, character)| {
                    (*character == marker).then_some((row, column))
                })
        })
        .collect();
    let value: Vec<Vec<char>> = value
        .split('\n')
        .map(|line| line.chars().collect())
        .collect();

    for (row, column) in positions {
        content[row][column] = ' ';
        for (row_offset, value_line) in value.iter().enumerate() {
            let target_row = row + row_offset;
            if target_row == content.len() {
                content.push(Vec::new());
            }
            if content[target_row].len() < column + value_line.len() {
                content[target_row].resize(column + value_line.len(), ' ');
            }
            for (column_offset, character) in value_line.iter().enumerate() {
                content[target_row][column + column_offset] = *character;
            }
        }
    }
}

#[cfg(feature = "perf-workloads")]
pub(crate) fn perf_dependency_workload(size: usize) -> usize {
    let mut content = (0..size)
        .map(|row| {
            let marker = if row % 4 == 0 { 'X' } else { ' ' };
            format!("{marker}{:width$}", "", width = size.saturating_sub(1))
                .chars()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    overlay_svgbob_at_marker(&mut content, 'X', "+---+\n| A |\n+---+");
    content.iter().map(Vec::len).sum()
}

fn replace_svgbob_overlays(
    state: &mut AppState,
    id: egui::Id,
    content: &str,
    editors: &[(egui::Id, String, String, String, OutputType)],
) -> Result<String, String> {
    let mut lines = content.split('\n').peekable();
    let mut declarations = Vec::new();
    while let Some(line) = lines.peek() {
        let Some(declaration) = svgbob_overlay_declaration(line) else {
            break;
        };
        declarations.push(declaration);
        lines.next();
    }
    if declarations.is_empty() {
        return Ok(content.to_owned());
    }

    let mut overlays = Vec::new();
    for (marker, name) in declarations {
        let Some((editor_id, _, _, value, output_type)) = editors
            .iter()
            .find(|(_, editor_name, _, _, _)| editor_name == name)
        else {
            return Ok(content.to_owned());
        };
        if *output_type != OutputType::Svgbob {
            return Err(format!(
                "Generated overlay {marker} = {name} uses {} output, but this editor uses Svgbob",
                output_type.label()
            ));
        }
        overlays.push((marker, *editor_id, value));
    }

    let mut body: Vec<Vec<char>> = lines.map(|line| line.chars().collect()).collect();
    for (marker, editor_id, value) in overlays {
        state.editor_deps.entry(editor_id).or_default().insert(id);
        overlay_svgbob_at_marker(&mut body, marker, value);
    }
    Ok(body
        .into_iter()
        .map(|line| line.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n"))
}

pub(crate) fn replace_generated_content(
    state: &mut AppState,
    id: egui::Id,
    content: &str,
    output_type: OutputType,
) -> Result<String, String> {
    let editors: Vec<(egui::Id, String, String, String, OutputType)> = state
        .windows
        .values()
        .flat_map(|e| e.as_editor_window())
        .filter(|e| e.id != &id)
        .map(|e| {
            let source_output_type = e.mini_window.output_type();
            let generated_content = e.content.get_generated_content();
            let generated_content = match source_output_type {
                OutputType::Pikchr => generated_content.trim().replace('\n', ";"),
                OutputType::Svgbob => generated_content,
            };
            (
                *e.id,
                e.name.to_owned(),
                format!("$${}$$", e.name),
                generated_content,
                source_output_type,
            )
        })
        .collect();
    let mut content = String::from(content);

    for (repl_id, name, _repl, _value, source_output_type) in &editors {
        if has_generated_dependency(&content, name) && *source_output_type != output_type {
            return Err(format!(
                "Generated reference $${name}$$ uses {} output, but this editor uses {}",
                source_output_type.label(),
                output_type.label()
            ));
        }
        let entry = state.editor_deps.entry(*repl_id).or_default();
        if has_generated_dependency(&content, name) {
            slog_scope::debug!("new dependency"; "type" => "generated", "payload" => format!("{:?} -> {:?}", repl_id, id));
            entry.insert(id);
        };
    }
    for _ in 1..=3 {
        for (_repl_id, _name, repl, value, source_output_type) in &editors {
            let wrapped_value = match source_output_type {
                OutputType::Pikchr => format!("{value};"),
                OutputType::Svgbob => value.clone(),
            };
            content = content.replace(repl, &wrapped_value);
        }
    }
    if output_type == OutputType::Svgbob {
        content = replace_svgbob_overlays(state, id, &content, &editors)?;
    }
    Ok(content)
}
