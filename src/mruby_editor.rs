use eframe::egui::{self, Context, Ui};
use tokio::sync::mpsc::Sender;

use crate::{
    Msg,
    editor::{self, GenericEditor, HandleEnter as _},
    impl_generated_content, impl_id, impl_indexable, impl_render, impl_target, impl_visible,
    mini_window::{self, HasMenu, HasName as _, MiniWindow},
    sender_ext::DebouncedTrySend as _,
    setter_getter_for_trait,
    text_highlighting::memoized_syntax_layouter,
};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MrubyEditor {
    pub id: egui::Id,
    pub(crate) visible: bool,
    target_svg: egui::Id,
    content: String,
    pikchr_content: String,
    index: usize,
    name: String,
    error: Option<String>,
    #[serde(default = "default_render")]
    pub(crate) render: bool,
    #[serde(default)]
    pub(crate) output_type: crate::OutputType,
}

fn default_render() -> bool {
    true
}

impl MrubyEditor {
    pub fn new(id: egui::Id, target_svg: egui::Id) -> Self {
        Self {
            visible: true,
            pikchr_content: String::new(),
            content: Self::template_content(),
            name: id.short_debug_format(),
            id,
            target_svg,
            index: 1,
            error: None,
            render: true,
            output_type: crate::OutputType::Pikchr,
        }
    }

    fn template_content() -> String {
        r#"
def run(pikchr)
  puts pikchr
end

run "box"
        "#
        .trim()
        .into()
    }
}

impl mini_window::EditorWindow for MrubyEditor {
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

impl HasMenu for MrubyEditor {}
impl MiniWindow for MrubyEditor {
    fn get_title(&self) -> String {
        format!("Ruby - {}", self.get_name())
    }
    fn help_topic(&self) -> crate::help::HelpTopic {
        crate::help::HelpTopic::Mruby
    }
    fn can_save_to_library(&self) -> bool {
        true
    }
}
impl GenericEditor for MrubyEditor {
    fn editor_spec(&mut self, editor_id: egui::Id, ui: &mut Ui) -> egui::text_edit::TextEditOutput {
        egui::TextEdit::multiline(&mut self.content)
            .code_editor()
            .desired_width(f32::INFINITY)
            .id(editor_id)
            .layouter(&mut |ui, textbuffer, wrap_width| {
                memoized_syntax_layouter(editor_id, ui, textbuffer, wrap_width, "Ruby")
            })
            .show(ui)
    }

    fn handle_enter(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) {
        self.handle_indent(ctx, ui, editor_id, |current_line| {
            let indent = editor::get_line_indent(current_line);
            if current_line.trim_end().ends_with(" do")
                || current_line.trim_start().starts_with("def ")
                || current_line.trim_start().starts_with("class ")
                || current_line.trim_start().starts_with("module ")
            {
                format!("{indent}  ")
            } else {
                indent
            }
        });
    }

    fn editor_on_changed(&self, tx: Sender<Msg>, ctx: &Context) {
        let _ = tx.try_send_debounced(
            self.id,
            400,
            Msg::UpdateMruby(ctx.clone(), self.id, self.content.clone()),
        );
    }

    fn initialize(&mut self, _tx: Sender<Msg>) {}
}

impl crate::mini_window::EditorType for MrubyEditor {
    fn get_editor_type(&self) -> crate::EditorType {
        crate::EditorType::Mruby
    }
}

impl editor::Editor for MrubyEditor {}
impl_render!(MrubyEditor, render);
impl_id!(MrubyEditor, id);
impl_indexable!(MrubyEditor);
impl_visible!(MrubyEditor, visible);
impl_generated_content!(MrubyEditor, pikchr_content);
impl_target!(MrubyEditor, target_svg);
setter_getter_for_trait! { (content => String | content.clone() => String) for MrubyEditor as raw_content for mini_window::RawContent }
setter_getter_for_trait! { (error => Option<String> | error.clone() => Option<String>) for MrubyEditor as error for mini_window::HasError }
setter_getter_for_trait! { (name => String | name.clone() => String) for MrubyEditor as name for mini_window::HasName }
