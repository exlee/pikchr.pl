use eframe::egui::{self, Context, MenuBar, Ui};
use tokio::sync::{mpsc::Sender, watch};

use crate::{
    Msg,
    help::{HelpTopic, HelpWindow},
    icons::{AppIcon, icon_button, selectable_icon_button},
    mruby_editor, pikchr_editor, plain_text_editor, prolog_editor,
    state::DiagramBackground,
    svg, svgbob_editor, tcl_editor,
};

const WINDOW_ZOOM_STEP: f32 = 0.1;
const MIN_WINDOW_ZOOM: f32 = 0.5;
const MAX_WINDOW_ZOOM: f32 = 3.0;

fn window_zoom_id(window_id: egui::Id) -> egui::Id {
    window_id.with("window_zoom")
}

pub(crate) fn window_zoom_factor(ctx: &Context, window_id: egui::Id) -> f32 {
    ctx.data_mut(|data| {
        data.get_persisted::<f32>(window_zoom_id(window_id))
            .unwrap_or(1.0)
    })
}

fn set_window_zoom_factor(ctx: &Context, window_id: egui::Id, zoom: f32) {
    ctx.data_mut(|data| {
        data.insert_persisted(
            window_zoom_id(window_id),
            zoom.clamp(MIN_WINDOW_ZOOM, MAX_WINDOW_ZOOM),
        );
    });
    ctx.request_repaint();
}

fn apply_window_zoom(style: &mut egui::Style, zoom: f32) {
    for font_id in style.text_styles.values_mut() {
        font_id.size *= zoom;
    }
    style.spacing.item_spacing *= zoom;
    style.spacing.window_margin *= zoom;
    style.spacing.menu_margin *= zoom;
    style.spacing.button_padding *= zoom;
    style.spacing.interact_size *= zoom;
    style.spacing.icon_width *= zoom;
    style.spacing.icon_width_inner *= zoom;
    style.spacing.icon_spacing *= zoom;
    style.spacing.indent *= zoom;
    style.spacing.scroll.bar_width *= zoom;
    style.spacing.scroll.handle_min_length *= zoom;
    style.spacing.scroll.floating_width *= zoom;
    style.visuals.resize_corner_size *= zoom;
}

fn zoom_icon_button(ui: &mut Ui, icon: AppIcon, enabled: bool) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| icon_button(ui, icon)).inner
}

fn window_zoom_controls(ui: &mut Ui, ctx: &Context, window_id: egui::Id) {
    let zoom = window_zoom_factor(ctx, window_id);
    let zoom_out = zoom_icon_button(ui, AppIcon::ZoomOut, zoom > MIN_WINDOW_ZOOM).on_hover_text(
        format!("Zoom out this window\nCurrent: {:.0}%", zoom * 100.0),
    );
    if zoom_out.clicked() {
        set_window_zoom_factor(ctx, window_id, zoom - WINDOW_ZOOM_STEP);
    }

    let reset = zoom_icon_button(ui, AppIcon::ZoomReset, (zoom - 1.0).abs() > f32::EPSILON)
        .on_hover_text("Reset this window to the workspace zoom");
    if reset.clicked() {
        set_window_zoom_factor(ctx, window_id, 1.0);
    }

    let zoom_in = zoom_icon_button(ui, AppIcon::ZoomIn, zoom < MAX_WINDOW_ZOOM).on_hover_text(
        format!("Zoom In this window\nCurrent: {:.0}%", zoom * 100.0),
    );
    if zoom_in.clicked() {
        set_window_zoom_factor(ctx, window_id, zoom + WINDOW_ZOOM_STEP);
    }
}

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

    fn close_requested(&mut self, _ctx: &Context, command_only: bool, tx: Sender<Msg>) {
        if command_only {
            let _ = tx.try_send(Msg::DeleteWindow(self.get_id()));
        }
    }

    fn show(&mut self, ctx: &Context, tx: Sender<Msg>, background: DiagramBackground) {
        if !self.should_show() {
            return;
        };
        let zoom = window_zoom_factor(ctx, self.get_id());
        let workspace_style = ctx.style();
        let mut window_style = (*workspace_style).clone();
        apply_window_zoom(&mut window_style, zoom);
        ctx.set_style(window_style);

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
                        ui.add_space(8.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(76.0 * zoom, ui.spacing().interact_size.y),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| window_zoom_controls(ui, ctx, self.get_id()),
                        );
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
                                    let (icon, next_output_type) = match output_type {
                                        crate::OutputType::Pikchr => {
                                            (AppIcon::PikchrOutput, crate::OutputType::Svgbob)
                                        },
                                        crate::OutputType::Svgbob => {
                                            (AppIcon::SvgbobOutput, crate::OutputType::Pikchr)
                                        },
                                    };
                                    if icon_button(ui, icon)
                                        .on_hover_text(format!(
                                            "{} output\nSwitch to {}",
                                            output_type.label(),
                                            next_output_type.label()
                                        ))
                                        .clicked()
                                    {
                                        self.set_output_type(next_output_type);
                                        let _ = tx.try_send(Msg::SetOutputType(
                                            ctx.clone(),
                                            self.get_id(),
                                            next_output_type,
                                        ));
                                    }
                                }
                                let render = self.render_enabled();
                                if selectable_icon_button(ui, AppIcon::Render, render)
                                    .on_hover_text("Render diagram\n(unselect for include-only)")
                                    .clicked()
                                {
                                    let _ = tx.try_send(Msg::SetRenderEnabled(
                                        ctx.clone(),
                                        self.get_id(),
                                        !render,
                                    ));
                                }
                            }
                            if self.can_save_to_library()
                                && icon_button(ui, AppIcon::Rename)
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
                self.inner_window(ctx, ui, tx.clone(), background);
            });
        });
        ctx.set_style(workspace_style);
        if self.visible() && !visible {
            let command_only = ctx.input(|i| i.modifiers.command_only());
            self.close_requested(ctx, command_only, tx.clone());
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
            let (tx, rx) = tokio::sync::watch::channel(Self::empty_change_data());
            self.set_watch_tx(tx);

            tokio::task::spawn(forward_watch_changes(rx, event_tx, Self::watch_change_fn));
        }
    }
}

async fn forward_watch_changes<T>(
    mut rx: watch::Receiver<T>,
    event_tx: Sender<Msg>,
    watch_change_fn: fn(T) -> Msg,
) where
    T: Clone + Send + Sync + 'static,
{
    let duration = tokio::time::Duration::from_millis(100);
    let mut interval = tokio::time::interval(duration);
    loop {
        interval.tick().await;
        match rx.has_changed() {
            Ok(true) => {
                let data = rx.borrow_and_update().clone();
                if event_tx.send(watch_change_fn(data)).await.is_err() {
                    break;
                }
            },
            Ok(false) => {},
            Err(_) => break,
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

#[cfg(test)]
mod tests {
    use super::*;

    struct ZoomWindow {
        id: egui::Id,
        visible: bool,
        content: String,
    }

    impl Visible for ZoomWindow {
        fn visible(&self) -> bool {
            self.visible
        }

        fn set_visible(&mut self, visible: bool) {
            self.visible = visible;
        }
    }

    impl Id for ZoomWindow {
        fn get_id(&self) -> egui::Id {
            self.id
        }
    }

    impl HasMenu for ZoomWindow {}
    impl RenderToggle for ZoomWindow {}

    impl InnerWindow for ZoomWindow {
        fn inner_window(
            &mut self,
            _ctx: &Context,
            ui: &mut Ui,
            _tx: Sender<Msg>,
            _background: DiagramBackground,
        ) {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut self.content),
                    );
                });
        }
    }

    impl MiniWindow for ZoomWindow {
        fn get_title(&self) -> String {
            "Zoom test".to_owned()
        }

        fn help_topic(&self) -> HelpTopic {
            HelpTopic::Overview
        }
    }

    #[test]
    fn window_zoom_is_scoped_by_window_id_and_resets_to_workspace_default() {
        let ctx = Context::default();
        let first = egui::Id::new("first-window");
        let second = egui::Id::new("second-window");

        set_window_zoom_factor(&ctx, first, 1.4);
        assert!((window_zoom_factor(&ctx, first) - 1.4).abs() < f32::EPSILON);
        assert!((window_zoom_factor(&ctx, second) - 1.0).abs() < f32::EPSILON);

        set_window_zoom_factor(&ctx, first, 1.0);
        assert!((window_zoom_factor(&ctx, first) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn window_zoom_is_clamped_to_supported_range() {
        let ctx = Context::default();
        let id = egui::Id::new("window");

        set_window_zoom_factor(&ctx, id, 0.0);
        assert!((window_zoom_factor(&ctx, id) - MIN_WINDOW_ZOOM).abs() < f32::EPSILON);

        set_window_zoom_factor(&ctx, id, 10.0);
        assert!((window_zoom_factor(&ctx, id) - MAX_WINDOW_ZOOM).abs() < f32::EPSILON);
    }

    #[test]
    fn zoom_style_does_not_expand_a_window_to_the_workspace_width() {
        use egui_kittest::Harness;

        let id = egui::Id::new("zoom-window");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let mut window = ZoomWindow {
            id,
            visible: true,
            content: "content".to_owned(),
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1200.0, 800.0))
            .build(move |ctx| {
                window.show(ctx, tx.clone(), DiagramBackground::White);
            });

        harness.run_steps(2);
        let initial = harness
            .ctx
            .memory(|memory| memory.area_rect(id).expect("window should be visible"));
        assert!(
            initial.width() < 600.0,
            "unzoomed window filled workspace: {initial:?}"
        );

        set_window_zoom_factor(&harness.ctx, id, 1.5);
        harness.run_steps(2);
        let rect = harness
            .ctx
            .memory(|memory| memory.area_rect(id).expect("window should be visible"));

        assert!(
            rect.width() < 600.0,
            "zoomed window filled workspace: {rect:?}"
        );
        assert!(
            rect.height() > initial.height(),
            "outer window chrome did not scale: {initial:?} -> {rect:?}"
        );
    }

    #[derive(Default)]
    struct WatchHarness {
        initialized: bool,
    }

    impl Id for WatchHarness {
        fn get_id(&self) -> egui::Id {
            egui::Id::new("watch-harness")
        }
    }

    impl Initialize for WatchHarness {
        fn is_initialized(&self) -> bool {
            self.initialized
        }

        fn set_initialized(&mut self) {
            self.initialized = true;
        }
    }

    impl InitializeWatchTx for WatchHarness {
        type ChangeData = usize;

        fn watch_change_fn(data: Self::ChangeData) -> Msg {
            Msg::UpdateContent(egui::Id::new("watch-harness"), data.to_string())
        }

        fn set_watch_tx(&mut self, _tx: watch::Sender<Self::ChangeData>) {}

        fn empty_change_data() -> Self::ChangeData {
            0
        }
    }

    #[tokio::test]
    async fn watched_change_waits_for_message_queue_capacity() {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1);
        event_tx.send(Msg::CheckDependencies).await.unwrap();
        let (watch_tx, watch_rx) = watch::channel(0);
        watch_tx.send(7).unwrap();

        let forwarder = tokio::spawn(forward_watch_changes(
            watch_rx,
            event_tx,
            WatchHarness::watch_change_fn,
        ));
        tokio::task::yield_now().await;

        assert!(matches!(
            event_rx.recv().await,
            Some(Msg::CheckDependencies)
        ));
        let forwarded = tokio::time::timeout(tokio::time::Duration::from_secs(1), event_rx.recv())
            .await
            .expect("watched update should wait for queue capacity");
        assert!(matches!(
            forwarded,
            Some(Msg::UpdateContent(_, content)) if content == "7"
        ));

        drop(watch_tx);
        tokio::time::timeout(tokio::time::Duration::from_secs(1), forwarder)
            .await
            .expect("forwarder should stop after watch closes")
            .unwrap();
    }
}
