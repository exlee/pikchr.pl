use eframe::egui::{self, Context, Ui};
use tokio::sync::mpsc::Sender;

mod grammar;
mod guide;

use grammar::{GrammarViewState, render_grammar};
use guide::render_guide;

use crate::{
    Msg, impl_id, impl_indexable, impl_visible,
    mini_window::{self, HasMenu, Id, MiniWindow, NormalWindow, RenderToggle, WindowView},
    state::DiagramBackground,
};

/// Which document a [`HelpWindow`] shows. `Overview` and the per-editor
/// variants render the User Guide (with a context section); `Grammar` renders
/// the full Pikchr grammar reference with a table of contents.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HelpTopic {
    #[default]
    Overview,
    Pikchr,
    Svgbob,
    Prolog,
    Tcl,
    Mruby,
    PlainText,
    Render,
    Grammar,
}

impl HelpTopic {
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "DiagramIDE Help",
            Self::Pikchr => "Pikchr Help",
            Self::Svgbob => "Svgbob Help",
            Self::Prolog => "Prolog Help",
            Self::Tcl => "Tcl Help",
            Self::Mruby => "Ruby Help",
            Self::PlainText => "Plain Text Help",
            Self::Render => "Render Window Help",
            Self::Grammar => "Pikchr Grammar",
        }
    }

    /// Whether this topic renders the big grammar document (which needs a TOC
    /// and a wider window) rather than the guide body.
    fn is_grammar(self) -> bool {
        matches!(self, Self::Grammar)
    }
}

// ── Help as a first-class window ──────────────────────────────────────────

/// A help/documentation window. One window type that renders different content
/// depending on its [`HelpTopic`]: the User Guide, or the Pikchr Grammar
/// reference (with a sidebar table of contents).
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct HelpWindow {
    pub id: egui::Id,
    pub(crate) visible: bool,
    pub topic: HelpTopic,
    index: usize,
    /// Pending TOC navigation target for the Grammar view. `Some(i)` asks the
    /// renderer to scroll heading `#i` into view; consumed on use.
    scroll_target: Option<usize>,
    #[serde(skip, default)]
    grammar_view: GrammarViewState,
}

impl HelpWindow {
    pub fn new(id: egui::Id, topic: HelpTopic) -> Self {
        Self {
            id,
            visible: true,
            topic,
            index: 0,
            scroll_target: None,
            grammar_view: GrammarViewState::default(),
        }
    }
}

impl HasMenu for HelpWindow {}
impl RenderToggle for HelpWindow {}

impl MiniWindow for HelpWindow {
    fn get_title(&self) -> String {
        self.topic.title().to_owned()
    }

    fn help_topic(&self) -> HelpTopic {
        // The Guide topics are themselves help; the Grammar window documents
        // Pikchr, so its own Help button re-opens the Pikchr guide section.
        if self.topic.is_grammar() {
            HelpTopic::Pikchr
        } else {
            HelpTopic::Overview
        }
    }

    fn outer_window(&self, ctx: &Context) -> egui::Window<'static> {
        let default = if self.topic.is_grammar() {
            (900.0, 650.0)
        } else {
            (520.0, 560.0)
        };
        egui::Window::new(self.get_title())
            .resizable(true)
            .default_size(default)
            .min_width(360.0)
            .id(self.get_id())
            .frame(egui::Frame::window(&ctx.style()).inner_margin(0.0))
    }
}

impl NormalWindow for HelpWindow {
    fn get_window(&self) -> WindowView<'_> {
        WindowView {
            index: &self.index,
            id: &self.id,
            mini_window: self as &dyn MiniWindow,
        }
    }
}

impl mini_window::InnerWindow for HelpWindow {
    fn inner_window(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        tx: Sender<Msg>,
        _background: DiagramBackground,
    ) {
        if self.topic.is_grammar() {
            render_grammar(ui, &mut self.scroll_target, &mut self.grammar_view);
        } else {
            render_guide(ui, self.topic, &tx);
        }
    }
}

impl_id!(HelpWindow, id);
impl_indexable!(HelpWindow);
impl_visible!(HelpWindow, visible);
#[cfg(test)]
mod tests {
    use super::HelpTopic;

    #[test]
    fn help_topic_defaults_to_overview() {
        assert_eq!(HelpTopic::default(), HelpTopic::Overview);
    }
}
