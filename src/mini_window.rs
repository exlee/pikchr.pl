use eframe::egui::{self, Context, MenuBar, Ui};
use tokio::sync::{mpsc::Sender, watch};

use crate::{
    Msg,
    help::{HelpTopic, HelpWindow},
    icons::{AppIcon, CustomIcon, custom_icon, icon_button, selectable_icon_button},
    mruby_editor, pikchr_editor, plain_text_editor, prolog_editor,
    state::DiagramBackground,
    svg, svgbob_editor, tcl_editor,
};

pub trait Visible {
    fn visible(&self) -> bool;
    fn set_visible(&mut self, new: bool);
    fn toggle_visible(&mut self) {
        self.set_visible(!self.visible());
    }
}

#[macro_export]
macro_rules! impl_visible {
    ($struct:ident,$field_name:ident) => {
        impl $crate::mini_window::Visible for $struct {
            fn visible(&self) -> bool {
                self.$field_name
            }
            fn set_visible(&mut self, value: bool) {
                self.$field_name = value;
            }
        }
    };
}

/// Implements `MiniWindow::has_renderer`/`render_enabled`/`set_render_enabled`
/// for an editor that owns a render (SVG) window. The `$field` is the bool that
/// stores whether rendering is desired; when false the SVG window is hidden.
#[macro_export]
macro_rules! impl_render {
    ($struct:ident, $field:ident) => {
        impl $crate::mini_window::RenderToggle for $struct {
            fn has_renderer(&self) -> bool {
                true
            }
            fn render_enabled(&self) -> bool {
                self.$field
            }
            fn set_render_enabled(&mut self, on: bool) {
                self.$field = on;
            }
            fn output_type(&self) -> $crate::OutputType {
                self.output_type
            }
            fn set_output_type(&mut self, output_type: $crate::OutputType) {
                self.output_type = output_type;
            }
        }
    };
}

pub trait Id: Send + Sync {
    fn get_id(&self) -> egui::Id;
}

pub trait HasMenu: Send + Sync {
    fn has_menu(&self) -> bool {
        false
    }
    fn menu(&self, _ui: &mut Ui, _tx: Sender<Msg>) {}
}
pub trait HasError: Send + Sync {
    fn set_error(&mut self, error: Option<String>);
    fn get_error(&self) -> Option<String>;
}

pub trait HasName: Send + Sync {
    fn set_name(&mut self, name: String);
    fn get_name(&self) -> String;
}

pub trait InnerWindow {
    fn inner_window(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        tx: Sender<Msg>,
        background: DiagramBackground,
    );
}
pub trait RenderToggle: Send + Sync {
    /// Whether this window owns a render (SVG) window that can be toggled.
    fn has_renderer(&self) -> bool {
        false
    }
    /// Whether the render window is currently desired (shown).
    fn render_enabled(&self) -> bool {
        true
    }
    fn set_render_enabled(&mut self, _on: bool) {}
    fn output_type(&self) -> crate::OutputType {
        crate::OutputType::Pikchr
    }
    fn set_output_type(&mut self, _output_type: crate::OutputType) {}
    fn has_output_selector(&self) -> bool {
        true
    }
}

pub trait MiniWindow: Send + Sync + Visible + Id + HasMenu + InnerWindow + RenderToggle {
    fn get_title(&self) -> String;
    fn help_topic(&self) -> HelpTopic;
    fn can_save_to_library(&self) -> bool {
        false
    }

    fn should_be_listed(&self) -> bool {
        true
    }

    fn should_show(&self) -> bool {
        self.visible()
    }

    fn show(&mut self, ctx: &Context, tx: Sender<Msg>, background: DiagramBackground) {
        if !self.should_show() {
            return;
        };
        let mut visible = self.visible();
        let window = self.outer_window(ctx).open(&mut visible);

        window.show(ctx, |ui| {
            let style = ui.style_mut();
            style.spacing.menu_margin = egui::Margin {
                left: 10,
                right: 10,
                top: 10,
                bottom: 10,
            };
            egui::Frame::new().inner_margin(0.0).show(ui, |ui| {
                egui::Frame::new().inner_margin(0.0).show(ui, |ui| {
                    MenuBar::new().ui(ui, |ui| {
                        if self.has_menu() {
                            ui.add_space(8.0);
                            self.menu(ui, tx.clone());
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, AppIcon::Help)
                                .on_hover_text("Help for this window")
                                .clicked()
                            {
                                let _ = tx.try_send(Msg::ShowHelp(self.help_topic()));
                            }
                            if self.can_save_to_library()
                                && icon_button(ui, AppIcon::Save)
                                    .on_hover_text("Save to Library")
                                    .clicked()
                            {
                                let _ = tx.try_send(Msg::SaveEditorToLibraryRequest(
                                    ctx.clone(),
                                    self.get_id(),
                                ));
                            }

                            // Render toggle (only on windows that own a renderer).
                            if self.has_renderer() {
                                if self.has_output_selector() {
                                    let output_type = self.output_type();
                                    ui.menu_button("Output", |ui| {
                                        for candidate in
                                            [crate::OutputType::Pikchr, crate::OutputType::Svgbob]
                                        {
                                            if ui
                                                .selectable_label(
                                                    output_type == candidate,
                                                    candidate.label(),
                                                )
                                                .clicked()
                                            {
                                                self.set_output_type(candidate);
                                                let _ = tx.try_send(Msg::SetOutputType(
                                                    ctx.clone(),
                                                    self.get_id(),
                                                    candidate,
                                                ));
                                                ui.close();
                                            }
                                        }
                                    });
                                }
                                let render = self.render_enabled();
                                if selectable_icon_button(ui, AppIcon::Render, render)
                                    .on_hover_text("Render diagram\n(unselect for include-only)")
                                    .clicked()
                                {
                                    self.set_render_enabled(!render);
                                }
                            }
                            if self.can_save_to_library()
                                && custom_icon(
                                    ui,
                                    CustomIcon::Rename,
                                    Some(ui.visuals().text_color()),
                                )
                                .on_hover_text("Rename")
                                .clicked()
                            {
                                let _ = tx.try_send(Msg::RequestRename(ctx.clone(), self.get_id()));
                            }
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    // a
                                    if self.can_save_to_library()
                                        && icon_button(ui, AppIcon::Export)
                                            .on_hover_text("Export Library Entry as JSON")
                                            .clicked()
                                    {
                                        let _ = tx
                                            .try_send(Msg::ExportEditorLibraryEntry(self.get_id()));
                                    }
                                },
                            );
                        });
                    });
                });
                ui.add_space(2.0 * -ui.spacing().item_spacing.y);
                ui.separator();
                self.inner_window(ctx, ui, tx.clone(), background)
            });
        });
        let modifiers = ctx.input(|i| i.modifiers);
        if modifiers.command_only() && self.visible() != visible {
            let _ = tx.clone().try_send(Msg::DeleteWindow(self.get_id()));
        }
        self.set_visible(visible);
    }

    fn outer_window(&self, ctx: &Context) -> egui::Window<'static> {
        egui::Window::new(self.get_title())
            .resizable(true)
            .default_size((300.0, 150.0))
            .id(self.get_id())
            .frame(egui::Frame::window(&ctx.style()).inner_margin(0.0))
    }
}

pub trait Indexable: Send + Sync {
    fn set_index(&mut self, value: usize);
    fn get_index(&self) -> usize;
}

pub trait Initialize: Send + Sync + Id {
    fn is_initialized(&self) -> bool;
    fn set_initialized(&mut self);
}

pub trait Target: Send + Sync {
    fn get_target(&self) -> egui::Id;
    fn set_target(&mut self, target: egui::Id);
}

pub trait EditorType: Send + Sync {
    fn get_editor_type(&self) -> crate::EditorType;
}

pub trait GeneratedContent: Send + Sync + Indexable {
    fn get_generated_content(&self) -> String;
    fn set_generated_content(&mut self, value: String);
}
pub trait RawContent: Send + Sync + Indexable {
    fn get_raw_content(&self) -> String;
    fn set_raw_content(&mut self, value: String);
}
pub trait InitializeWatchTx: Send + Sync + Initialize {
    type ChangeData: Clone + Send + Sync + 'static;
    fn watch_change_fn(data: Self::ChangeData) -> Msg;
    fn set_watch_tx(&mut self, tx: watch::Sender<Self::ChangeData>);
    fn empty_change_data() -> Self::ChangeData;
    fn initialize(&mut self, event_tx: Sender<Msg>) {
        if !self.is_initialized() {
            self.set_initialized();
            let (tx, mut rx) = tokio::sync::watch::channel(Self::empty_change_data());
            self.set_watch_tx(tx);

            tokio::task::spawn(async move {
                let duration = tokio::time::Duration::from_millis(100);
                let mut interval = tokio::time::interval(duration);
                loop {
                    interval.tick().await;
                    if rx.has_changed().unwrap_or_default() {
                        let data: Self::ChangeData = rx.borrow_and_update().clone();
                        let _ = event_tx.try_send(Self::watch_change_fn(data));
                    }
                }
            });
        }
    }
}
#[macro_export]
macro_rules! impl_initialize {
    ($name:ident, $field:ident) => {
        impl $crate::mini_window::Initialize for $name {
            fn set_initialized(&mut self) {
                self.$field = true;
            }
            fn is_initialized(&self) -> bool {
                self.$field
            }
        }
    };
}
#[macro_export]
macro_rules! impl_initialize_tx {
    ($name:ident, $field:ident, on_change: $closure:expr, data: $data:tt, empty: $empty:tt) => {
        impl $crate::mini_window::InitializeWatchTx for $name {
            type ChangeData = $data;
            fn set_watch_tx(&mut self, tx: tokio::sync::watch::Sender<Self::ChangeData>) {
                self.$field = Some(tx);
            }
            fn empty_change_data() -> Self::ChangeData {
                $empty
            }
            fn watch_change_fn(data: Self::ChangeData) -> Msg {
                let closure = $closure;
                closure(data)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_indexable {
    ($name:ident) => {
        impl $crate::mini_window::Indexable for $name {
            fn set_index(&mut self, value: usize) {
                self.index = value;
            }
            fn get_index(&self) -> usize {
                self.index
            }
        }
    };
}
#[macro_export]
macro_rules! impl_id {
    ($name:ident, $field:ident) => {
        impl $crate::mini_window::Id for $name {
            fn get_id(&self) -> egui::Id {
                self.$field
            }
        }
    };
}
#[macro_export]
macro_rules! impl_target {
    ($name:ident, $field:ident) => {
        impl $crate::mini_window::Target for $name {
            fn get_target(&self) -> egui::Id {
                self.$field
            }
            fn set_target(&mut self, value: egui::Id) {
                self.$field = value
            }
        }
    };
}

#[macro_export]
macro_rules! impl_generated_content {
    ($name:ident, $field:ident) => {
        impl $crate::mini_window::GeneratedContent for $name {
            fn get_generated_content(&self) -> String {
                self.$field.clone()
            }
            fn set_generated_content(&mut self, value: String) {
                self.$field = value;
            }
        }
    };
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(tag = "type", content = "fields")]
pub enum Window {
    PikchrEditor(pikchr_editor::PikchrEditor),
    SvgbobEditor(svgbob_editor::SvgbobEditor),
    PrologEditor(prolog_editor::PrologEditor),
    TclEditor(tcl_editor::TclEditor),
    MrubyEditor(mruby_editor::MrubyEditor),
    PlainTextEditor(plain_text_editor::PlainTextEditor),
    SvgWindow(svg::SvgWindow),
    HelpWindow(HelpWindow),
}
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum WindowType {
    PikchrEditor,
    SvgbobEditor,
    PrologEditor,
    TclEditor,
    MrubyEditor,
    PlainTextEditor,
    SvgWindow,
    HelpWindow,
}

#[macro_export]
macro_rules! setter_getter_for_trait {
		{($infield:ident => $intype:ty | $outfield:ident $(.$outmethod:ident ())?=> $outtype:ty ) for $struct:ty as $name:ident for $trait:ty} => {
    		paste::paste! {
        		impl $trait for $struct {
            		fn [<get_ $name>](&self) -> $outtype{
                		self.$outfield $(.$outmethod())?
            		}
            		fn [<set_ $name>](&mut self, value: $intype) {
                		self.$infield = value;
            		}
        		}
    		}
		}
}

macro_rules! trait_getter {
    (
        $tr:ty, $name:ident,
        $([$( $some_variant:ident $(,)? ),*] $(,)?)?
    ) => {
        paste::paste! {
            pub fn $name(&self) -> Option<&dyn $tr> {
                match self {
                    $($( Self::$some_variant(e) =>  Some(e as &dyn $tr),  )*)?
                    #[allow(unreachable_patterns)]
                    _ => None
                }
            }
            pub fn [<$name _mut>](&mut self) -> Option<&mut dyn $tr> {
                match self {
                    $($( Self::$some_variant(e) =>  Some(e as &mut dyn $tr),  )*)?
                    #[allow(unreachable_patterns)]
                    _ => None
                }
            }
        }
    };
    (
        view $view:ty, $name:ident, $fun:ident,
        $([$( $some_variant:ident $(,)? ),*] $(,)?)?
    ) => {
        paste::paste! {
            pub fn $name(&self) -> Option<$view> {
                match self {
                    $($( Self::$some_variant(e) =>  Some(e.$fun()),  )*)?
                    #[allow(unreachable_patterns)]
                    _ => None
                }
            }
        }
    };
    (
        mut_view $view:ty, $name:ident, $fun:ident,
        $([$( $some_variant:ident $(,)? ),*] $(,)?)?
    ) => {
        paste::paste! {
            pub fn $name(&mut self) -> Option<$view> {
                match self {
                    $($( Self::$some_variant(e) =>  Some(e.$fun()),  )*)?
                    #[allow(unreachable_patterns)]
                    _ => None
                }
            }
        }
    };
}

impl Window {
    trait_getter!(
        RawContent,
        as_raw_content,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor,
            PlainTextEditor
        ],
    );
    trait_getter!(
        Target,
        as_target,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor
        ],
    );
    trait_getter!(
        Id,
        as_id,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor,
            PlainTextEditor,
            SvgWindow
        ]
    );
    trait_getter!(
        Indexable,
        as_indexable,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor,
            PlainTextEditor,
            SvgWindow
        ]
    );
    trait_getter!(
        Initialize,
        as_initialize,
        [PikchrEditor, SvgbobEditor, SvgWindow],
    );
    trait_getter!(
        MiniWindow,
        as_mini_window,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor,
            PlainTextEditor,
            SvgWindow,
            HelpWindow
        ]
    );
    trait_getter!(
        EditorType,
        as_editor_type,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor,
            PlainTextEditor
        ],
    );
    trait_getter!(
        RenderToggle,
        as_render_toggle,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor,
            SvgWindow
        ],
    );
    trait_getter!(
        view EditorWindowView<'_>, as_editor_window, get_editor_window,
        [PikchrEditor,SvgbobEditor,PrologEditor, TclEditor,MrubyEditor],
    );
    trait_getter!(
        mut_view svg::SvgWindowView<'_>, as_svg_window, get_svg_window_mut,
        [SvgWindow],
    );
    trait_getter!(
        view WindowView<'_>, as_window, get_window,
        [SvgWindow,PikchrEditor,SvgbobEditor,PrologEditor, TclEditor,MrubyEditor,PlainTextEditor,HelpWindow],
    );
    trait_getter!(
        HasError,
        as_error,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor,
            PlainTextEditor
        ],
    );
    trait_getter!(
        HasName,
        as_name,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor,
            PlainTextEditor,
            SvgWindow
        ],
    );
    trait_getter!(
        GeneratedContent,
        as_generated_content,
        [
            PikchrEditor,
            SvgbobEditor,
            PrologEditor,
            TclEditor,
            MrubyEditor
        ],
    );
}

pub trait SvgWindow {
    fn get_svg_window_mut(&mut self) -> svg::SvgWindowView<'_>;
}

pub trait NormalWindow {
    fn get_window(&self) -> WindowView<'_>;
}

pub trait EditorWindow {
    fn get_editor_window(&self) -> EditorWindowView<'_>;
}

impl<T> NormalWindow for T
where
    T: EditorWindow,
{
    fn get_window(&self) -> WindowView<'_> {
        let value = self.get_editor_window();
        WindowView {
            index: value.index,
            id: value.id,
            mini_window: value.mini_window,
        }
    }
}

pub struct WindowView<'a> {
    pub index: &'a usize,
    pub id: &'a egui::Id,
    pub mini_window: &'a dyn MiniWindow,
}
pub struct EditorWindowView<'a> {
    pub index: &'a usize,
    pub id: &'a egui::Id,
    pub content: &'a dyn GeneratedContent,
    pub editor_type: &'a dyn EditorType,
    pub name: &'a str,
    pub mini_window: &'a dyn MiniWindow,
}
