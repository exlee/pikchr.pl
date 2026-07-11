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

mod contracts;
pub use contracts::*;
#[cfg(test)]
use contracts::forward_watch_changes;

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
#[path = "mini_window/tests.rs"]
mod tests;
