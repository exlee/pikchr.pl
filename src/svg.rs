use eframe::egui::{self, Context, Vec2};
use std::fmt;

use crate::{
    Msg,
    icons::{AppIcon, icon_image},
    impl_id, impl_indexable, impl_initialize, impl_initialize_tx,
    mini_window::{
        self, HasMenu, HasName as _, InitializeWatchTx, InnerWindow, MiniWindow, Visible,
    },
    setter_getter_for_trait,
};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct SvgWindow {
    pub id: egui::Id,
    pub owner_id: egui::Id,
    #[serde(skip)]
    pub diagram_texture: Option<egui::TextureHandle>,
    pub svg_string: Option<String>,
    pub initial_size: Vec2,
    pub prev_size: Option<Vec2>,
    pub scale: f32,
    #[serde(skip)]
    image: Option<egui::ColorImage>,
    #[serde(skip)]
    watch_tx: Option<tokio::sync::watch::Sender<(egui::Context, egui::Id)>>,
    #[serde(default)]
    pub(crate) output_type: crate::OutputType,
    index: usize,
    #[serde(skip_serializing, default)]
    initialized: bool,
    name: String,
}
impl fmt::Debug for SvgWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SvgWindow")
            .field("id", &self.id)
            // Use a placeholder string for the non-Debug field
            .field(
                "diagram_texture",
                &self.diagram_texture.as_ref().map(|_| "TextureHandle(...)"),
            )
            .field("image", &self.image.as_ref().map(|_| "Image(...)"))
            .field("svg_string", &self.svg_string)
            .field("initial_size", &self.initial_size)
            .field("prev_size", &self.prev_size)
            .field("scale", &self.scale)
            // Skip complex channels or internal types entirely if irrelevant
            .field("watch_tx", &"Option<Sender>")
            .finish_non_exhaustive() // Indicates other fields exist (index, initialized)
    }
}

impl SvgWindow {
    pub fn new(id: egui::Id, owner_id: egui::Id) -> Self {
        Self {
            id,
            owner_id,
            index: 1,
            diagram_texture: None,
            svg_string: None,
            initial_size: Vec2::from((300.0, 300.0)),
            name: owner_id.short_debug_format(),
            prev_size: None,
            scale: 1.5,
            image: None,
            output_type: crate::OutputType::Pikchr,
            initialized: false,
            watch_tx: None,
        }
    }
}

impl_indexable!(SvgWindow);
impl_initialize!(SvgWindow, initialized);
impl_initialize_tx!(
    SvgWindow, watch_tx,
    on_change: |(ctx,id)| Msg::RequestRedraw(ctx,id),
    data: (Context, egui::Id),
    empty: (egui::Context::default(), egui::Id::new(""))
);
impl HasMenu for SvgWindow {
    fn has_menu(&self) -> bool {
        true
    }
    fn menu(&self, ui: &mut egui::Ui, tx: tokio::sync::mpsc::Sender<Msg>) {
        ui.menu_image_button(
            icon_image(AppIcon::Export, ui.visuals().text_color()),
            |ui| export_menu(ui, &tx, self),
        );
    }
}

fn export_menu(ui: &mut egui::Ui, tx: &tokio::sync::mpsc::Sender<Msg>, window: &SvgWindow) {
    egui::Grid::new(("copy_menu", window.id))
        .num_columns(3)
        .show(ui, |ui| {
            export_row(
                ui,
                tx,
                "SVG",
                window.id,
                window.get_title(),
                crate::ExportType::Svg,
            );
            export_row(
                ui,
                tx,
                "PNG",
                window.id,
                window.get_title(),
                crate::ExportType::Png,
            );
            export_row(
                ui,
                tx,
                "Transparent PNG",
                window.id,
                window.get_title(),
                crate::ExportType::PngTransparent,
            );
            export_row(
                ui,
                tx,
                &format!("{} Source", window.output_type.label()),
                window.owner_id,
                window.get_title(),
                crate::ExportType::Source(window.output_type),
            );
        });
}

fn export_row(
    ui: &mut egui::Ui,
    tx: &tokio::sync::mpsc::Sender<Msg>,
    label: &str,
    id: egui::Id,
    file_name: String,
    export_type: crate::ExportType,
) {
    ui.label(label);
    if ui.small_button("FILE").clicked() {
        let _ = tx.try_send(Msg::ExportModal(id, file_name, export_type));
        ui.close();
    }
    if ui.small_button("COPY").clicked() {
        let _ = tx.try_send(Msg::CopyExport(
            ui.ctx().clone(),
            id,
            export_type,
            Box::new(ui.visuals().clone()),
        ));
        ui.close();
    }
    ui.end_row();
}

impl crate::mini_window::RenderToggle for SvgWindow {}

impl MiniWindow for SvgWindow {
    fn outer_window(&self, ctx: &egui::Context) -> egui::Window<'static> {
        egui::Window::new(self.get_title())
            .id(self.id)
            .resizable(true)
            .default_size(self.initial_size)
            .frame(egui::Frame::window(&ctx.style()).inner_margin(0.0))
    }
    fn should_show(&self) -> bool {
        self.diagram_texture.is_some()
    }

    fn close_requested(
        &mut self,
        ctx: &egui::Context,
        _command_only: bool,
        tx: tokio::sync::mpsc::Sender<Msg>,
    ) {
        let _ = tx.try_send(Msg::SetRenderEnabled(ctx.clone(), self.owner_id, false));
    }

    fn should_be_listed(&self) -> bool {
        false
    }

    fn get_title(&self) -> String {
        format!("Render - {}", self.get_name())
    }
    fn help_topic(&self) -> crate::help::HelpTopic {
        crate::help::HelpTopic::Render
    }
}
impl InnerWindow for SvgWindow {
    fn inner_window(
        &mut self,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        tx: tokio::sync::mpsc::Sender<crate::Msg>,
        background: crate::state::DiagramBackground,
    ) {
        self.initialize(tx.clone());
        if self.diagram_texture.is_none() {
            return;
        }
        let texture = self.diagram_texture.as_ref().expect("Just checked");
        let background_color = background.resolve(ui.visuals()).to_opaque();
        egui::Frame::new().inner_margin(10.0).show(ui, |ui| {
            egui::Frame::new()
                .fill(background_color)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                        let available = ui.available_size();
                        if self.prev_size.is_some() && self.prev_size != Some(available.ceil()) {
                            self.scale = (available.ceil() / self.initial_size.ceil()).max_elem();
                            let _ = self
                                .watch_tx
                                .as_ref()
                                .expect("Should be initialized")
                                .send((ui.ctx().clone(), self.id));
                        }
                        self.prev_size = Some(available.ceil());

                        ui.set_min_size(available);

                        let logical_size = texture.size_vec2() / self.scale;
                        let aspect = logical_size.x / logical_size.y;
                        let mut new_size = available;

                        if available.x / available.y > aspect {
                            new_size.x = available.y * aspect;
                        } else {
                            new_size.y = available.x / aspect;
                        }

                        let img = egui::Image::new(texture).fit_to_exact_size(new_size).uv(
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        );

                        ui.centered_and_justified(|ui| {
                            ui.add(img);
                        });
                    });
                });
        });
    }
}

pub struct SvgWindowView<'a> {
    pub diagram_texture: &'a mut Option<egui::TextureHandle>,
    pub svg_string: &'a mut Option<String>,
    pub scale: &'a mut f32,
    pub image: &'a mut Option<egui::ColorImage>,
    pub id: &'a egui::Id,
}

impl mini_window::SvgWindow for SvgWindow {
    fn get_svg_window_mut(&mut self) -> self::SvgWindowView<'_> {
        SvgWindowView {
            id: &mut self.id,
            diagram_texture: &mut self.diagram_texture,
            svg_string: &mut self.svg_string,
            scale: &mut self.scale,
            image: &mut self.image,
        }
    }
}

impl mini_window::NormalWindow for SvgWindow {
    fn get_window(&self) -> mini_window::WindowView<'_> {
        mini_window::WindowView {
            index: &self.index,
            id: &self.id,
            mini_window: self as &dyn MiniWindow,
        }
    }
}
impl_id!(SvgWindow, id);
impl Visible for SvgWindow {
    fn visible(&self) -> bool {
        true
    }

    fn set_visible(&mut self, _new: bool) {}
}
setter_getter_for_trait! { (name => String | name.clone() => String) for SvgWindow as name for mini_window::HasName }

#[cfg(test)]
mod tests {
    use super::*;

    use egui_kittest::{Harness, kittest::Queryable as _};

    #[test]
    fn closing_render_window_turns_off_owner_rendering() {
        let owner_id = egui::Id::new("owner");
        let mut window = SvgWindow::new(egui::Id::new("render"), owner_id);
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        window.close_requested(&egui::Context::default(), true, tx);

        assert!(matches!(
            rx.try_recv(),
            Ok(Msg::SetRenderEnabled(_, id, false)) if id == owner_id
        ));
    }

    #[test]
    fn render_window_has_no_independent_visibility_state() {
        let mut window = SvgWindow::new(egui::Id::new("render"), egui::Id::new("owner"));

        window.set_visible(false);

        assert!(window.visible());
    }

    #[test]
    fn export_menu_has_file_and_copy_actions_for_every_format() {
        let window = SvgWindow::new(egui::Id::new("render"), egui::Id::new("owner"));
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let harness = Harness::new_ui(move |ui| export_menu(ui, &tx, &window));

        for label in ["SVG", "PNG", "Transparent PNG", "Pikchr Source"] {
            assert!(harness.query_by_label(label).is_some(), "missing {label}");
        }
        assert_eq!(harness.query_all_by_label("FILE").count(), 4);
        assert_eq!(harness.query_all_by_label("COPY").count(), 4);
    }
}
