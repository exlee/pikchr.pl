// This file is part of pikchr.pl.
//
// pikchr.pl is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// pikchr.pl is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with pikchr.pl. If not, see <https://www.gnu.org/licenses/>.

use std::{fmt::Display, path::PathBuf, time::Duration};

use iced::{
    Alignment, Color, Element, Length, Task, Theme,
    widget::{
        button, column, container, pane_grid, pick_list, row, space, stack, svg, text::Shaping,
        text_editor::Content,
    }, window::icon,
};
use pikchr_pro::{
    pikchr::{self, PikchrCode},
    prolog::{engine::trealla::EngineAsync as PrologEngine},
};
use thiserror::Error;
use tokio::sync::watch;

mod editor_actions_handler;
mod editor_state;
mod file_watcher;
mod keybindings;
mod messages;
mod string_ext;
mod undo;
mod constants;
mod prolog_modules;

use editor_state::Editor;
use messages::Message;

use crate::{prolog_modules::PrologModules, string_ext::StringExt, undo::UndoStack};

const DEBOUNCE_MS: u64 = 100;

pub fn main() -> iced::Result {
    let window_settings = iced::window::Settings {
        icon: Some(load_icon()),
        ..Default::default()
    };
    dbg!(&window_settings);
    PrologEngine::init();
    iced::application(Editor::new, Editor::update, Editor::view)
        .title(Editor::set_title)
        .font(SPACE_MONO_BYTES)
        .subscription(Editor::subscriptions)
        .window(window_settings)
        .run()

}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperatingMode {
    PikchrMode,
    PrologMode,
}

enum PaneContent {
    Editor,
    Preview,
}

impl Display for OperatingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperatingMode::PikchrMode => write!(f, "Pikchr Mode"),
            OperatingMode::PrologMode => write!(f, "Prolog Mode"),
        }
    }
}

impl Editor {
    fn new() -> (Self, Task<Message>) {
        (Editor::default(), Task::done(Message::RunLogic))
    }
    fn set_title(&self) -> String {
        let file: String = self
            .current_file
            .clone()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(String::from("Unnamed"));
        let dirty = if self.dirty { "*" } else { "" };
        format!("Pikchr.pl - {}{}", file, dirty)
    }
    fn reset_editor(&mut self) -> Task<Message> {
        *self = Self::default();
        Task::done(Message::RunLogic)
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        use Message::*;
        // Message Matching Logic
        match message {
            LoadFileSelected(Some(path_buf)) => {
                self.current_file = Some(path_buf);
                let path_ref: &PathBuf = self.current_file.as_ref().unwrap();

                if let Ok(file_as_string) = std::fs::read_to_string(path_ref) {
                    self.content = Content::with_text(&file_as_string);
                    self.undo_stack = UndoStack::new(self.content.clone());
                    self.dirty = false;
                    Task::done(Message::RunLogic)
                } else {
                    Task::done(Message::ShowError(ApplicationError::FileLoadFailure(
                        path_ref.clone(),
                    )))
                }
            },
            LoadFileSelected(_) => Task::none(),
            LoadRequested => {
                let mode = self.operating_mode;
                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_title("Load File")
                            .add_filter_according_to_mode(mode)
                            .pick_file()
                            .await
                            .map(|handle| handle.path().to_path_buf())
                    },
                    Message::LoadFileSelected,
                )
            },
            ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
                Task::none()
            },
            NewRequested => self.reset_editor(),
            Edit(action @ iced::widget::text_editor::Action::Edit(_)) => {
                self.undo_stack.push(&self.content);
                self.dirty = true;
                self.content.perform(action);
                Task::done(RunLogic)
            },
            Edit(action) => {
                self.content.perform(action);
                Task::none()
            },
            EditBatch(actions) => Task::batch(
                actions
                    .into_iter()
                    .map(Message::Edit)
                    .map(Task::done)
                    .collect::<Vec<_>>(),
            ),
            PikchrFinished(result) => {
                self.is_compiling = false;
                match result {
                    // Stale result
                    None => Task::none(),
                    Some(Ok(string)) => {
                        let bytes = string.into_bytes();
                        let handle = svg::Handle::from_memory(bytes);
                        self.last_successful = true;
                        self.last_error.set(String::new());
                        self.svg_handle = Some(handle);
                        Task::none()
                    },
                    Some(Err(e)) => {
                        self.last_successful = false;
                        Task::done(Message::ShowError(e))
                    },
                }
            },
            PrologFinished(result) => {
                match result {
                    Some(Ok(input)) => {
                        self.pikchr_code = Some(input.clone());
                        Task::batch(vec![
                            //Task::done(Message::ShowPikchr(input.clone())),
                            Task::done(Message::RunPikchr(input)),
                        ])
                    },
                    Some(Err(err)) => Task::done(Message::ShowError(err)),
                    None => Task::none(),
                }
            },
            RadioSelected(operating_mode) => {
                self.operating_mode = operating_mode;
                Task::done(Message::RunLogic)
            },
            RefreshTick => {
                self.last_error.commit();
                Task::none()
            },
            RunLogic => {
                let input = self.content.text();
                match self.operating_mode {
                    OperatingMode::PikchrMode => Task::done(Message::RunPikchr(input.into())),
                    OperatingMode::PrologMode => Task::done(Message::RunProlog(input)),
                }
            },
            RunPikchr(input) => {
                let input_rx = self.pikchr_input_rx.clone();
                let _ = self.pikchr_input_tx.send(input);
                Task::perform(
                    render_pikchr(self.last_successful, input_rx),
                    Message::PikchrFinished,
                )
            },
            RunProlog(input) => {
                let input_rx = self.prolog_input_rx.clone();
                let _ = self.prolog_input_tx.send(input);
                let modules = self.modules.clone();

                Task::perform(render_diagram(self.last_successful, input_rx, modules), Message::PrologFinished)
            },
            SaveFileSelected(path_buf_opt) => {
                self.current_file = path_buf_opt.clone();
                if let Some(path_buf) = path_buf_opt {
                    match std::fs::write(path_buf, self.content.text()) {
                        Ok(_) => (),
                        Err(_) => {
                                return Task::done(ShowError(ApplicationError::SaveFailure))
                        },
                    };
                }
                Task::done(Message::SaveFinished)
            },
            SaveFinished => {
                self.dirty = false;
                Task::none()
            },
            SaveRequested => {
                if self.current_file.is_none() {
                    Task::done(SaveAsRequested)
                } else {
                    Task::done(SaveFileSelected(self.current_file.clone()))
                }
            },
            SaveAsRequested => {
                let mode = self.operating_mode;
                let current_file_opt = self
                    .current_file
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|n| n.to_string_lossy().into_owned());

                Task::perform(
                    async move {
                        let dialog = rfd::AsyncFileDialog::new()
                            .set_title("Save File")
                            .add_filter_according_to_mode(mode);

                        if let Some(basename) = current_file_opt {
                            dialog.set_file_name(basename)
                        } else {
                            dialog
                        }
                        .save_file()
                        .await
                        .map(|handle| handle.path().to_path_buf())
                    },
                    Message::SaveFileSelected,
                )
            },
            ShowError(error) => {
                match error {
                    ApplicationError::PikchrPrologError(render_error) => {
                        self.last_error.set(format!("{:?}", render_error));
                    },
                    ApplicationError::PikchrError(render_error) => {
                        let trimmed = render_error.trim_last_chars(500).trim_last_lines(5);

                        self.last_error.set(trimmed);
                    },
                    ApplicationError::PikchrEmpty => (),
                    ApplicationError::Unknown => self.last_error.set(String::from("Unknown error")),
                    ApplicationError::FileLoadFailure(path_buf) => {
                        self.last_error.set(format!(
                            "Failed to load file: {}",
                            path_buf.to_string_lossy()
                        ));
                    },
                    ApplicationError::SaveFailure => self.last_error.set(error.to_string()),
                }
                Task::none()
            },
            ToggleDebugOverlay => {
                self.show_debug = !self.show_debug;
                Task::none()
            },
            ToggleFileWatch => {
                self.file_watch_mode = !self.file_watch_mode;
                Task::none()
            },
            Undo => {
                self.undo_stack.undo_into(&mut self.content);
                Task::none()
            },
            Redo => {
                self.undo_stack.redo_into(&mut self.content);
                Task::none()
            },
            PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Task::none()
            },
            EditorAction(msg) => editor_actions_handler::handle(self, msg),
            LoadedFileChanged => Task::done(Message::LoadFileSelected(self.current_file.clone())),
        }
    }
    fn view(&self) -> Element<'_, Message> {
        let panes = pane_grid(&self.panes, |_pane, content, _is_focused| {
            let content_widget = match content {
                PaneContent::Editor => self.input_pane(),
                PaneContent::Preview => self.preview_pane(),
            };
            pane_grid::Content::new(content_widget)
        })
        .on_resize(10, Message::PaneResized)
        .spacing(10)
        .width(Length::Fill)
        .height(Length::Fill);

        let info_box = container(
            iced::widget::text(self.last_error.get())
                .font(iced::font::Font::MONOSPACE)
                .shaping(Shaping::Basic)
                .width(Length::Fill)
                .size(10),
        )
        .height(Length::Fixed(75.0))
        .padding(10);

        let main_pane = if self.file_watch_mode {
            self.preview_pane()
        } else {
            panes.into()
        };

        let content = if self.show_debug {
            stack![main_pane, self.debug_overlay()]
        } else {
            stack![main_pane]
        };
        column![self.menu_bar(), content, row![info_box]]
            .spacing(10)
            .padding(10)
            .into()
    }

    fn subscriptions(&self) -> iced::Subscription<Message> {
        let mut subscriptions = vec![
            iced::time::every(Duration::from_millis(500)).map(|_| Message::RefreshTick),
            keybindings::listen(),
        ];
        if let (true, Some(file)) = (self.file_watch_mode, &self.current_file) {
            subscriptions.push(file_watcher::file_watcher(file))
        } 
        iced::Subscription::batch(subscriptions)
    }

    fn input_pane(&self) -> Element<'_, Message> {
        iced::widget::text_editor(&self.content)
            .on_action(Message::Edit)
            .key_binding(keybindings::handle_action)
            .height(Length::Fill)
            .size(12)
            .font(iced::font::Font::MONOSPACE)
            .into()
    }
    fn preview_pane(&self) -> Element<'_, Message> {
        if let Some(handle) = &self.svg_handle {
            // FIX: Clone the handle here.
            // The `svg` widget consumes the handle, it does not take a reference.
            container(svg(handle.clone()).width(Length::Fill).height(Length::Fill))
                .style(|_theme| container::Style {
                    // Force background to white
                    background: Some(Color::WHITE.into()),
                    // Keep other defaults (text color, border, etc.)
                    ..container::Style::default()
                })
                .padding(10)
                .into()
        } else {
            container(iced::widget::text("").font(iced::font::Font::MONOSPACE))
                .padding(10)
                .into()
        }
    }
    fn debug_overlay(&self) -> Element<'_, Message> {
        let code = self
            .pikchr_code
            .clone()
            .map(|i| i.clone().into_inner())
            .unwrap_or_default();

        let inner_bg = |t: &Theme| t.palette().background;
        let overlay_bg = |t: &Theme| t.palette().background.scale_alpha(0.7);
        let border_color = |t: &Theme| t.palette().background.inverse();

        let inner_container = container(iced::widget::scrollable(
            iced::widget::text(code)
                .width(Length::Fill)
                .size(12)
                .font(iced::font::Font::MONOSPACE),
        ))
        .style(move |theme: &Theme| container::Style {
            background: Some(iced::Background::Color(inner_bg(theme))),
            border: iced::Border {
                color: border_color(theme),
                width: 2.0,
                ..Default::default()
            },
            ..Default::default()
        })
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill);
        //.center_x(Length::Fill)
        //.center_y(Length::Fill);

        let outer_container = container(inner_container)
            .style(move |theme: &Theme| container::Style {
                background: Some(iced::Background::Color(overlay_bg(theme))),
                ..Default::default()
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(100);

        outer_container.into()
    }
    fn menu_bar(&self) -> Element<'_, Message> {
        let op_modes = [OperatingMode::PrologMode, OperatingMode::PikchrMode];
        let operating_mode_list =
            pick_list(op_modes, Some(self.operating_mode), Message::RadioSelected);

        let button_new = button("New").on_press(Message::NewRequested);

        let button_save = if self.modifiers.command() {
            button("Save As").on_press(Message::SaveAsRequested)
        } else {
            button("Save").on_press(Message::SaveRequested)
        };
        let button_load = button("Load").on_press(Message::LoadRequested);

        let toggle_debug = iced::widget::toggler(self.show_debug)
            .label("Debug Overlay (F2)")
            .on_toggle(|_| Message::ToggleDebugOverlay);

        let toggle_watch: Element<'_, Message> = if self.current_file.is_some() {
            iced::widget::toggler(self.file_watch_mode)
                .label("File Watch Mode")
                .on_toggle(|_| Message::ToggleFileWatch)
                .into()
        } else {
            space::horizontal().into()
        };

        row![
            button_new,
            button_save,
            button_load,
            space::horizontal(),
            toggle_watch,
            toggle_debug,
            operating_mode_list,
        ]
        .align_y(Alignment::Center)
        .spacing(10)
        .into()
    }
}

#[derive(Error, Debug, Clone)]
pub enum ApplicationError {
    #[error("SAVE UNSUCCESSFUL")]
    SaveFailure,
    #[error("PikchrProlog error: {0}")]
    PikchrPrologError(#[from] pikchr_pro::prolog::RenderError),
    #[error("Pikchr render error: {0}")]
    PikchrError(String),
    #[error("Pikchr render is empty")]
    PikchrEmpty,
    #[error("Failed to load file {0:?}")]
    FileLoadFailure(PathBuf),
    #[error("unknown error")]
    Unknown,
}

async fn render_diagram(last_successful: bool, mut input_rx: watch::Receiver<String>, prolog_modules: PrologModules) -> Option<Result<PikchrCode, ApplicationError>> {
    let input = input_rx.borrow_and_update().clone();
    if last_successful {
        tokio::time::sleep(std::time::Duration::from_millis(DEBOUNCE_MS)).await;
    }
    if input_rx.has_changed().unwrap_or(false) {
        return None;
    }
    let result = PrologEngine::process_diagram(vec![input,prolog_modules.to_merged_string()])
        .await
        .map_err(|s| s.into());

    Some(result)
}

async fn render_pikchr(
    last_successful: bool,
    mut input_rx: watch::Receiver<PikchrCode>,
) -> Option<Result<String, ApplicationError>> {
    let input = input_rx.borrow_and_update().clone();
    if last_successful {
        tokio::time::sleep(std::time::Duration::from_millis(DEBOUNCE_MS)).await;
    }

    if input_rx.has_changed().unwrap_or(false) {
        return None;
    }

    let result =
        tokio::task::spawn_blocking(move || match pikchr::render(&input.into_inner(), None, 1) {
            Ok(pik) if pik.is_error() => Err(ApplicationError::PikchrError(pik.into_string())),
            Ok(pik) if pik.is_empty() => Err(ApplicationError::PikchrEmpty),
            Ok(pik) => Ok(inject_svg_style(pik.into_string())),
            Err(e) => Err(ApplicationError::PikchrError(e)),
        })
        .await
        .unwrap();
    Some(result)
}

fn inject_svg_style(input: String) -> String {
    let mut input = input.clone();
    if let Some(idx) = input.find(">") {
        let style = format!(
            "<style>text,path {{ font-family: '{}'; }}</style>",
            SPACE_MONO_NAME
        );
        input.insert_str(idx + 1, &style);
    }
    input
}

const ICON: &[u8;15574] = include_bytes!("../../../assets/icon_1024.png");
fn load_icon() -> iced::window::Icon {
    icon::from_file_data(
        ICON, Some(image::ImageFormat::Png)
    ).expect("Can't load app icon")
}

trait AsyncFileDialogExt {
    fn add_filter_according_to_mode(self, mode: OperatingMode) -> Self;
}
impl AsyncFileDialogExt for rfd::AsyncFileDialog {
    fn add_filter_according_to_mode(self, mode: OperatingMode) -> Self {
        match mode {
            OperatingMode::PikchrMode => self.add_filter("pikchr", &["pik"]),
            OperatingMode::PrologMode => self.add_filter("prolog", &["pl"]),
        }
    }
}
const SPACE_MONO_BYTES: &[u8] = include_bytes!("../fonts/SpaceMono-Regular.ttf");
const SPACE_MONO_NAME: &str = "Space Mono"; // Must match the internal TTF Name
