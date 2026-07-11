use std::{
    env,
    io::Write as _,
    path::{Path, PathBuf},
};

use eframe::egui::{self, Context, Layout, Margin, Vec2};
use tokio::sync::mpsc::Sender;

use crate::{ExportType, Msg, response_ext::ResponseExt as _, state::WorkspaceId};

pub trait Modal: Sync + Send + std::fmt::Debug {
    fn show(&mut self, ctx: &Context, tx: Sender<Msg>);
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ExportModal {
    svg_id: egui::Id,
    export_type: ExportType,
    destination: String,
    #[allow(unused)]
    file_name: String,
}
impl ExportModal {
    pub fn new(svg_id: egui::Id, file_name: String, export_type: ExportType) -> Self {
        Self {
            svg_id,
            destination: Self::build_destination(&file_name, &export_type),
            export_type,
            file_name,
        }
    }
    fn build_destination(file: &str, export_type: &ExportType) -> String {
        let extension = match export_type {
            ExportType::Svg => "svg",
            ExportType::Png => "png",
            ExportType::PngTransparent => "png",
            ExportType::Source(output_type) => output_type.source_extension(),
        };
        let file_cleaned: String = file
            .chars()
            .filter(|&c| c.is_alphanumeric() || c == ' ')
            .collect();
        let file_cleaned = file_cleaned
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("_");
        let joined_pb = env::current_dir()
            .unwrap_or(PathBuf::from("."))
            .join(file_cleaned);
        let joined = joined_pb.to_string_lossy();
        format!("{}.{}", joined, extension)
    }
}

impl Modal for ExportModal {
    fn show(&mut self, ctx: &Context, tx: Sender<Msg>) {
        egui::Modal::new(egui::Id::new("egui_modal"))
            //.backdrop_color(Color32::BLACK)
            .show(ctx, |ui| {
                let title = match self.export_type {
                    ExportType::Svg => "Export as SVG".to_owned(),
                    ExportType::Png => "Export as PNG".to_owned(),
                    ExportType::PngTransparent => "Export as transparent PNG".to_owned(),
                    ExportType::Source(output_type) => {
                        format!("Export {} source", output_type.label())
                    },
                };
                ui.set_min_size(Vec2::from((400.0, 50.0)));
                ui.heading(title);
                ui.separator();

                ui.add_space(10.0);
                ui.add_sized(
                    (ui.available_width(), 30.0),
                    egui::TextEdit::singleline(&mut self.destination)
                        .margin(Margin::symmetric(4, 8)),
                );
                ui.add_space(10.0);

                ui.separator();
                ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                    if ui.button("Export").clicked() {
                        let _ = tx.try_send(Msg::Export(
                            self.svg_id,
                            self.destination.clone(),
                            self.export_type,
                            Box::new(ui.visuals().clone()),
                        ));
                    };
                    ui.add_space(10.0);

                    if ui.button("Close").clicked() {
                        let _ = tx.try_send(Msg::PopModal);
                    };
                });
            });
    }
}

#[derive(Debug)]
pub struct FileModalView<'a> {
    dialog_title: &'a str,
    action_name: &'a str,
    destination: &'a mut String,
}
pub trait FileModalTrait: Modal {
    fn on_action(&self, ctx: &Context, tx: Sender<Msg>) -> Result<(), Box<dyn std::error::Error>>;
    fn get_modal_view(&mut self) -> FileModalView<'_>;

    fn build_destination(extension: &str, base_name: &str) -> String {
        let file_cleaned: String = base_name
            .chars()
            .filter(|&c| c.is_alphanumeric() || c == ' ')
            .collect();
        let file_cleaned = file_cleaned
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("_");
        let joined_pb = env::current_dir()
            .unwrap_or(PathBuf::from("."))
            .join(file_cleaned);
        let joined = joined_pb.to_string_lossy();
        format!("{}.{}", joined, extension)
    }
}

impl<T> Modal for T
where
    T: FileModalTrait,
{
    fn show(&mut self, ctx: &Context, tx: Sender<Msg>) {
        egui::Modal::new(egui::Id::new("egui_modal")).show(ctx, |ui| {
            let view = self.get_modal_view();
            let title = view.dialog_title.to_string();
            let action_name = view.action_name.to_string();
            {
                let destination_ref = view.destination;

                ui.set_min_size(Vec2::from((400.0, 50.0)));
                ui.heading(title);
                ui.separator();

                ui.add_space(10.0);
                ui.add_sized(
                    (ui.available_width(), 30.0),
                    egui::TextEdit::singleline(destination_ref).margin(Margin::symmetric(4, 8)),
                );
            }
            ui.add_space(10.0);

            ui.separator();
            ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                if ui.button(action_name).clicked() {
                    #[allow(clippy::single_match)]
                    match self.on_action(ctx, tx.clone()) {
                        Ok(_) => {
                            let _ = tx.try_send(Msg::PopModal);
                        },
                        Err(err) => {
                            tracing::error!(error = %err, "modal action failed");
                        },
                    };
                };
                ui.add_space(10.0);

                if ui.button("Close").clicked() {
                    let _ = tx.try_send(Msg::PopModal);
                };
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_exports_use_output_specific_extensions() {
        let pikchr = ExportModal::build_destination(
            "Render - Example",
            &ExportType::Source(crate::OutputType::Pikchr),
        );
        let svgbob = ExportModal::build_destination(
            "Render - Example",
            &ExportType::Source(crate::OutputType::Svgbob),
        );

        assert!(pikchr.ends_with("Render_Example.pikchr"));
        assert!(svgbob.ends_with("Render_Example.txt"));
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct FileSaveModal {
    dialog_title: Option<String>,
    extension: String,
    base_name: String,
    #[serde(skip, default)]
    destination: String,
    payload: Box<[u8]>,
}
impl FileSaveModal {
    pub fn new(
        payload: Box<[u8]>,
        extension: &str,
        base_name: &str,
        dialog_title: Option<&str>,
    ) -> Self {
        Self {
            payload,
            extension: String::from(extension),
            base_name: String::from(base_name),
            dialog_title: dialog_title.map(String::from),
            destination: Self::build_destination(extension, base_name),
        }
    }
}
impl FileModalTrait for FileSaveModal {
    fn get_modal_view(&mut self) -> FileModalView<'_> {
        let dialog_title = self
            .dialog_title
            .get_or_insert(String::from("Save file..."));
        FileModalView {
            dialog_title,
            action_name: "Save",
            destination: &mut self.destination,
        }
    }
    fn on_action(
        &self,
        _ctx: &Context,
        _tx: Sender<Msg>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path: String = self.destination.clone();
        let path = Path::new(&path);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        };
        let mut file = std::fs::File::create_new(path)?;
        file.write_all(&self.payload)?;

        Ok(())
    }
}

pub struct FileOpenModal {
    dialog_title: String,
    extension: String,
    destination: String,
    action_fn: Box<ActionFn>,
}

impl std::fmt::Debug for FileOpenModal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileOpenModal")
            .field("dialog_title", &self.dialog_title)
            .field("extension", &self.extension)
            .field("destination", &self.destination)
            .field("action_fn", &"<ActionFn>")
            .finish()
    }
}
type ActionFn =
    dyn Fn(String, &Context, Sender<Msg>) -> Result<(), Box<dyn std::error::Error>> + Send + Sync;
impl FileOpenModal {
    pub fn new(dialog_title: &str, extension: &str, on_action: Box<ActionFn>) -> Self {
        let destination = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(String::from("/"));
        Self {
            action_fn: on_action,
            dialog_title: String::from(dialog_title),
            extension: String::from(extension),
            destination,
        }
    }
}
impl FileModalTrait for FileOpenModal {
    fn on_action(
        &self,
        context: &Context,
        tx: Sender<Msg>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let closure = &self.action_fn;
        let _ = closure(self.destination.clone(), context, tx);
        Ok(())
    }

    fn get_modal_view(&mut self) -> FileModalView<'_> {
        FileModalView {
            dialog_title: &self.dialog_title,
            action_name: "Load",
            destination: &mut self.destination,
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ConfirmationModal {
    confirmation_msg: Msg,
    question: String,
}

impl ConfirmationModal {
    pub fn new(confirmation_msg: Msg, question: &str) -> Self {
        Self {
            confirmation_msg,
            question: String::from(question),
        }
    }
}

impl Modal for ConfirmationModal {
    fn show(&mut self, ctx: &Context, tx: Sender<Msg>) {
        let confirmation_msg = self.confirmation_msg.clone();
        egui::Modal::new(egui::Id::new("egui_confirm")).show(ctx, |ui| {
            ui.set_min_size(Vec2::from((200.0, 100.0)));
            ui.set_max_size(Vec2::from((200.0, 200.00)));
            ui.heading("Confirm");
            ui.separator();
            ui.add_space(10.0);
            ui.label(&self.question);
            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Confirm").clicked() {
                    let _ = tx.try_send(confirmation_msg);
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        let _ = tx.try_send(Msg::PopModal);
                    };
                });
            });
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                let _ = tx.try_send(Msg::PopModal);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                let _ = tx.try_send(self.confirmation_msg.clone());
            }
        });
    }
}

#[derive(Debug)]
pub struct StringEditModal<'a> {
    variable: &'a mut String,
    name: &'static str,
    var_temp: String,
}
impl<'a> StringEditModal<'a> {
    pub fn new(name: &'static str, variable: &'a mut String) -> Self {
        let var_temp = variable.clone();
        Self {
            variable,
            name,
            var_temp,
        }
    }
}

impl<'a> Modal for StringEditModal<'a> {
    fn show(&mut self, ctx: &Context, tx: Sender<Msg>) {
        let mut heading = String::from("Edit ");
        heading.push_str(self.name);
        egui::Modal::new(egui::Id::new("egui_confirm")).show(ctx, |ui| {
            ui.set_min_size(Vec2::from((200.0, 100.0)));
            ui.set_max_size(Vec2::from((200.0, 200.00)));
            ui.heading(&heading);
            ui.separator();
            ui.add_space(4.0);
            ui.text_edit_singleline(&mut self.var_temp);
            ui.add_space(4.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    *self.variable = self.var_temp.clone();
                    let _ = tx.try_send(Msg::PopModal);
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        let _ = tx.try_send(Msg::PopModal);
                    };
                });
            });
        });
    }
}

#[derive(Debug)]
pub struct SaveToLibraryModal {
    editor_id: egui::Id,
    temp: String,
}

impl SaveToLibraryModal {
    pub fn new(editor_id: egui::Id, initial: &str) -> Self {
        Self {
            editor_id,
            temp: initial.into(),
        }
    }
}

impl Modal for SaveToLibraryModal {
    fn show(&mut self, ctx: &Context, tx: Sender<Msg>) {
        const MODAL_WIDTH: f32 = 320.0;
        const INPUT_HEIGHT: f32 = 30.0;

        let confirm_action = |path: String| {
            let _ = tx.try_send(Msg::SaveEditorToLibrary {
                editor_id: self.editor_id,
                path,
                overwrite: false,
            });
        };

        egui::Modal::new(egui::Id::new("egui_save_to_library")).show(ctx, |ui| {
            ui.set_width(MODAL_WIDTH);

            ui.heading(
                egui::RichText::new("Save to Library")
                    .size(14.0)
                    .color(ui.visuals().strong_text_color()),
            );
            ui.separator();
            ui.add_space(6.0);

            let input_bg = ui.visuals().extreme_bg_color;
            let response = ui.add_sized(
                [ui.available_width(), INPUT_HEIGHT],
                egui::TextEdit::singleline(&mut self.temp)
                    .hint_text("folder/name")
                    .desired_width(f32::INFINITY)
                    .vertical_align(egui::Align::Center)
                    .background_color(input_bg)
                    .margin(egui::Margin::symmetric(6, 2)),
            );
            response.request_focus();

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(2.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("Save").size(12.0))
                            .fill(ui.visuals().selection.bg_fill)
                            .min_size(egui::vec2(64.0, 22.0)),
                    )
                    .clicked()
                {
                    confirm_action(self.temp.clone());
                }
                if ui
                    .add(egui::Button::new("Cancel").min_size(egui::vec2(64.0, 22.0)))
                    .clicked()
                {
                    let _ = tx.try_send(Msg::PopModal);
                }
            });

            response
                .on_key_escape(|| {
                    let _ = tx.try_send(Msg::PopModal);
                })
                .on_key_enter(|| {
                    confirm_action(self.temp.clone());
                });
        });
    }
}
/// Owned modal for capturing a workspace name. Used for both creating a new
/// workspace (`workspace_id == None`) and renaming an existing one.
#[derive(Debug)]
pub struct WorkspaceNameModal {
    workspace_id: Option<WorkspaceId>,
    temp: String,
}

impl WorkspaceNameModal {
    pub fn new(workspace_id: Option<WorkspaceId>, initial: &str) -> Self {
        Self {
            workspace_id,
            temp: initial.into(),
        }
    }
}

impl Modal for WorkspaceNameModal {
    fn show(&mut self, ctx: &Context, tx: Sender<Msg>) {
        const MODAL_WIDTH: f32 = 240.0;
        const INPUT_HEIGHT: f32 = 30.0;

        let heading = match self.workspace_id {
            Some(_) => "Rename Workspace",
            None => "New Workspace",
        };
        let workspace_id = self.workspace_id;
        let confirm_tx = tx.clone();
        let confirm_action = move |name: String| {
            let msg = match workspace_id {
                Some(id) => Msg::RenameWorkspace(id, name),
                None => Msg::NewWorkspace(name),
            };
            let _ = confirm_tx.try_send(Msg::Batch(vec![msg, Msg::PopModal]));
        };
        egui::Modal::new(egui::Id::new("egui_confirm")).show(ctx, |ui| {
            ui.set_width(MODAL_WIDTH);

            ui.heading(
                egui::RichText::new(heading)
                    .size(14.0)
                    .color(ui.visuals().strong_text_color()),
            );
            ui.separator();
            ui.add_space(6.0);

            let input_bg = ui.visuals().extreme_bg_color;
            let response = ui.add_sized(
                [ui.available_width(), INPUT_HEIGHT],
                egui::TextEdit::singleline(&mut self.temp)
                    .desired_width(f32::INFINITY)
                    .vertical_align(egui::Align::Center)
                    .background_color(input_bg)
                    .margin(egui::Margin::symmetric(6, 2)),
            );
            response.request_focus();

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(2.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ok_label = match self.workspace_id {
                    Some(_) => "Rename",
                    None => "Create",
                };
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(ok_label).size(12.0))
                            .fill(ui.visuals().selection.bg_fill)
                            .min_size(egui::vec2(64.0, 22.0)),
                    )
                    .clicked()
                {
                    confirm_action(self.temp.clone());
                }
                if ui
                    .add(egui::Button::new("Cancel").min_size(egui::vec2(64.0, 22.0)))
                    .clicked()
                {
                    let _ = tx.try_send(Msg::PopModal);
                }
            });

            response
                .on_key_escape(|| {
                    let _ = tx.try_send(Msg::PopModal);
                })
                .on_key_enter(|| {
                    confirm_action(self.temp.clone());
                });
        });
    }
}

#[derive(Debug)]
pub struct RenameModal {
    editor_id: egui::Id,
    temp: String,
}
impl RenameModal {
    pub fn new(editor_id: egui::Id, initial_value: &str) -> Self {
        let temp = initial_value.into();
        Self { editor_id, temp }
    }
}

impl Modal for RenameModal {
    fn show(&mut self, ctx: &Context, tx: Sender<Msg>) {
        const MODAL_WIDTH: f32 = 240.0;
        const INPUT_HEIGHT: f32 = 30.0;

        let confirm_action = |new_name: String| {
            let _ = tx.try_send(Msg::Batch(vec![
                Msg::RenameWindow(self.editor_id, new_name),
                Msg::PopModal,
            ]));
        };
        egui::Modal::new(egui::Id::new("egui_confirm")).show(ctx, |ui| {
            ui.set_width(MODAL_WIDTH);

            ui.heading(
                egui::RichText::new("Rename Editor")
                    .size(14.0)
                    .color(ui.visuals().strong_text_color()),
            );
            ui.separator();
            ui.add_space(6.0);

            let input_bg = ui.visuals().extreme_bg_color;
            let response = ui.add_sized(
                [ui.available_width(), INPUT_HEIGHT],
                egui::TextEdit::singleline(&mut self.temp)
                    .desired_width(f32::INFINITY)
                    .vertical_align(egui::Align::Center)
                    .background_color(input_bg)
                    .margin(egui::Margin::symmetric(6, 2)),
            );
            response.request_focus();

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(2.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("Rename").size(12.0))
                            .fill(ui.visuals().selection.bg_fill)
                            .min_size(egui::vec2(64.0, 22.0)),
                    )
                    .clicked()
                {
                    confirm_action(self.temp.clone());
                };
                if ui
                    .add(egui::Button::new("Cancel").min_size(egui::vec2(64.0, 22.0)))
                    .clicked()
                {
                    let _ = tx.try_send(Msg::PopModal);
                };
            });

            response
                .on_key_escape(|| {
                    let _ = tx.try_send(Msg::PopModal);
                })
                .on_key_enter(|| {
                    confirm_action(self.temp.clone());
                });
        });
    }
}
