use eframe::egui;
use parking_lot::RwLock;
use slog::{Logger, debug, info, o};
use std::sync::Arc;
use tokio::sync::mpsc;

use state::AppState;
use state_serialize::DiagramIDEPersistent;

mod dependencies;
mod editor;
mod editor_state;
pub mod help;
mod icons;
mod identifiers;
mod image;
pub mod logger;
mod menubar;
mod messages;
pub mod message_handler;
mod mini_window;
mod modal;
mod mruby;
mod mruby_editor;
#[cfg(feature = "perf-workloads")]
#[doc(hidden)]
pub mod perf_support;
mod pikchr_editor;
mod plain_text_editor;
mod prolog_editor;
mod render;
mod response_ext;
mod sender_ext;
pub mod state;
mod state_serialize;
mod svg;
mod svgbob_editor;
mod tcl;
mod tcl_editor;
pub mod text_highlighting;
pub mod theme;
pub use messages::{EditorType, ExportType, Msg, OutputType, SvgbobEditMode, Window};
pub(crate) use dependencies::{clean_old_deps, replace_content};
#[cfg(test)]
pub(crate) use dependencies::replace_generated_content;
#[cfg(feature = "perf-workloads")]
pub(crate) use dependencies::perf_dependency_workload;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(from = "DiagramIDEPersistent", into = "DiagramIDEPersistent")]
pub struct DiagramIDE {
    tx: mpsc::Sender<Msg>,
    state: Arc<RwLock<AppState>>,
    pub window_size: egui::Vec2,
    first_frame: bool,
    pub logger: Logger,
    /// Tracks the active workspace id so the UI loop can detect a switch and
    /// refresh SVG textures for the newly-promoted workspace.
    seen_workspace_id: state::WorkspaceId,
}
impl DiagramIDE {
    pub fn new_test(
        ctx: &egui::Context,
        tx: mpsc::Sender<Msg>,
        state: Arc<RwLock<AppState>>,
    ) -> Self {
        egui_extras::install_image_loaders(ctx);
        crate::install_help_fonts(ctx);
        let seen_workspace_id = state.read().active_workspace_id;
        Self {
            tx,
            state,
            first_frame: true,
            window_size: egui::vec2(800.0, 600.0),
            logger: crate::logger::init_logger(),
            seen_workspace_id,
        }
    }
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        // Register the SpaceMono families used by the Grammar help window
        // during construction, so they are bound before the first frame's
        // `Fonts` is built. (set_fonts mid-frame only takes effect next frame,
        // which would panic a restored HelpWindow on frame 1.)
        crate::install_help_fonts(&cc.egui_ctx);
        let logger = crate::logger::init_logger();
        let start_def = || {
            let blank_state = Arc::new(RwLock::new(AppState::default()));
            let seen_workspace_id = blank_state.read().active_workspace_id;
            let tx = Self::spawn_message_handler(logger.clone(), blank_state.clone());

            Self {
                tx: tx.clone(),
                state: blank_state,
                first_frame: true,
                window_size: egui::vec2(800.0, 600.0),
                logger: logger.clone(),
                seen_workspace_id,
            }
        };
        let pers_logger = logger.new(o!("category" => "persistence"));
        if let Some(storage) = cc.storage {
            if let Some(persistent) =
                eframe::get_value::<DiagramIDEPersistent>(storage, eframe::APP_KEY)
            {
                info!(pers_logger, "Load happening");
                let mut prev_state = DiagramIDE::from(persistent);
                let tx = Self::spawn_message_handler(
                    prev_state.logger.clone(),
                    prev_state.state.clone(),
                );
                prev_state.tx = tx.clone();
                //let _ = tx.try_send(Msg::ReloadSvgs(cc.egui_ctx.clone()));
                prev_state
            } else {
                info!(pers_logger, "Prev state not found");
                start_def()
            }
        } else {
            info!(pers_logger, "Storage not found");
            start_def()
        }
    }
    pub fn spawn_message_handler(
        logger: Logger,
        state: Arc<RwLock<AppState>>,
    ) -> mpsc::Sender<Msg> {
        debug!(logger, "Spawning logger");
        let (tx, rx) = mpsc::channel::<Msg>(100);
        tokio::spawn(message_handler::handle(rx, logger, state.clone()));
        tx
    }
    pub fn ui(&mut self, ctx: &egui::Context) {
        if self.first_frame {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(self.window_size));
            let selected = self.state.read().active_theme.clone();
            let selected = theme::initialize(&selected, ctx);
            self.state.write().active_theme = selected;
            self.seen_workspace_id = self.state.read().active_workspace_id;
            let _ = self.tx.try_send(Msg::RefreshWorkspace(ctx.clone()));
            self.first_frame = false;
        }
        // Detect a workspace switch / delete / import and refresh SVG
        // textures for the newly-promoted live windows.
        {
            let active = self.state.read().active_workspace_id;
            if active != self.seen_workspace_id {
                self.seen_workspace_id = active;
                let _ = self.tx.try_send(Msg::RefreshWorkspace(ctx.clone()));
            }
        }
        //ctx.options_mut(|opt| opt.zoom_factor = 0.75);
        let state = self.state.clone();
        let tx_clone = self.tx.clone();
        #[cfg(target_os = "macos")]
        menubar::titlebar(ctx);
        egui::TopBottomPanel::top("top_panel").show(ctx, menubar::widget(state, tx_clone));

        egui::CentralPanel::default().show(ctx, |ui| {
            let heading = self.state.read().active_workspace_name.clone();
            ui.heading(format!("Workspace: {heading}"));
        });

        {
            let state = self.state.clone();
            let tx_clone = self.tx.clone();
            if let Some(modal) = state.read().modals.front() {
                modal.write().show(ctx, tx_clone);
            }
        }

        {
            let background = self.state.read().diagram_background;
            let mut state = self.state.write();

            // SVG windows whose owner editor has rendering disabled should
            // not be shown. The pikchr content is still computed and remains
            // available for inclusion by other editors; we simply hide the
            // render window itself.
            let hidden_renders: std::collections::HashSet<egui::Id> = state
                .windows
                .values()
                .filter_map(|w| {
                    let mini = w.as_mini_window()?;
                    if !mini.render_enabled() {
                        w.as_target().map(|t| t.get_target())
                    } else {
                        None
                    }
                })
                .collect();

            for window in state.windows.values_mut() {
                let skip = window
                    .as_mini_window()
                    .is_some_and(|m| hidden_renders.contains(&m.get_id()));
                if skip {
                    continue;
                }
                if let Some(mini) = window.as_mini_window_mut() {
                    mini.show(ctx, self.tx.clone(), background);
                }
            }
        }

        if self.state.read().window_states.log {
            egui::Window::new("Log")
                .resizable(true)
                .default_size((200.0, 200.0))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false]) // Key: Stop it from shrinking to fit content!
                        .stick_to_bottom(true) // Optional: Auto-scroll to new entries
                        .show(ui, |ui| {
                            for entry in &self.state.read().log {
                                ui.label(entry);
                            }
                        });
                });
        }

        if self.state.read().window_states.debug {
            egui::Window::new("FPS").show(ctx, |ui| {
                ctx.inspection_ui(ui);
            });
        }
        egui::Area::new(egui::Id::new("bottom_right_status"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("Non-mandated use only. Contact for commercial license.")
                        .weak(),
                );
            });
    }
}

impl eframe::App for DiagramIDE {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(feature = "profile")]
        let _span = {
            tracing::info!(tracy.frame_mark = true);
            tracing::info_span!("ui_update").entered()
        };

        self.window_size = ctx.content_rect().size();
        self.ui(ctx);
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        info!(slog_scope::logger(), "Saving!"; "category" => "persistence");
        let persistent = DiagramIDEPersistent::from(self.clone());
        eframe::set_value(storage, eframe::APP_KEY, &persistent);
        storage.flush();
    }
}

pub const SPACE_MONO_BYTES: &[u8] = include_bytes!("../assets/fonts/SpaceMono-Regular.ttf");
pub const SPACE_MONO_NAME: &str = "Space Mono"; // Must match the internal TTF Name
pub const SPACE_MONO_BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/SpaceMono-Bold.ttf");
pub const NOTO_SANS_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
pub const NOTO_SANS_SYMBOLS2_BYTES: &[u8] =
    include_bytes!("../assets/fonts/NotoSansSymbols2-Regular.ttf");

/// Register the SpaceMono (regular + bold) font families used by the Grammar
/// help window for true bold weight. Extends the default egui FontDefinitions
/// (the theme layer only customizes style/visuals, not fonts) so other UI is
/// untouched. Safe to call on a freshly-created context.
pub fn install_help_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "space-mono".into(),
        std::sync::Arc::new(egui::FontData::from_static(SPACE_MONO_BYTES)),
    );
    fonts.font_data.insert(
        "space-mono-bold".into(),
        std::sync::Arc::new(egui::FontData::from_static(SPACE_MONO_BOLD_BYTES)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Name("SpaceMono".into()))
        .or_default()
        .push("space-mono".into());
    fonts
        .families
        .entry(egui::FontFamily::Name("SpaceMonoBold".into()))
        .or_default()
        .push("space-mono-bold".into());
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::RwLock;

    use crate::{
        DiagramIDE, Msg,
        mini_window::{HasName, RawContent, RenderToggle, Window, WindowType},
        pikchr_editor::PikchrEditor,
        plain_text_editor::PlainTextEditor,
        state::AppState,
    };

    #[test]
    fn plain_text_is_only_available_as_raw_content() {
        let plain_id = crate::egui::Id::new("plain");
        let pikchr_id = crate::egui::Id::new("pikchr");
        let svg_id = crate::egui::Id::new("svg");
        let mut plain = PlainTextEditor::new(plain_id);
        plain.set_name("REF".into());
        plain.set_raw_content("embedded text".into());

        let mut state = AppState::default();
        state
            .windows
            .insert(plain_id, Window::PlainTextEditor(plain));
        state.windows.insert(
            pikchr_id,
            Window::PikchrEditor(PikchrEditor::new(pikchr_id, svg_id)),
        );

        assert_eq!(
            crate::replace_content(&mut state, pikchr_id, "before !!REF!! after").unwrap(),
            "before embedded text after"
        );
        assert_eq!(
            crate::replace_generated_content(
                &mut state,
                pikchr_id,
                "$$REF$$",
                crate::OutputType::Pikchr,
            )
            .unwrap(),
            "$$REF$$"
        );
        assert!(
            state
                .windows
                .get(&plain_id)
                .and_then(Window::as_generated_content)
                .is_none()
        );
        assert!(state.editor_deps[&plain_id].contains(&pikchr_id));
    }

    #[test]
    fn generated_references_preserve_output_language_rules() {
        let source_id = crate::egui::Id::new("source");
        let target_id = crate::egui::Id::new("target");
        let mut source = PikchrEditor::new(source_id, crate::egui::Id::new("source-svg"));
        let mut target = PikchrEditor::new(target_id, crate::egui::Id::new("target-svg"));
        source.set_name("SOURCE".into());
        source.set_raw_content("box\ncircle".into());

        let mut state = AppState::default();
        state
            .windows
            .insert(source_id, Window::PikchrEditor(source.clone()));
        state
            .windows
            .insert(target_id, Window::PikchrEditor(target.clone()));

        assert_eq!(
            crate::replace_generated_content(
                &mut state,
                target_id,
                "$$SOURCE$$",
                crate::OutputType::Pikchr,
            )
            .unwrap(),
            "box;circle;"
        );

        source.set_output_type(crate::OutputType::Svgbob);
        source.set_raw_content("+---+\n| A |\n+---+".into());
        target.set_output_type(crate::OutputType::Svgbob);
        state
            .windows
            .insert(source_id, Window::PikchrEditor(source));
        state
            .windows
            .insert(target_id, Window::PikchrEditor(target));

        assert_eq!(
            crate::replace_generated_content(
                &mut state,
                target_id,
                "before\n$$SOURCE$$\nafter",
                crate::OutputType::Svgbob,
            )
            .unwrap(),
            "before\n+---+\n| A |\n+---+\nafter"
        );

        let error = crate::replace_generated_content(
            &mut state,
            target_id,
            "$$SOURCE$$",
            crate::OutputType::Pikchr,
        )
        .unwrap_err();
        assert!(error.contains("Svgbob"));
        assert!(error.contains("Pikchr"));
    }

    #[test]
    fn svgbob_generated_reference_can_overlay_another_editor_column_wise() {
        let edit_id = crate::egui::Id::new("edit");
        let overlay_id = crate::egui::Id::new("overlay");
        let target_id = crate::egui::Id::new("target");
        let mut edit = PikchrEditor::new(edit_id, crate::egui::Id::new("edit-svg"));
        let mut overlay = PikchrEditor::new(overlay_id, crate::egui::Id::new("overlay-svg"));
        let mut target = PikchrEditor::new(target_id, crate::egui::Id::new("target-svg"));

        edit.set_name("EDIT".into());
        edit.set_output_type(crate::OutputType::Svgbob);
        edit.set_raw_content("9 = 3320\nAAA  9\nAAA\nAAA".into());
        overlay.set_name("3320".into());
        overlay.set_output_type(crate::OutputType::Svgbob);
        overlay.set_raw_content("ZZ\nZZ".into());
        target.set_output_type(crate::OutputType::Svgbob);

        let mut state = AppState::default();
        state.windows.insert(edit_id, Window::PikchrEditor(edit));
        state
            .windows
            .insert(overlay_id, Window::PikchrEditor(overlay));
        state
            .windows
            .insert(target_id, Window::PikchrEditor(target));

        assert_eq!(
            crate::replace_generated_content(
                &mut state,
                edit_id,
                "9 = 3320\nAAA  9\nAAA\nAAA",
                crate::OutputType::Svgbob,
            )
            .unwrap(),
            "AAA  ZZ\nAAA  ZZ\nAAA"
        );
        assert_eq!(
            crate::replace_generated_content(
                &mut state,
                target_id,
                "$$EDIT$$",
                crate::OutputType::Svgbob,
            )
            .unwrap(),
            "AAA  ZZ\nAAA  ZZ\nAAA"
        );
        assert!(state.editor_deps[&edit_id].contains(&target_id));
        assert!(state.editor_deps[&overlay_id].contains(&target_id));

        crate::clean_old_deps(&mut state);
        assert!(state.editor_deps[&overlay_id].contains(&edit_id));
    }

    #[tokio::test]
    async fn creating_plain_text_does_not_create_an_svg_window() {
        let state = Arc::new(RwLock::new(AppState::default()));
        let tx = DiagramIDE::spawn_message_handler(crate::logger::init_logger(), state.clone());

        tx.send(Msg::NewWindow(
            eframe::egui::Context::default(),
            WindowType::PlainTextEditor,
        ))
        .await
        .unwrap();
        while state.read().windows.is_empty() {
            tokio::task::yield_now().await;
        }

        let state = state.read();
        assert_eq!(state.windows.len(), 1);
        assert!(matches!(
            state.windows.values().next(),
            Some(Window::PlainTextEditor(_))
        ));
    }
}
