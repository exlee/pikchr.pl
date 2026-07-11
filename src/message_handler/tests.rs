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
            mini_window::Window::PikchrEditor(pikchr_editor::PikchrEditor::new(pikchr_id, svg_id)),
        );
        state.windows.insert(
            plain_id,
            mini_window::Window::PlainTextEditor(plain_text_editor::PlainTextEditor::new(plain_id)),
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
            mini_window::Window::PikchrEditor(pikchr_editor::PikchrEditor::new(editor_id, svg_id)),
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
