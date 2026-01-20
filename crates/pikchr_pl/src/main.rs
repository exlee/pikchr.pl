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

use std::{
    fmt::Display,
    path::PathBuf,
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};

use anyhow::{Result, anyhow};
use iced::{
    Alignment,
    Color,
    Element,
    Length,
    Task,
    Theme,
    keyboard::Modifiers,
    widget::{
        button,
        column,
        container,
        pick_list,
        radio,
        row,
        space,
        stack,
        svg,
        text::Shaping,
        text_editor::{Content, Motion},
    },
};
use pikchr_pro::{
    pikchr::{self, PikchrCode},
    prolog,
};
use thiserror::Error;
use tokio::sync::{RwLock, watch};

mod editor_state;
mod keybindings;
mod messages;

use editor_state::Editor;
use messages::Message;

pub fn main() -> iced::Result {
    prolog::asynch::init();
    iced::application(Editor::new, Editor::update, Editor::view)
        .title(Editor::set_title)
        .font(SPACE_MONO_BYTES)
        .subscription(Editor::subscriptions)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperatingMode {
    PikchrMode,
    PrologMode,
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
            Edit(action) => {
                self.content.perform(action);
                Task::done(Message::RunLogic)
            },
            Ignore => Task::none(),
            LoadFileSelected(Some(path_buf)) => {
                self.current_file = Some(path_buf.clone());
                if let Ok(file_as_string) = std::fs::read_to_string(&path_buf) {
                    self.content = Content::with_text(&file_as_string);
                    Task::done(Message::RunLogic)
                } else {
                    Task::done(Message::ShowError(ApplicationError::FileLoadFailure(
                        path_buf,
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
            PerformAction(action) => {
                self.content.perform(action);
                Task::none()
            },
            PerformActions(run_actions, actions) => {
                for action in actions {
                    self.content.perform(action);
                }
                if run_actions {
                    let _ = self.input_tx.send(self.content.text().into());
                    Task::done(Message::RunLogic)
                } else {
                    Task::none()
                }
            },
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
                    Ok(input) => {
                        self.pikchr_code = Some(input.clone());
                        Task::batch(vec![
                            //Task::done(Message::ShowPikchr(input.clone())),
                            Task::done(Message::RunPikchr(input)),
                        ])
                    },
                    Err(err) => Task::done(Message::ShowError(err)),
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
                let input_rx = self.input_rx.clone();
                let _ = self.input_tx.send(input);
                Task::perform(
                    render_pikchr(self.last_successful, input_rx),
                    Message::PikchrFinished,
                )
            },
            RunProlog(input) => Task::perform(render_diagram(input), Message::PrologFinished),
            SaveFileSelected(path_buf_opt) => {
                self.current_file = path_buf_opt.clone();
                if let Some(path_buf) = path_buf_opt {
                    std::fs::write(path_buf, self.content.text());
                }
                Task::done(Message::SaveFinished)
            },
            SaveFinished => {
                self.dirty = false;
                Task::none()
            },
            SaveRequested => {
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
                        const MAX_ERROR_LENGTH: usize = 500;

                        let err_len = render_error.len();
                        if err_len > MAX_ERROR_LENGTH {
                            self.last_error.set(String::from(
                                render_error
                                    .clone()
                                    .split_at_checked(err_len - MAX_ERROR_LENGTH)
                                    .unwrap_or(("", &render_error))
                                    .1,
                            ))
                        } else {
                            self.last_error.set(render_error);
                        }
                    },
                    ApplicationError::PikchrEmpty => (),
                    ApplicationError::Unknown => self.last_error.set(String::from("Unknown error")),
                    ApplicationError::FileLoadFailure(path_buf) => {
                        self.last_error.set(format!(
                            "Failed to load file: {}",
                            path_buf.to_string_lossy()
                        ));
                    },
                }
                Task::none()
            },
            ShowPikchr(pikchr_code) => Task::none(),
            ToggleDebugOverlay => {
                self.show_debug = !self.show_debug;
                Task::none()
            },
        }
    }
    fn view(&self) -> Element<Message> {
        // Editor Pane
        let input_pane = iced::widget::text_editor(&self.content)
            .on_action(Message::Edit)
            .key_binding(keybindings::handle_action)
            .height(Length::Fill)
            .size(12)
            .font(iced::font::Font::MONOSPACE);

        let preview_pane: Element<Message> = if let Some(handle) = &self.svg_handle {
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
        };

        let info_box = container(
            iced::widget::text(self.last_error.get())
                .font(iced::font::Font::MONOSPACE)
                .shaping(Shaping::Basic)
                .width(Length::Fill)
                .size(10),
        )
        .height(Length::Fixed(50.0))
        .padding(10);

        let main_content = row![
            column![input_pane]
                .width(Length::FillPortion(2))
                .spacing(10),
            column![
                container(preview_pane)
                    .style(container::bordered_box)
                    .width(Length::Fill)
                    .height(Length::Fill)
            ]
            .width(Length::FillPortion(3))
            .spacing(10)
        ]
        .spacing(10);
        let content = if self.show_debug {
            stack![main_content, self.debug_overlay()]
        } else {
            stack![main_content]
        };
        column![self.menu_bar(), content, row![info_box]]
            .spacing(10)
            .padding(10)
            .into()
    }

    fn subscriptions(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            iced::time::every(Duration::from_millis(500)).map(|_| Message::RefreshTick),
            keybindings::listen(),
        ])
    }
    fn debug_overlay<'a>(&self) -> Element<'a, Message> {
        let code = self
            .pikchr_code
            .clone()
            .map(|i| i.clone().into_inner())
            .unwrap_or_default();

        let inner_bg = |t: &Theme| t.palette().background;
        let overlay_bg = |t: &Theme| t.palette().background.scale_alpha(0.7);
        let border_color = |t: &Theme| t.palette().background.inverse();

        let inner_container = container(iced::widget::scrollable(
            iced::widget::text(code).width(Length::Fill),
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
    fn menu_bar<'a>(&self) -> Element<'a, Message> {
        let op_modes = [OperatingMode::PrologMode, OperatingMode::PikchrMode];
        let operating_mode_list =
            pick_list(op_modes, Some(self.operating_mode), Message::RadioSelected);

        let button_new = button("New").on_press(Message::NewRequested);
        let button_save = button("Save").on_press(Message::SaveRequested);
        let button_load = button("Load").on_press(Message::LoadRequested);

        let toggle_debug = iced::widget::toggler(self.show_debug)
            .label("Debug Overlay (F2)")
            .on_toggle(|_| Message::ToggleDebugOverlay);

        row![
            button_new,
            button_save,
            button_load,
            space::horizontal(),
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

const INIT: &str = include_str!("../native/prolog/init.pl");
async fn render_diagram(input: String) -> Result<PikchrCode, ApplicationError> {
    prolog::asynch::process_diagram(vec![String::from(INIT), input])
        .await
        .map_err(|s| s.into())
}

async fn render_pikchr(
    last_successful: bool,
    mut input_rx: watch::Receiver<PikchrCode>,
) -> Option<Result<String, ApplicationError>> {
    let input = input_rx.borrow_and_update().clone();
    if last_successful {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
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
