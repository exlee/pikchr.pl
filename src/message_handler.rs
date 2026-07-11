use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::BufReader,
    sync::Arc,
};

use eframe::egui;
use parking_lot::RwLock;
use slog::{Logger, Serde, debug, o};
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt as _;
use tokio_util::time::{DelayQueue, delay_queue::Key as DelayKey};
use tracing::Instrument as _;

use crate::{
    AppState, Msg, clean_old_deps, identifiers, mini_window,
    modal::{
        ConfirmationModal, ExportModal, FileOpenModal, FileSaveModal, RenameModal,
        SaveToLibraryModal, StringEditModal, WorkspaceNameModal,
    },
    mruby, mruby_editor, pikchr_editor, plain_text_editor, prolog_editor,
    state::{LibraryEntry, Workspace},
    state_serialize::AppStatePersistent,
    svg, svgbob_editor, tcl, tcl_editor,
};

macro_rules! push_modal {
    ($state:ident, $modal:expr) => {
        $state
            .write()
            .modals
            .push_back(Arc::new(RwLock::new($modal)));
    };
}

macro_rules! create_editor_window {
    ($state: ident, $wintype:ident, $fun:path) => {{
        let editor_id = identifiers::next_global_id();
        let svg_id = identifiers::next_global_id();
        let editor_insert = mini_window::Window::$wintype($fun(editor_id, svg_id));
        let output_type = editor_insert
            .as_render_toggle()
            .map(|render| render.output_type())
            .unwrap_or_default();
        let mut svg_window = svg::SvgWindow::new(svg_id, editor_id);
        svg_window.output_type = output_type;
        let svg_insert = mini_window::Window::SvgWindow(svg_window);
        let mut state_write = $state.write();
        state_write.windows.insert(editor_id, editor_insert);
        state_write.windows.insert(svg_id, svg_insert);
        editor_id
    }};
}
macro_rules! create_plain_text_window {
    ($state: ident) => {{
        let editor_id = identifiers::next_global_id();
        let editor_insert = mini_window::Window::PlainTextEditor(
            plain_text_editor::PlainTextEditor::new(editor_id),
        );
        $state.write().windows.insert(editor_id, editor_insert);
        editor_id
    }};
}
macro_rules! create_help_window {
    ($state: ident, $topic:expr) => {{
        let id = identifiers::next_global_id();
        let insert = mini_window::Window::HelpWindow(crate::help::HelpWindow::new(id, $topic));
        $state.write().windows.insert(id, insert);
        id
    }};
}

fn install_diagram_texture(
    state: &Arc<RwLock<AppState>>,
    id: egui::Id,
    texture: egui::TextureHandle,
) -> Option<()> {
    let mut state = state.write();
    let window = state.windows.get_mut(&id)?.as_svg_window()?;
    *window.diagram_texture = Some(texture);
    Some(())
}

fn clean_library_path(path: String) -> String {
    path.split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn leaf_name(path: &str) -> String {
    path.rsplit('/')
        .find(|part| !part.trim().is_empty())
        .unwrap_or(path)
        .trim()
        .to_owned()
}

fn create_window_from_library_entry(
    state: &Arc<RwLock<AppState>>,
    entry: &LibraryEntry,
) -> Option<egui::Id> {
    let editor_id = match entry.editor_type {
        crate::EditorType::Pikchr => {
            create_editor_window!(state, PikchrEditor, pikchr_editor::PikchrEditor::new)
        },
        crate::EditorType::Svgbob => {
            create_editor_window!(state, SvgbobEditor, svgbob_editor::SvgbobEditor::new)
        },
        crate::EditorType::Prolog => {
            create_editor_window!(state, PrologEditor, prolog_editor::PrologEditor::new)
        },
        crate::EditorType::Tcl => {
            create_editor_window!(state, TclEditor, tcl_editor::TclEditor::new)
        },
        crate::EditorType::Mruby => {
            create_editor_window!(state, MrubyEditor, mruby_editor::MrubyEditor::new)
        },
        crate::EditorType::PlainText => create_plain_text_window!(state),
    };

    let mut state = state.write();
    let window = state.windows.get_mut(&editor_id)?;
    if let Some(content) = window.as_raw_content_mut() {
        content.set_raw_content(entry.content.clone());
    }
    if let Some(render) = window.as_render_toggle_mut() {
        render.set_output_type(entry.output_type);
    }
    let output_type = window
        .as_render_toggle()
        .map(|render| render.output_type())
        .unwrap_or_default();
    let target_svg = window.as_target().map(|target| target.get_target());
    if let Some(svg_id) = target_svg
        && let Some(mini_window::Window::SvgWindow(svg_window)) = state.windows.get_mut(&svg_id)
    {
        svg_window.output_type = output_type;
    }
    state
        .window_library_paths
        .insert(editor_id, entry.path.clone());
    Some(editor_id)
}

fn export_library_entry_to_json(state: &Arc<RwLock<AppState>>, entry: &LibraryEntry) -> Option<()> {
    let Some(destination) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_file_name(format!("{}.json", leaf_name(&entry.path)))
        .save_file()
    else {
        return Some(());
    };
    match serde_json::to_vec_pretty(entry) {
        Ok(payload) => {
            if let Err(err) = std::fs::write(&destination, payload) {
                state
                    .write()
                    .log
                    .push(format!("Could not export library entry: {err}"));
            }
        },
        Err(err) => {
            state
                .write()
                .log
                .push(format!("Could not serialize library entry: {err}"));
        },
    }
    Some(())
}
pub async fn handle(
    mut rx: tokio::sync::mpsc::Receiver<Msg>,
    logger: Logger,
    state: Arc<RwLock<AppState>>,
) {
    let mut local_queue: VecDeque<Msg> = VecDeque::new();
    let mut delay_queue: DelayQueue<(egui::Id, Msg)> = DelayQueue::new();
    let mut pending_debounces: HashMap<egui::Id, DelayKey> = HashMap::new();
    let mut cleanup_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    let logger = logger.new(o!("category" => "event"));

    loop {
        tokio::select! {
            biased;
            _ = cleanup_interval.tick() => {
                local_queue.push_back(Msg::CheckDependencies);
            }

            Some(expired) = delay_queue.next(), if !delay_queue.is_empty() => {
                let (id, msg) = expired.into_inner();
                pending_debounces.remove(&id);
                local_queue.push_back(msg);
            }
            maybe_msg = rx.recv() => {
                match maybe_msg {
                    Some(Msg::Debounce(dur, id, inner)) => {
                        if let Some(delay_key) = pending_debounces.get(&id) {
                            delay_queue.remove(delay_key);
                        }
                        let queue_key = delay_queue.insert_at(
                            (id, *inner),
                            tokio::time::Instant::now() + dur,
                        );
                        pending_debounces.insert(id, queue_key);

                    }
                    Some(msg) => local_queue.push_back(msg),
                    None => break,
                }
            }

        };
        while let Some(msg) = local_queue.pop_front() {
            #[cfg(feature = "profile")]
            {
                tracing::info!(
                    tracy.plot = "Event Local Queue Size",
                    value = local_queue.len() as f64
                );
            }
            let span = tracing::info_span!("handle_event", msg = ?msg);
            let _ = handle_event(logger.clone(), msg, state.clone(), &mut local_queue)
                .instrument(span)
                .await;
        }
    }
    debug!(logger, "Handler exiting");
}

async fn handle_event(
    logger: Logger,
    msg: Msg,
    state: Arc<RwLock<AppState>>,
    local_queue: &mut VecDeque<Msg>,
) -> Option<()> {
    debug!(logger, "handle msg"; "msg" => Serde(msg.clone()));
    match msg {
        Msg::Debounce(..) => unreachable!(),
        Msg::ShowHelp(topic) => {
            // Help is a regular window: reuse an existing window for the
            // requested topic (re-showing it if hidden), otherwise create one.
            let existing = state.read().windows.values().find_map(|w| match w {
                mini_window::Window::HelpWindow(h) if h.topic == topic => Some(h.id),
                _ => None,
            });
            if let Some(id) = existing {
                if let Some(mini_window::Window::HelpWindow(h)) = state.write().windows.get_mut(&id)
                {
                    h.visible = true;
                }
            } else {
                create_help_window!(state, topic);
            }
        },
        Msg::SelectTheme(ctx, id) => {
            if crate::theme::set_active(&id, &ctx) {
                state.write().active_theme = id;
                local_queue.push_back(Msg::ReloadSvgs(ctx));
            } else {
                state.write().log.push(format!("Theme not found: {id}"));
            }
        },
        Msg::ReloadThemes(ctx) => {
            let errors = crate::theme::reload(&ctx);
            let active = crate::theme::active_id();
            let mut state = state.write();
            state.active_theme = active;
            state.log.extend(errors);
            drop(state);
            local_queue.push_back(Msg::ReloadSvgs(ctx));
        },
        Msg::OpenThemesFolder => {
            if let Err(err) = crate::theme::open_themes_dir() {
                state.write().log.push(err);
            }
        },
        Msg::SetDiagramBackground(ctx, background) => {
            state.write().diagram_background = background;
            local_queue.push_back(Msg::ReloadSvgs(ctx));
        },
        Msg::Batch(msgs) => {
            for m in msgs {
                local_queue.push_back(m);
            }
        },
        Msg::DeleteWindow(id) => {
            let mut state = state.write();
            let dkeys: Vec<egui::Id> = state.editor_deps.keys().cloned().collect();
            {
                if let Some(targetable) = state.windows.get(&id).and_then(|w| w.as_target()) {
                    let target_id = targetable.get_target();
                    state.windows.remove(&target_id);
                    state.window_library_paths.remove(&target_id);
                }
                state.windows.remove(&id);
                state.window_library_paths.remove(&id);
            }
            for dkey in dkeys {
                state.editor_deps.entry(dkey).and_modify(|e| {
                    e.remove(&id);
                });
            }
        },
        Msg::UpdateContent(id, _content) => {
            let mut state = state.write();
            let r = state.windows.get_mut(&id)?;
            if let Some(_c) = r.as_raw_content() {
                // Generated content is updated by the renderer-specific paths.
            };
        },
        Msg::UpdateGeneratedContent(id, content) => {
            let mut state = state.write();
            let r = state.windows.get_mut(&id)?;
            if let Some(c) = r.as_generated_content_mut() {
                c.set_generated_content(content);
            };
        },
        Msg::SetRenderEnabled(ctx, id, enabled) => {
            let mut state = state.write();
            let render = state.windows.get_mut(&id)?.as_render_toggle_mut()?;
            if !render.has_renderer() {
                return None;
            }
            render.set_render_enabled(enabled);
            ctx.request_repaint();
        },
        Msg::SetOutputType(ctx, id, output_type) => {
            let mut state_write = state.write();
            let target_svg = state_write
                .windows
                .get(&id)
                .and_then(|window| window.as_target())
                .map(|target| target.get_target());
            let changed = if let Some(render) = state_write
                .windows
                .get_mut(&id)
                .and_then(|window| window.as_render_toggle_mut())
            {
                render.set_output_type(output_type);
                true
            } else {
                false
            };
            if changed {
                let effective_output_type = state_write
                    .windows
                    .get(&id)
                    .and_then(|window| window.as_render_toggle())
                    .map(|render| render.output_type())
                    .unwrap_or_default();
                if let Some(svg_id) = target_svg
                    && let Some(mini_window::Window::SvgWindow(svg_window)) =
                        state_write.windows.get_mut(&svg_id)
                {
                    svg_window.output_type = effective_output_type;
                }
                if let Some(errorable) = state_write
                    .windows
                    .get_mut(&id)
                    .and_then(|window| window.as_error_mut())
                {
                    errorable.set_error(None);
                }
                drop(state_write);
                local_queue.push_back(Msg::Refresh(ctx, id));
            }
        },
        Msg::SetSvgbobEditMode(id, mode) => {
            if let Some(mini_window::Window::SvgbobEditor(editor)) =
                state.write().windows.get_mut(&id)
            {
                editor.set_edit_mode(mode);
            }
        },
        Msg::RequestRedraw(ctx, id) => {
            let (svg_string, scale, background, deps) = {
                let state_r = state.read();
                let mini_window::Window::SvgWindow(reference) = state_r.windows.get(&id)? else {
                    return None;
                };
                let svg_string = reference.svg_string.clone()?;
                let scale = reference.scale;
                let deps = state_r
                    .editor_deps
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>();
                (svg_string, scale, state_r.diagram_background, deps)
            };
            let background = background.resolve(&ctx.style().visuals);
            let image = crate::image::render_svg_to_image(
                &svg_string,
                scale,
                crate::image::RenderBackground::Color(background),
            )?;

            {
                let texture =
                    ctx.load_texture("pikchr_diagram", image, egui::TextureOptions::LINEAR);
                install_diagram_texture(&state, id, texture)?;
            }

            for dep_id in deps {
                if dep_id == id {
                    return None;
                }
                local_queue.push_back(Msg::RequestRedraw(ctx.clone(), dep_id));
            }

            ctx.request_repaint();
        },
        Msg::UpdateRender(ctx, id, content) => {
            let (content, output_type, svg_id) = {
                let mut writable_state = state.write();
                let content = match crate::replace_content(&mut writable_state, id, &content) {
                    Ok(content) => content,
                    Err(err) => {
                        if let Some(errorable) = writable_state
                            .windows
                            .get_mut(&id)
                            .and_then(|w| w.as_error_mut())
                        {
                            errorable.set_error(Some(err.clone()));
                        }
                        writable_state.log.push(err);
                        ctx.request_repaint();
                        return Some(());
                    },
                };
                let window = writable_state.windows.get(&id)?;
                let svg_id = window.as_target().map(|t| t.get_target())?;
                let output_type = window
                    .as_render_toggle()
                    .map(|render| render.output_type())
                    .unwrap_or_default();
                (content, output_type, svg_id)
            };
            let svg_maybe = crate::render::render(output_type, &content);

            let mut writable_state = state.write();
            match svg_maybe {
                Err(err) => {
                    if let Some(errorable) = writable_state
                        .windows
                        .get_mut(&id)
                        .and_then(|w| w.as_error_mut())
                    {
                        errorable.set_error(Some(err.clone()));
                    };
                    writable_state.log.push(err);
                },
                Ok(svg_string) => {
                    local_queue.push_back(Msg::ResetError(id));
                    if let Some(reference) = writable_state
                        .windows
                        .get_mut(&svg_id)
                        .and_then(|s| s.as_svg_window())
                    {
                        *reference.svg_string = Some(svg_string);
                        local_queue.push_back(Msg::RequestRedraw(ctx.clone(), svg_id));
                    } else {
                        local_queue.push_back(Msg::RecreateSvg(ctx.clone(), id))
                    }
                },
            }
            let deps: Vec<egui::Id> = writable_state
                .editor_deps
                .get(&id)
                .into_iter()
                .flatten()
                .copied()
                .collect();
            for dep in deps {
                local_queue.push_back(Msg::Refresh(ctx.clone(), dep))
            }

            ctx.request_repaint();
        },
        Msg::RecreateSvg(ctx, id) => {
            let svg_id = identifiers::next_global_id();
            let mut state_write = state.write();
            let output_type = state_write
                .windows
                .get(&id)
                .and_then(|window| window.as_render_toggle())
                .map(|render| render.output_type())
                .unwrap_or_default();
            let mut svg_window = svg::SvgWindow::new(svg_id, id);
            svg_window.output_type = output_type;
            let svg_insert = mini_window::Window::SvgWindow(svg_window);
            state_write.windows.insert(svg_id, svg_insert);

            let content = state_write
                .windows
                .get(&id)?
                .as_generated_content()?
                .get_generated_content();
            if let Some(targetable) = state_write
                .windows
                .get_mut(&id)
                .and_then(|w| w.as_target_mut())
            {
                targetable.set_target(svg_id);
            }
            local_queue.push_back(Msg::UpdateRender(ctx, id, content));
        },
        Msg::UpdateProlog(ctx, id, content) => {
            // Logic for immediate updates
            let content = {
                let mut state_write = state.write();
                match crate::replace_content(&mut state_write, id, &content) {
                    Ok(content) => content,
                    Err(err) => {
                        if let Some(errorable) = state_write
                            .windows
                            .get_mut(&id)
                            .and_then(|w| w.as_error_mut())
                        {
                            errorable.set_error(Some(err.clone()));
                        }
                        state_write.log.push(err);
                        ctx.request_repaint();
                        return Some(());
                    },
                }
            };
            let pikchr_code =
                pikchr_pro::prolog::engine::trealla::EngineAsync::process_diagram(vec![content])
                    .await;

            match pikchr_code {
                Err(err) => {
                    let mut state_write = state.write();
                    state_write.log.push(format!("{:?}", err));
                    if let Some(errorable) = state_write
                        .windows
                        .get_mut(&id)
                        .and_then(|w| w.as_error_mut())
                    {
                        errorable.set_error(Some(err.inner_string()));
                    }
                    ctx.request_repaint();
                },
                Ok(pikchr) => {
                    local_queue.push_back(Msg::Batch(vec![
                        Msg::ResetError(id),
                        Msg::UpdateGeneratedContent(id, pikchr.clone().into_inner()),
                        Msg::UpdateRender(ctx.clone(), id, pikchr.into_inner()),
                    ]));
                },
            }
            for dep in state
                .write()
                .editor_deps
                .get(&id)
                .unwrap_or(&HashSet::new())
            {
                local_queue.push_back(Msg::Refresh(ctx.clone(), *dep))
            }
        },
        Msg::UpdateTcl(ctx, id, content) => {
            // Logic for immediate updates
            let content = {
                let mut state_write = state.write();
                match crate::replace_content(&mut state_write, id, &content) {
                    Ok(content) => content,
                    Err(err) => {
                        if let Some(errorable) = state_write
                            .windows
                            .get_mut(&id)
                            .and_then(|w| w.as_error_mut())
                        {
                            errorable.set_error(Some(err.clone()));
                        }
                        state_write.log.push(err);
                        ctx.request_repaint();
                        return Some(());
                    },
                }
            };

            let pikchr_code = tcl::safe_eval_tcl(content).await;

            match pikchr_code {
                Err(err) => {
                    let mut state_write = state.write();
                    state_write.log.push(format!("{:?}", err));
                    if let Some(errorable) = state_write
                        .windows
                        .get_mut(&id)
                        .and_then(|w| w.as_error_mut())
                    {
                        errorable.set_error(Some(err));
                    }
                    ctx.request_repaint();
                },
                Ok(pikchr) => {
                    local_queue.push_back(Msg::Batch(vec![
                        Msg::ResetError(id),
                        Msg::UpdateGeneratedContent(id, pikchr.as_str().into()),
                        Msg::UpdateRender(ctx.clone(), id, pikchr),
                    ]));
                },
            }

            let deps = state.read().editor_deps.clone();
            debug!(logger, "Dependency handling"; "id" => id.short_debug_format(),
                "deps" => Serde(deps));
            for dep in state.read().editor_deps.get(&id).unwrap_or(&HashSet::new()) {
                local_queue.push_back(Msg::Refresh(ctx.clone(), *dep))
            }
        },
        Msg::UpdateMruby(ctx, id, content) => {
            let content = {
                let mut state_write = state.write();
                match crate::replace_content(&mut state_write, id, &content) {
                    Ok(content) => content,
                    Err(err) => {
                        if let Some(errorable) = state_write
                            .windows
                            .get_mut(&id)
                            .and_then(|w| w.as_error_mut())
                        {
                            errorable.set_error(Some(err.clone()));
                        }
                        state_write.log.push(err);
                        ctx.request_repaint();
                        return Some(());
                    },
                }
            };
            let pikchr_code = mruby::safe_eval_mruby(content).await;

            match pikchr_code {
                Err(err) => {
                    let mut state_write = state.write();
                    state_write.log.push(format!("{:?}", err));
                    if let Some(errorable) = state_write
                        .windows
                        .get_mut(&id)
                        .and_then(|w| w.as_error_mut())
                    {
                        errorable.set_error(Some(err));
                    }
                    ctx.request_repaint();
                },
                Ok(pikchr) => {
                    local_queue.push_back(Msg::Batch(vec![
                        Msg::ResetError(id),
                        Msg::UpdateGeneratedContent(id, pikchr.clone()),
                        Msg::UpdateRender(ctx.clone(), id, pikchr),
                    ]));
                },
            }

            for dep in state.read().editor_deps.get(&id).unwrap_or(&HashSet::new()) {
                local_queue.push_back(Msg::Refresh(ctx.clone(), *dep))
            }
        },
        Msg::UpdatePlainText(ctx, id) => {
            for dep in state.read().editor_deps.get(&id).unwrap_or(&HashSet::new()) {
                local_queue.push_back(Msg::Refresh(ctx.clone(), *dep))
            }
        },
        Msg::ToggleWindow(crate::Window::Logger) => {
            let mut state_write = state.write();
            let current = state_write.window_states.log;
            state_write.window_states.log = !current;
        },
        Msg::ToggleWindow(crate::Window::Debugger) => {
            let mut state_write = state.write();
            let current = state_write.window_states.debug;
            state_write.window_states.debug = !current;
        },
        Msg::ToggleWindowById(id) => {
            if let Some(window) = state
                .write()
                .windows
                .get_mut(&id)
                .and_then(|w| w.as_mini_window_mut())
            {
                window.toggle_visible();
            }
        },
        Msg::NewWindow(ctx, window_type) => match window_type {
            crate::mini_window::WindowType::PikchrEditor => {
                let editor_id =
                    create_editor_window!(state, PikchrEditor, pikchr_editor::PikchrEditor::new);
                local_queue.push_back(Msg::Refresh(ctx, editor_id));
            },
            crate::mini_window::WindowType::SvgbobEditor => {
                let editor_id =
                    create_editor_window!(state, SvgbobEditor, svgbob_editor::SvgbobEditor::new);
                local_queue.push_back(Msg::Refresh(ctx, editor_id));
            },
            crate::mini_window::WindowType::PrologEditor => {
                let editor_id =
                    create_editor_window!(state, PrologEditor, prolog_editor::PrologEditor::new);
                local_queue.push_back(Msg::Refresh(ctx, editor_id));
            },
            crate::mini_window::WindowType::TclEditor => {
                let editor_id = create_editor_window!(state, TclEditor, tcl_editor::TclEditor::new);
                local_queue.push_back(Msg::Refresh(ctx, editor_id));
            },
            crate::mini_window::WindowType::MrubyEditor => {
                let editor_id =
                    create_editor_window!(state, MrubyEditor, mruby_editor::MrubyEditor::new);
                local_queue.push_back(Msg::Refresh(ctx, editor_id));
            },
            crate::mini_window::WindowType::PlainTextEditor => {
                create_plain_text_window!(state);
            },
            crate::mini_window::WindowType::SvgWindow => (),
            crate::mini_window::WindowType::HelpWindow => {
                create_help_window!(state, crate::help::HelpTopic::Overview);
            },
        },
        Msg::ExportModal(id, name, export_type) => {
            push_modal!(state, ExportModal::new(id, name, export_type));
        },
        Msg::Export(svg_id, file, crate::ExportType::Png, visuals) => {
            let background = state.read().diagram_background.resolve_for_export(&visuals);
            let mut state = state.write();
            let svg_string = state
                .windows
                .get_mut(&svg_id)
                .and_then(|w| w.as_svg_window())
                .and_then(|s| s.svg_string.clone())?;
            let image = crate::image::render_svg_to_image(&svg_string, 2.0, background)?;
            let _ = crate::image::write_png(file, image);
            local_queue.push_back(Msg::PopModal);
        },
        Msg::Export(svg_id, file, crate::ExportType::PngTransparent, _visuals) => {
            let mut state = state.write();
            let svg_string = state
                .windows
                .get_mut(&svg_id)
                .and_then(|w| w.as_svg_window())
                .and_then(|s| s.svg_string.clone())?;
            let image = crate::image::render_svg_to_image(
                &svg_string,
                2.0,
                crate::image::RenderBackground::Transparent,
            )?;
            let _ = crate::image::write_png(file, image);
            local_queue.push_back(Msg::PopModal);
        },
        Msg::Export(svg_id, file, crate::ExportType::Svg, _visuals) => {
            let mut state = state.write();
            let svg = state
                .windows
                .get_mut(&svg_id)
                .and_then(|w| w.as_svg_window())
                .and_then(|s| s.svg_string.clone())?;
            let _ = crate::image::write_svg(file, svg);
            local_queue.push_back(Msg::PopModal);
        },
        Msg::ExportSourceToClipboard(ctx, id) => {
            let content = state
                .read()
                .windows
                .get(&id)?
                .as_generated_content()?
                .get_generated_content();

            let content = match crate::replace_content(&mut state.write(), id, &content) {
                Ok(content) => content,
                Err(err) => {
                    state.write().log.push(err);
                    return Some(());
                },
            };
            ctx.copy_text(content);
        },
        Msg::PopModal => {
            state.write().modals.pop_front();
        },
        Msg::ReloadSvgs(ctx) => {
            for w in state
                .write()
                .windows
                .values_mut()
                .flat_map(|m| m.as_svg_window())
            {
                local_queue.push_back(Msg::RequestRedraw(ctx.clone(), *w.id))
            }
        },
        Msg::ResetError(id) => {
            if let Some(errorable) = state
                .write()
                .windows
                .get_mut(&id)
                .and_then(|w| w.as_error_mut())
            {
                errorable.set_error(None);
            }
        },
        Msg::ResetWorkspace => {
            // Clears the *active* workspace only; dormant ones are untouched.
            let mut state = state.write();
            state.windows = HashMap::new();
            state.editor_deps = HashMap::new();
            state.window_library_paths = HashMap::new();
            // NOTE: do NOT call flush_active() here — it would insert the
            // active workspace into the dormant `workspaces` map, causing it
            // to appear twice in workspace_listing() (once live, once dormant).
            drop(state);
            local_queue.push_back(Msg::PopModal);
        },
        Msg::ResetWorkspaceRequest => {
            push_modal!(
                state,
                ConfirmationModal::new(Msg::ResetWorkspace, "Reset active workspace?")
            );
        },
        Msg::SaveWorkspace => {
            let cloned: AppState = state.clone().read().clone();
            // Export only the *active* workspace as a single-workspace file
            // (backward compatible with pre-workspace save format).
            let persisted = AppStatePersistent::from(cloned);
            let export = AppStatePersistent {
                workspaces: HashMap::new(),
                active_workspace_id: 0,
                active_workspace_name: persisted.active_workspace_name.clone(),
                library: Default::default(),
                ..persisted
            };
            let payload: Box<[u8]> = serde_json::to_vec(&export).unwrap().into_boxed_slice();
            push_modal!(
                state,
                FileSaveModal::new(payload, "json", "workspace", Some("Save Workspace"))
            );
        },
        Msg::SaveEditorToLibraryRequest(ctx, id) => {
            let initial = {
                let state = state.read();
                state
                    .window_library_paths
                    .get(&id)
                    .cloned()
                    .or_else(|| {
                        state
                            .windows
                            .get(&id)?
                            .as_name()
                            .map(|name| name.get_name())
                    })
                    .unwrap_or_default()
            };
            push_modal!(state, SaveToLibraryModal::new(id, &initial));
            ctx.request_repaint();
        },
        Msg::SaveEditorToLibrary {
            editor_id,
            path,
            overwrite,
        } => {
            let path = clean_library_path(path);
            if path.is_empty() {
                return None;
            }

            let entry = {
                let state_r = state.read();
                let window = state_r.windows.get(&editor_id)?;
                let editor_type = window.as_editor_type()?.get_editor_type();
                let output_type = window
                    .as_render_toggle()
                    .map(|render| render.output_type())
                    .unwrap_or_default();
                let content = window.as_raw_content()?.get_raw_content();
                LibraryEntry {
                    path: path.clone(),
                    editor_type,
                    output_type,
                    content,
                }
            };

            let exists = state.read().library.contains_key(&path);
            if exists && !overwrite {
                let mut state = state.write();
                state.modals.pop_front();
                state
                    .modals
                    .push_back(Arc::new(RwLock::new(ConfirmationModal::new(
                        Msg::SaveEditorToLibrary {
                            editor_id,
                            path,
                            overwrite: true,
                        },
                        "Overwrite existing library entry?",
                    ))));
                return Some(());
            }

            let mut state = state.write();
            state.library.insert(path.clone(), entry);
            state.window_library_paths.insert(editor_id, path);
            drop(state);
            local_queue.push_back(Msg::PopModal);
        },
        Msg::ExportEditorLibraryEntry(editor_id) => {
            let entry = {
                let state = state.read();
                let window = state.windows.get(&editor_id)?;
                let path = state
                    .window_library_paths
                    .get(&editor_id)
                    .cloned()
                    .or_else(|| window.as_name().map(|name| name.get_name()))
                    .map(clean_library_path)
                    .filter(|path| !path.is_empty())?;
                LibraryEntry {
                    path,
                    editor_type: window.as_editor_type()?.get_editor_type(),
                    output_type: window
                        .as_render_toggle()
                        .map(|render| render.output_type())
                        .unwrap_or_default(),
                    content: window.as_raw_content()?.get_raw_content(),
                }
            };
            export_library_entry_to_json(&state, &entry);
        },
        Msg::LoadWorkspaceRequest => {
            let modal = FileOpenModal::new(
                "Load Workspace",
                "json",
                Box::new(|path, _ctx, tx: Sender<Msg>| {
                    let _ = tx.try_send(Msg::LoadWorkspace(path));
                    Ok(())
                }),
            );
            push_modal!(state, modal);
        },
        Msg::LoadWorkspace(path) => {
            let path = std::path::Path::new(&path);
            if !path.exists() {
                return None;
            }
            let Ok(file) = std::fs::File::open(path) else {
                eprintln!("Can't open file");
                return None;
            };
            let reader = BufReader::new(file);
            let Ok(imported) = serde_json::from_reader::<_, AppState>(reader) else {
                eprintln!("Can't deserialize state");
                return None;
            };
            // Import the file's active workspace as a brand-new workspace
            // (fresh id ⇒ dormant ids never collide) and switch to it.
            let mut current = state.write();
            let new_id = current.new_workspace(imported.active_workspace_name.clone());
            if let Some(ws) = current.workspaces.get_mut(&new_id) {
                ws.windows = imported.windows;
                ws.editor_deps = imported.editor_deps;
                ws.window_library_paths = imported.window_library_paths;
            }
            current.switch_to(new_id);
            drop(current);
            local_queue.push_back(Msg::PopModal);
        },
        Msg::OpenLibraryEntry(ctx, path) => {
            let entry = state.read().library.get(&path).cloned()?;
            let editor_id = create_window_from_library_entry(&state, &entry)?;
            local_queue.push_back(Msg::Refresh(ctx, editor_id));
        },
        Msg::DeleteLibraryEntryRequest(path) => {
            push_modal!(
                state,
                ConfirmationModal::new(Msg::DeleteLibraryEntry(path), "Delete library entry?")
            );
        },
        Msg::DeleteLibraryEntry(path) => {
            let mut state = state.write();
            state.library.remove(&path);
            state.window_library_paths.retain(|_, value| value != &path);
            drop(state);
            local_queue.push_back(Msg::PopModal);
        },
        Msg::ExportLibraryEntry(path) => {
            let entry = state.read().library.get(&path).cloned()?;
            export_library_entry_to_json(&state, &entry);
        },
        Msg::ImportLibraryEntries => {
            let Some(files) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_files()
            else {
                return Some(());
            };
            for file in files {
                match std::fs::File::open(&file)
                    .map(BufReader::new)
                    .map_err(|err| err.to_string())
                    .and_then(|reader| {
                        serde_json::from_reader::<_, LibraryEntry>(reader)
                            .map_err(|err| err.to_string())
                    }) {
                    Ok(mut entry) => {
                        entry.path = clean_library_path(entry.path);
                        if !entry.path.is_empty() {
                            local_queue.push_back(Msg::ImportLibraryEntry(entry, false));
                        }
                    },
                    Err(err) => {
                        state
                            .write()
                            .log
                            .push(format!("Could not import library entry: {err}"));
                    },
                }
            }
        },
        Msg::ImportLibraryEntry(entry, overwrite) => {
            let exists = state.read().library.contains_key(&entry.path);
            if exists && !overwrite {
                push_modal!(
                    state,
                    ConfirmationModal::new(
                        Msg::ImportLibraryEntry(entry, true),
                        "Overwrite existing library entry?"
                    )
                );
                return Some(());
            }
            state.write().library.insert(entry.path.clone(), entry);
            if overwrite {
                local_queue.push_back(Msg::PopModal);
            }
        },

        // ── Multiple workspaces ─────────────────────────────────────────
        Msg::SwitchWorkspace(id) => {
            state.write().switch_to(id);
            // Editor contents are refreshed by the UI loop when it notices
            // the active workspace id has changed (see DiagramIDE::ui).
        },
        Msg::NewWorkspaceRequest => {
            push_modal!(state, WorkspaceNameModal::new(None, ""));
        },
        Msg::NewWorkspace(name) => {
            let mut state = state.write();
            let id = state.new_workspace(name);
            state.switch_to(id);
        },
        Msg::RenameWorkspaceRequest(id) => {
            let initial = state
                .read()
                .workspace_listing()
                .into_iter()
                .find(|(wid, _, _)| wid == &id)
                .map(|(_, name, _)| name)
                .unwrap_or_default();
            push_modal!(state, WorkspaceNameModal::new(Some(id), &initial));
        },
        Msg::RenameWorkspace(id, name) => {
            state.write().rename_workspace(id, name);
        },
        Msg::DuplicateWorkspace(id) => {
            let mut state = state.write();
            if id == state.active_workspace_id {
                state.duplicate_active();
            } else {
                // duplicate a dormant workspace in place
                if let Some(src) = state.workspaces.get(&id).cloned() {
                    let new_id = identifiers::next_workspace_id();
                    state.workspaces.insert(
                        new_id,
                        Workspace {
                            id: new_id,
                            name: format!("{} (copy)", src.name),
                            windows: src.windows,
                            editor_deps: src.editor_deps,
                            window_library_paths: src.window_library_paths,
                        },
                    );
                }
            }
        },
        Msg::DeleteWorkspaceRequest(id) => {
            // No confirmation — delete immediately.
            state.write().delete_workspace(id);
        },
        Msg::FontSizeModal(_id) => {
            let value = String::from("abc");
            let value = Box::leak(Box::new(value));
            let modal = StringEditModal::new("VALUE", value);
            push_modal!(state, modal);
        },
        Msg::RequestRename(ctx, id) => {
            let name = {
                let state_r = state.read();
                state_r.windows.get(&id)?.as_name()?.get_name()
            };
            let modal = RenameModal::new(id, &name);
            push_modal!(state, modal);
            ctx.request_repaint();
        },
        Msg::RenameWindow(id, new_title) => {
            let target = {
                let state_r = state.read();
                state_r
                    .windows
                    .get(&id)?
                    .as_target()
                    .map(|target| target.get_target())
            };
            if let Some(svg_id) = target {
                local_queue.push_back(Msg::RenameWindow(svg_id, new_title.clone()));
            }
            let mut state = state.write();
            let as_name = state.windows.get_mut(&id)?.as_name_mut()?;
            as_name.set_name(new_title);
        },
        Msg::Refresh(ctx, id) => {
            let state = state.read();
            let window = state.windows.get(&id)?;
            let et = window.as_editor_type()?.get_editor_type();

            let content = window.as_raw_content()?.get_raw_content();

            local_queue.push_back(match et {
                crate::EditorType::Prolog => Msg::UpdateProlog(ctx, id, content),
                crate::EditorType::Pikchr => Msg::UpdateRender(ctx, id, content),
                crate::EditorType::Svgbob => Msg::UpdateRender(ctx, id, content),
                crate::EditorType::Tcl => Msg::UpdateTcl(ctx, id, content),
                crate::EditorType::Mruby => Msg::UpdateMruby(ctx, id, content),
                crate::EditorType::PlainText => Msg::UpdatePlainText(ctx, id),
            });
        },
        Msg::RefreshWorkspace(ctx) => {
            let ids: Vec<egui::Id> = state
                .read()
                .windows
                .iter()
                .filter_map(|(id, window)| window.as_editor_type().map(|_| *id))
                .collect();
            for id in ids {
                local_queue.push_back(Msg::Refresh(ctx.clone(), id));
            }
        },
        Msg::CheckDependencies => {
            clean_old_deps(&mut state.write());
        },
    };
    Some(())
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use parking_lot::RwLock;

    use super::*;

    fn editor_matches(window: &mini_window::Window, editor_type: crate::EditorType) -> bool {
        matches!(
            (window, editor_type),
            (
                mini_window::Window::PikchrEditor(_),
                crate::EditorType::Pikchr
            ) | (
                mini_window::Window::SvgbobEditor(_),
                crate::EditorType::Svgbob
            ) | (
                mini_window::Window::PrologEditor(_),
                crate::EditorType::Prolog
            ) | (mini_window::Window::TclEditor(_), crate::EditorType::Tcl)
                | (
                    mini_window::Window::MrubyEditor(_),
                    crate::EditorType::Mruby
                )
                | (
                    mini_window::Window::PlainTextEditor(_),
                    crate::EditorType::PlainText
                )
        )
    }

    #[test]
    fn texture_install_waits_for_transient_state_contention() {
        let id = egui::Id::new("svg");
        let owner_id = egui::Id::new("owner");
        let state = Arc::new(RwLock::new(AppState::default()));
        state.write().windows.insert(
            id,
            mini_window::Window::SvgWindow(svg::SvgWindow::new(id, owner_id)),
        );
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "contention-test",
            egui::ColorImage::new([1, 1], vec![egui::Color32::WHITE]),
            egui::TextureOptions::LINEAR,
        );

        let state_guard = state.write();
        let state_for_thread = state.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let installer = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let installed = install_diagram_texture(&state_for_thread, id, texture).is_some();
            done_tx.send(installed).unwrap();
        });

        started_rx.recv().unwrap();
        assert!(
            done_rx
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err(),
            "texture installation should wait instead of dropping the redraw"
        );
        drop(state_guard);

        assert!(
            done_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .unwrap()
        );
        installer.join().unwrap();
        assert!(matches!(
            state.read().windows.get(&id),
            Some(mini_window::Window::SvgWindow(window))
                if window.diagram_texture.is_some()
        ));
    }

    #[tokio::test]
    async fn refresh_workspace_queues_refresh_for_each_editor() {
        let pikchr_id = egui::Id::new("pikchr");
        let plain_id = egui::Id::new("plain");
        let svg_id = egui::Id::new("svg");
        let ctx = egui::Context::default();
        let state = Arc::new(RwLock::new(AppState::default()));
        {
            let mut state = state.write();
            state.windows.insert(
                pikchr_id,
                mini_window::Window::PikchrEditor(pikchr_editor::PikchrEditor::new(
                    pikchr_id, svg_id,
                )),
            );
            state.windows.insert(
                plain_id,
                mini_window::Window::PlainTextEditor(plain_text_editor::PlainTextEditor::new(
                    plain_id,
                )),
            );
            state.windows.insert(
                svg_id,
                mini_window::Window::SvgWindow(svg::SvgWindow::new(svg_id, pikchr_id)),
            );
        }

        let mut local_queue = VecDeque::new();
        handle_event(
            crate::logger::init_logger(),
            Msg::RefreshWorkspace(ctx),
            state,
            &mut local_queue,
        )
        .await;

        let refreshed: HashSet<egui::Id> = local_queue
            .into_iter()
            .filter_map(|msg| match msg {
                Msg::Refresh(_, id) => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(refreshed, HashSet::from([pikchr_id, plain_id]));
    }

    #[tokio::test]
    async fn render_toggle_is_owned_by_editor() {
        let editor_id = egui::Id::new("pikchr");
        let svg_id = egui::Id::new("svg");
        let ctx = egui::Context::default();
        let state = Arc::new(RwLock::new(AppState::default()));
        state.write().windows.insert(
            editor_id,
            mini_window::Window::PikchrEditor(pikchr_editor::PikchrEditor::new(editor_id, svg_id)),
        );
        let mut local_queue = VecDeque::new();

        handle_event(
            crate::logger::init_logger(),
            Msg::SetRenderEnabled(ctx.clone(), editor_id, false),
            state.clone(),
            &mut local_queue,
        )
        .await;
        assert!(
            !state
                .read()
                .windows
                .get(&editor_id)
                .unwrap()
                .as_render_toggle()
                .unwrap()
                .render_enabled()
        );

        handle_event(
            crate::logger::init_logger(),
            Msg::SetRenderEnabled(ctx, editor_id, true),
            state.clone(),
            &mut local_queue,
        )
        .await;
        assert!(
            state
                .read()
                .windows
                .get(&editor_id)
                .unwrap()
                .as_render_toggle()
                .unwrap()
                .render_enabled()
        );
    }

    #[tokio::test]
    async fn creating_mruby_editor_queues_initial_refresh() {
        let ctx = egui::Context::default();
        let state = Arc::new(RwLock::new(AppState::default()));
        let mut local_queue = VecDeque::new();

        handle_event(
            crate::logger::init_logger(),
            Msg::NewWindow(ctx, crate::mini_window::WindowType::MrubyEditor),
            state.clone(),
            &mut local_queue,
        )
        .await;

        let editor_id = state
            .read()
            .windows
            .iter()
            .find_map(|(id, window)| {
                matches!(window, mini_window::Window::MrubyEditor(_)).then_some(*id)
            })
            .expect("mruby editor should be created");

        assert!(
            local_queue
                .into_iter()
                .any(|msg| { matches!(msg, Msg::Refresh(_, id) if id == editor_id) })
        );
    }

    #[tokio::test]
    async fn creating_svgbob_editor_queues_initial_refresh() {
        let ctx = egui::Context::default();
        let state = Arc::new(RwLock::new(AppState::default()));
        let mut local_queue = VecDeque::new();

        handle_event(
            crate::logger::init_logger(),
            Msg::NewWindow(ctx, crate::mini_window::WindowType::SvgbobEditor),
            state.clone(),
            &mut local_queue,
        )
        .await;

        let editor_id = state
            .read()
            .windows
            .iter()
            .find_map(|(id, window)| {
                matches!(window, mini_window::Window::SvgbobEditor(_)).then_some(*id)
            })
            .expect("svgbob editor should be created");

        assert!(
            local_queue
                .iter()
                .any(|msg| { matches!(msg, Msg::Refresh(_, id) if *id == editor_id) })
        );
    }

    #[tokio::test]
    async fn changing_svgbob_mode_persists_on_the_dedicated_editor() {
        let editor_id = egui::Id::new("svgbob");
        let svg_id = egui::Id::new("svg");
        let state = Arc::new(RwLock::new(AppState::default()));
        state.write().windows.insert(
            editor_id,
            mini_window::Window::SvgbobEditor(svgbob_editor::SvgbobEditor::new(editor_id, svg_id)),
        );

        handle_event(
            crate::logger::init_logger(),
            Msg::SetSvgbobEditMode(editor_id, crate::SvgbobEditMode::Replace),
            state.clone(),
            &mut VecDeque::new(),
        )
        .await;

        assert!(matches!(
            state.read().windows.get(&editor_id),
            Some(mini_window::Window::SvgbobEditor(editor))
                if editor.edit_mode() == crate::SvgbobEditMode::Replace
        ));
    }

    #[tokio::test]
    async fn changing_output_type_updates_editor_preview_and_refreshes() {
        let editor_id = egui::Id::new("editor");
        let svg_id = egui::Id::new("svg");
        let ctx = egui::Context::default();
        let state = Arc::new(RwLock::new(AppState::default()));
        {
            let mut state = state.write();
            state.windows.insert(
                editor_id,
                mini_window::Window::PikchrEditor(pikchr_editor::PikchrEditor::new(
                    editor_id, svg_id,
                )),
            );
            state.windows.insert(
                svg_id,
                mini_window::Window::SvgWindow(svg::SvgWindow::new(svg_id, editor_id)),
            );
        }

        let mut local_queue = VecDeque::new();
        handle_event(
            crate::logger::init_logger(),
            Msg::SetOutputType(ctx, editor_id, crate::OutputType::Svgbob),
            state.clone(),
            &mut local_queue,
        )
        .await;

        let state_read = state.read();
        assert_eq!(
            state_read
                .windows
                .get(&editor_id)
                .and_then(|window| window.as_render_toggle())
                .map(|render| render.output_type()),
            Some(crate::OutputType::Svgbob)
        );
        assert!(matches!(
            state_read.windows.get(&svg_id),
            Some(mini_window::Window::SvgWindow(svg)) if svg.output_type == crate::OutputType::Svgbob
        ));
        assert!(
            local_queue
                .iter()
                .any(|msg| matches!(msg, Msg::Refresh(_, id) if *id == editor_id))
        );
    }

    #[tokio::test]
    async fn updating_pikchr_dependency_refreshes_dependent_from_its_own_content() {
        let source_id = egui::Id::new("source");
        let source_svg_id = egui::Id::new("source-svg");
        let dependent_id = egui::Id::new("dependent");
        let dependent_svg_id = egui::Id::new("dependent-svg");
        let ctx = egui::Context::default();
        let state = Arc::new(RwLock::new(AppState::default()));
        {
            let mut state = state.write();
            state.windows.insert(
                source_id,
                mini_window::Window::PikchrEditor(pikchr_editor::PikchrEditor::new(
                    source_id,
                    source_svg_id,
                )),
            );
            state.windows.insert(
                source_svg_id,
                mini_window::Window::SvgWindow(svg::SvgWindow::new(source_svg_id, source_id)),
            );
            state.windows.insert(
                dependent_id,
                mini_window::Window::PikchrEditor(pikchr_editor::PikchrEditor::new(
                    dependent_id,
                    dependent_svg_id,
                )),
            );
            state.windows.insert(
                dependent_svg_id,
                mini_window::Window::SvgWindow(svg::SvgWindow::new(dependent_svg_id, dependent_id)),
            );
            state
                .editor_deps
                .entry(source_id)
                .or_default()
                .insert(dependent_id);
        }

        let mut local_queue = VecDeque::new();
        handle_event(
            crate::logger::init_logger(),
            Msg::UpdateRender(ctx, source_id, "box".into()),
            state,
            &mut local_queue,
        )
        .await;

        assert!(
            local_queue
                .iter()
                .any(|msg| matches!(msg, Msg::Refresh(_, id) if *id == dependent_id))
        );
        assert!(
            !local_queue
                .iter()
                .any(|msg| matches!(msg, Msg::UpdateRender(_, id, content) if *id == dependent_id && content == "box"))
        );
    }

    #[tokio::test]
    async fn rename_request_queues_modal_repaints_and_can_be_confirmed() {
        let id = egui::Id::new("editor");
        let ctx = egui::Context::default();
        let repaint_requested = Arc::new(AtomicBool::new(false));
        let repaint_requested_clone = repaint_requested.clone();
        ctx.set_request_repaint_callback(move |_| {
            repaint_requested_clone.store(true, Ordering::SeqCst);
        });

        let state = Arc::new(RwLock::new(AppState::default()));
        state.write().windows.insert(
            id,
            mini_window::Window::PlainTextEditor(plain_text_editor::PlainTextEditor::new(id)),
        );

        let mut local_queue = VecDeque::new();
        handle_event(
            crate::logger::init_logger(),
            Msg::RequestRename(ctx, id),
            state.clone(),
            &mut local_queue,
        )
        .await;

        assert_eq!(state.read().modals.len(), 1);
        assert!(repaint_requested.load(Ordering::SeqCst));

        handle_event(
            crate::logger::init_logger(),
            Msg::RenameWindow(id, "renamed".into()),
            state.clone(),
            &mut local_queue,
        )
        .await;

        let name = state
            .read()
            .windows
            .get(&id)
            .and_then(|window| window.as_name())
            .map(|window| window.get_name());
        assert_eq!(name.as_deref(), Some("renamed"));
    }

    #[tokio::test]
    async fn saving_existing_library_path_requires_overwrite_confirmation() {
        let id = egui::Id::new("editor");
        let state = Arc::new(RwLock::new(AppState::default()));
        state.write().windows.insert(
            id,
            mini_window::Window::PlainTextEditor(plain_text_editor::PlainTextEditor::new(id)),
        );

        {
            let mut state = state.write();
            let content = state
                .windows
                .get_mut(&id)
                .and_then(|window| window.as_raw_content_mut())
                .expect("plain text has raw content");
            content.set_raw_content("first".into());
        }

        let mut local_queue = VecDeque::new();
        handle_event(
            crate::logger::init_logger(),
            Msg::SaveEditorToLibrary {
                editor_id: id,
                path: "samples/plain".into(),
                overwrite: false,
            },
            state.clone(),
            &mut local_queue,
        )
        .await;
        assert_eq!(state.read().library["samples/plain"].content, "first");

        {
            let mut state = state.write();
            let content = state
                .windows
                .get_mut(&id)
                .and_then(|window| window.as_raw_content_mut())
                .expect("plain text has raw content");
            content.set_raw_content("second".into());
        }

        handle_event(
            crate::logger::init_logger(),
            Msg::SaveEditorToLibrary {
                editor_id: id,
                path: "samples/plain".into(),
                overwrite: false,
            },
            state.clone(),
            &mut local_queue,
        )
        .await;
        assert_eq!(state.read().library["samples/plain"].content, "first");
        assert_eq!(state.read().modals.len(), 1);

        handle_event(
            crate::logger::init_logger(),
            Msg::SaveEditorToLibrary {
                editor_id: id,
                path: "samples/plain".into(),
                overwrite: true,
            },
            state.clone(),
            &mut local_queue,
        )
        .await;
        assert_eq!(state.read().library["samples/plain"].content, "second");
        assert_eq!(
            state
                .read()
                .window_library_paths
                .get(&id)
                .map(String::as_str),
            Some("samples/plain")
        );
    }

    #[tokio::test]
    async fn opening_library_entries_creates_matching_editors() {
        let ctx = egui::Context::default();
        for editor_type in [
            crate::EditorType::Pikchr,
            crate::EditorType::Svgbob,
            crate::EditorType::Prolog,
            crate::EditorType::Tcl,
            crate::EditorType::Mruby,
            crate::EditorType::PlainText,
        ] {
            let state = Arc::new(RwLock::new(AppState::default()));
            let output_type = if matches!(
                editor_type,
                crate::EditorType::Pikchr | crate::EditorType::Svgbob
            ) {
                crate::OutputType::Svgbob
            } else {
                crate::OutputType::Pikchr
            };
            let entry = LibraryEntry {
                path: format!("folder/{editor_type:?}"),
                editor_type,
                output_type,
                content: format!("content for {editor_type:?}"),
            };
            state
                .write()
                .library
                .insert(entry.path.clone(), entry.clone());

            let mut local_queue = VecDeque::new();
            handle_event(
                crate::logger::init_logger(),
                Msg::OpenLibraryEntry(ctx.clone(), entry.path.clone()),
                state.clone(),
                &mut local_queue,
            )
            .await;

            let state_read = state.read();
            let (id, window) = state_read
                .windows
                .iter()
                .find(|(_, window)| editor_matches(window, editor_type))
                .expect("matching editor should be created");
            assert_eq!(
                window
                    .as_raw_content()
                    .map(|content| content.get_raw_content()),
                Some(entry.content.clone())
            );
            assert_eq!(
                state_read.window_library_paths.get(id).map(String::as_str),
                Some(entry.path.as_str())
            );
            if editor_type != crate::EditorType::PlainText {
                assert_eq!(
                    window.as_render_toggle().map(|render| render.output_type()),
                    Some(if editor_type == crate::EditorType::Svgbob {
                        crate::OutputType::Svgbob
                    } else {
                        output_type
                    })
                );
            }
            assert!(
                local_queue
                    .iter()
                    .any(|msg| matches!(msg, Msg::Refresh(_, refresh_id) if refresh_id == id))
            );
        }
    }
}

// kakexec: <percent>s(?S)Msg::.*=<gt>.*\{<ret>mH<a-semicolon>L<space>jf,
