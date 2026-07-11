use eframe::egui::{self, Context, Ui};
use tokio::sync::mpsc::Sender;

use crate::{
    Msg,
    editor::{self, GenericEditor, HandleEnter as _},
    editor_state::{RenderedEditorState, impl_rendered_editor_state},
    impl_generated_content, impl_id, impl_indexable, impl_render, impl_target, impl_visible,
    mini_window::{self, HasMenu, HasName as _, MiniWindow},
    sender_ext::DebouncedTrySend as _,
    setter_getter_for_trait,
    text_highlighting::memoized_syntax_layouter,
};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct PrologEditor {
    #[serde(flatten)]
    core: RenderedEditorState,
}
impl PrologEditor {
    pub fn new(id: egui::Id, target_svg: egui::Id) -> Self {
        Self {
            core: RenderedEditorState::new(id, target_svg, Self::template_content()),
        }
    }
    fn template_content() -> String {
        r#"
diagram --> "box".
        "#
        .trim()
        .into()
    }
}
impl_rendered_editor_state!(PrologEditor);
impl mini_window::EditorWindow for PrologEditor {
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

impl HasMenu for PrologEditor {}
impl GenericEditor for PrologEditor {
    fn editor_spec(&mut self, editor_id: egui::Id, ui: &mut Ui) -> egui::text_edit::TextEditOutput {
        egui::TextEdit::multiline(&mut self.content)
            .code_editor()
            .desired_width(f32::INFINITY)
            .id(editor_id)
            .frame(true)
            .layouter(&mut |ui, textbuffer, wrap_width| {
                memoized_syntax_layouter(editor_id, ui, textbuffer, wrap_width, "Prolog")
            })
            .show(ui)
    }

    fn handle_enter(&mut self, ctx: &Context, ui: &mut Ui, editor_id: egui::Id) {
        self.handle_indent(ctx, ui, editor_id, |current_line| {
            if !current_line.is_empty() {
                if current_line.ends_with(".") {
                    return String::new();
                }
                for op in [":-", "-->", "("] {
                    if let Some(idx) = current_line.rfind(op)
                        && idx > 0
                    {
                        return " ".repeat(idx + op.len() + 1);
                    }
                }
            }
            editor::get_line_indent(current_line)
        });
    }

    fn editor_on_changed(&self, tx: Sender<Msg>, ctx: &Context) {
        let _ = tx.try_send_debounced(
            self.id,
            100,
            Msg::UpdateProlog(ctx.clone(), self.id, self.content.clone()),
        );
    }

    fn initialize(&mut self, _tx: Sender<Msg>) {}
}
impl MiniWindow for PrologEditor {
    fn get_title(&self) -> String {
        format!("Prolog - {}", self.get_name())
    }
    fn help_topic(&self) -> crate::help::HelpTopic {
        crate::help::HelpTopic::Prolog
    }
    fn can_save_to_library(&self) -> bool {
        true
    }
}

impl crate::mini_window::EditorType for PrologEditor {
    fn get_editor_type(&self) -> crate::EditorType {
        crate::EditorType::Prolog
    }
}

impl editor::Editor for PrologEditor {}
impl_render!(PrologEditor, render);
impl_id!(PrologEditor, id);
impl_indexable!(PrologEditor);
impl_visible!(PrologEditor, visible);
impl_generated_content!(PrologEditor, pikchr_content);
impl_target!(PrologEditor, target_svg);
setter_getter_for_trait! { (content => String | content.clone() => String) for PrologEditor as raw_content for mini_window::RawContent }
setter_getter_for_trait! { (error => Option<String> | error.clone() => Option<String>) for PrologEditor as error for mini_window::HasError }
setter_getter_for_trait! { (name => String | name.clone() => String) for PrologEditor as name for mini_window::HasName }
