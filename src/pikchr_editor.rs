use eframe::egui::{self, Context, Ui, text_edit::TextEditOutput};
use tokio::sync::{mpsc::Sender, watch};

use crate::{
    EditorType, Msg,
    editor::{self, Editor, GenericEditor, HandleEnter as _},
    impl_generated_content, impl_id, impl_indexable, impl_initialize, impl_initialize_tx,
    impl_render, impl_target, impl_visible,
    mini_window::{self, EditorWindow, HasMenu, HasName as _, MiniWindow, RawContent},
    setter_getter_for_trait,
    text_highlighting::memoized_syntax_layouter,
};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PikchrEditor {
    pub id: egui::Id,
    target_svg: egui::Id,
    pub(crate) visible: bool,
    pub(crate) content: String,
    pub(crate) index: usize,
    #[serde(skip_serializing, default)]
    initialized: bool,
    #[serde(skip)]
    watch_tx: Option<watch::Sender<(egui::Context, egui::Id, String)>>,
    error: Option<String>,
    name: String,
    /// Whether the render (SVG) window should be shown. When false the
    /// pikchr code is still computed and available for inclusion by other
    /// editors, but no render window is displayed.
    #[serde(default = "default_render")]
    pub(crate) render: bool,
    #[serde(default)]
    pub(crate) output_type: crate::OutputType,
}
fn default_render() -> bool {
    true
}
impl PikchrEditor {
    pub fn new(id: egui::Id, target_svg: egui::Id) -> Self {
        Self {
            visible: true,
            content: String::new(),
            id,
            name: id.short_debug_format(),
            target_svg,
            index: 1,
            watch_tx: None,
            initialized: false,
            error: None,
            render: true,
            output_type: crate::OutputType::Pikchr,
        }
    }
}

impl EditorWindow for PikchrEditor {
    fn get_editor_window(&self) -> crate::mini_window::EditorWindowView<'_> {
        crate::mini_window::EditorWindowView {
            index: &self.index,
            id: &self.id,
            content: self as &dyn mini_window::GeneratedContent,
            editor_type: self as &dyn mini_window::EditorType,
            mini_window: self as &dyn MiniWindow,
            name: &self.name,
        }
    }
}
impl HasMenu for PikchrEditor {
    fn has_menu(&self) -> bool {
        false
    }

    fn menu(&self, ui: &mut Ui, tx: Sender<Msg>) {
        ui.menu_button("View", |ui| {
            if ui.button("Font Size").clicked() {
                let _ = tx.try_send(Msg::FontSizeModal(self.id));
                ui.close();
            }
        });
    }
}
impl GenericEditor for PikchrEditor {
    fn editor_spec(&mut self, editor_id: egui::Id, ui: &mut Ui) -> TextEditOutput {
        let syntax = match self.output_type {
            crate::OutputType::Pikchr => "Pikchr",
            crate::OutputType::Svgbob => "Plain Text",
        };
        egui::TextEdit::multiline(&mut self.content)
            .code_editor()
            .desired_width(f32::INFINITY)
            .id(editor_id)
            .layouter(&mut |ui, textbuffer, wrap_width| {
                memoized_syntax_layouter(editor_id, ui, textbuffer, wrap_width, syntax)
            })
            .show(ui)
    }

    fn handle_enter(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) {
        self.handle_indent(ctx, ui, editor_id, |current_line| {
            editor::get_line_indent(current_line)
        });
    }

    fn editor_on_changed(&self, _tx: Sender<Msg>, ctx: &Context) {
        let _ = self
            .watch_tx
            .as_ref()
            .expect("Should be initialized")
            .send((ctx.clone(), self.id, self.get_raw_content()));
    }

    fn initialize(&mut self, tx: Sender<Msg>) {
        mini_window::InitializeWatchTx::initialize(self, tx);
    }
}
impl MiniWindow for PikchrEditor {
    fn get_title(&self) -> String {
        format!("Pikchr - {}", self.get_name())
    }
    fn help_topic(&self) -> crate::help::HelpTopic {
        crate::help::HelpTopic::Pikchr
    }
    fn can_save_to_library(&self) -> bool {
        true
    }
}
impl PikchrEditor {}
impl Editor for PikchrEditor {}
impl_render!(PikchrEditor, render);
impl crate::mini_window::EditorType for PikchrEditor {
    fn get_editor_type(&self) -> crate::EditorType {
        EditorType::Pikchr
    }
}

//impl crate::mini_window::InitializeWatchTx for PikchrEditor { }
impl_id!(PikchrEditor, id);
impl_target!(PikchrEditor, target_svg);
impl_visible!(PikchrEditor, visible);
impl_initialize!(PikchrEditor, initialized);
impl_indexable!(PikchrEditor);
impl_generated_content!(PikchrEditor, content);
impl_initialize_tx!(
    PikchrEditor, watch_tx,
    on_change: |(ctx,id,content)| Msg::UpdateRender(ctx, id, content),
    data: (Context,egui::Id, String),
    empty: (Context::default(),egui::Id::new(""), String::new())
);

setter_getter_for_trait! { (content => String | content.clone() => String) for PikchrEditor as raw_content for mini_window::RawContent }
setter_getter_for_trait! { (error => Option<String> | error.clone() => Option<String>) for PikchrEditor as error for mini_window::HasError }
setter_getter_for_trait! { (name => String | name.clone() => String) for PikchrEditor as name for mini_window::HasName }
