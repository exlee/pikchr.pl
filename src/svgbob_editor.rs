use eframe::egui::{self, Context, Ui, text_edit::TextEditOutput};
use tokio::sync::{mpsc::Sender, watch};

use crate::{
    Msg,
    editor::{self, GenericEditor, HandleEnter as _},
    impl_generated_content, impl_id, impl_indexable, impl_initialize, impl_initialize_tx,
    impl_target, impl_visible,
    mini_window::{
        self, EditorWindow, HasMenu, HasName as _, MiniWindow, RawContent, RenderToggle,
    },
    setter_getter_for_trait,
};

/// A dedicated ASCII-art editor. It is permanently rendered by Svgbob and has
/// its own binding hooks, separate from Pikchr and the generator editors.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SvgbobEditor {
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
    #[serde(default = "default_render")]
    pub(crate) render: bool,
}

fn default_render() -> bool {
    true
}

impl SvgbobEditor {
    pub fn new(id: egui::Id, target_svg: egui::Id) -> Self {
        Self {
            id,
            target_svg,
            visible: true,
            content: String::new(),
            index: 1,
            initialized: false,
            watch_tx: None,
            error: None,
            name: id.short_debug_format(),
            render: true,
        }
    }
}

impl EditorWindow for SvgbobEditor {
    fn get_editor_window(&self) -> mini_window::EditorWindowView<'_> {
        mini_window::EditorWindowView {
            index: &self.index,
            id: &self.id,
            content: self as &dyn mini_window::GeneratedContent,
            editor_type: self as &dyn mini_window::EditorType,
            mini_window: self as &dyn MiniWindow,
            name: &self.name,
        }
    }
}

impl HasMenu for SvgbobEditor {}

impl GenericEditor for SvgbobEditor {
    fn editor_spec(&mut self, editor_id: egui::Id, ui: &mut Ui) -> TextEditOutput {
        egui::TextEdit::multiline(&mut self.content)
            .code_editor()
            .desired_width(f32::INFINITY)
            .id(editor_id)
            .show(ui)
    }

    // ASCII art should not inherit indentation when adding a new line.
    fn handle_enter(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) {
        self.handle_indent(ctx, ui, editor_id, |_| String::new());
    }

    // Keep Tab available for dedicated Svgbob bindings instead of consuming it
    // for the shared source-editor indentation behavior.
    fn handle_tab_binding(&mut self, _ctx: &Context, _ui: &mut Ui, _editor_id: egui::Id) -> bool {
        false
    }

    // Cmd/Ctrl-R is intentionally not reserved here; future Svgbob-specific
    // bindings can use it without conflicting with the generic rename binding.
    fn handle_command_bindings(&mut self, _ctx: &Context, _ui: &mut Ui, _tx: &Sender<Msg>) {}

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

impl MiniWindow for SvgbobEditor {
    fn get_title(&self) -> String {
        format!("Svgbob - {}", self.get_name())
    }

    fn help_topic(&self) -> crate::help::HelpTopic {
        crate::help::HelpTopic::Svgbob
    }

    fn can_save_to_library(&self) -> bool {
        true
    }
}

impl RenderToggle for SvgbobEditor {
    fn has_renderer(&self) -> bool {
        true
    }

    fn render_enabled(&self) -> bool {
        self.render
    }

    fn set_render_enabled(&mut self, on: bool) {
        self.render = on;
    }

    fn output_type(&self) -> crate::OutputType {
        crate::OutputType::Svgbob
    }

    fn has_output_selector(&self) -> bool {
        false
    }
}

impl editor::Editor for SvgbobEditor {}
impl crate::mini_window::EditorType for SvgbobEditor {
    fn get_editor_type(&self) -> crate::EditorType {
        crate::EditorType::Svgbob
    }
}

impl_id!(SvgbobEditor, id);
impl_target!(SvgbobEditor, target_svg);
impl_visible!(SvgbobEditor, visible);
impl_initialize!(SvgbobEditor, initialized);
impl_indexable!(SvgbobEditor);
impl_generated_content!(SvgbobEditor, content);
impl_initialize_tx!(
    SvgbobEditor, watch_tx,
    on_change: |(ctx,id,content)| Msg::UpdateRender(ctx, id, content),
    data: (Context,egui::Id, String),
    empty: (Context::default(),egui::Id::new(""), String::new())
);

setter_getter_for_trait! { (content => String | content.clone() => String) for SvgbobEditor as raw_content for mini_window::RawContent }
setter_getter_for_trait! { (error => Option<String> | error.clone() => Option<String>) for SvgbobEditor as error for mini_window::HasError }
setter_getter_for_trait! { (name => String | name.clone() => String) for SvgbobEditor as name for mini_window::HasName }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_permanently_a_svgbob_renderer_without_output_selector() {
        let editor = SvgbobEditor::new(egui::Id::new("svgbob"), egui::Id::new("render"));
        assert_eq!(editor.output_type(), crate::OutputType::Svgbob);
        assert!(!editor.has_output_selector());
    }
}
