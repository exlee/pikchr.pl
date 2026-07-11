use eframe::egui;

use crate::OutputType;

/// Serialized state shared by editors that evaluate source into renderable output.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RenderedEditorState {
    pub id: egui::Id,
    pub(crate) visible: bool,
    pub(crate) target_svg: egui::Id,
    pub(crate) content: String,
    pub(crate) pikchr_content: String,
    pub(crate) index: usize,
    pub(crate) name: String,
    pub(crate) error: Option<String>,
    #[serde(default = "render_enabled")]
    pub(crate) render: bool,
    #[serde(default)]
    pub(crate) output_type: OutputType,
}

impl RenderedEditorState {
    pub(crate) fn new(id: egui::Id, target_svg: egui::Id, content: String) -> Self {
        Self {
            id,
            visible: true,
            target_svg,
            content,
            pikchr_content: String::new(),
            index: 1,
            name: id.short_debug_format(),
            error: None,
            render: true,
            output_type: OutputType::Pikchr,
        }
    }
}

fn render_enabled() -> bool {
    true
}

macro_rules! impl_rendered_editor_state {
    ($editor:ty) => {
        impl std::ops::Deref for $editor {
            type Target = RenderedEditorState;

            fn deref(&self) -> &Self::Target {
                &self.core
            }
        }

        impl std::ops::DerefMut for $editor {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.core
            }
        }
    };
}

pub(crate) use impl_rendered_editor_state;
