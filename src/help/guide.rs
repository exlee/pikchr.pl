use eframe::egui;
use tokio::sync::mpsc::Sender;

use crate::Msg;

use super::HelpTopic;

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(14.0);
    ui.label(egui::RichText::new(text).size(19.0).strong());
    ui.add_space(3.0);
}

fn feature(ui: &mut egui::Ui, name: &str, description: &str) {
    let term_width = (ui.available_width() * 0.28).clamp(112.0, 160.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(term_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(term_width);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(name)
                            .monospace()
                            .strong()
                            .color(ui.visuals().hyperlink_color),
                    )
                    .wrap(),
                );
            },
        );
        ui.add(egui::Label::new(description).wrap());
    });
    ui.add_space(5.0);
}

fn code_example(ui: &mut egui::Ui, title: &str, code: &str) {
    ui.label(egui::RichText::new(title).small().strong());
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(4.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(code).monospace());
        });
    ui.add_space(6.0);
}

/// A hyperlink-styled, keyboard-focusable label that opens the Pikchr Grammar
/// reference in its own help window.
fn grammar_link(ui: &mut egui::Ui, tx: &Sender<Msg>) {
    let accent = ui.visuals().hyperlink_color;
    let resp = ui.add(
        egui::Label::new(
            egui::RichText::new("Open Pikchr Grammar reference")
                .color(accent)
                .underline(),
        )
        .selectable(false)
        .sense(egui::Sense::click()),
    );
    if resp.clicked() {
        let _ = tx.try_send(Msg::ShowHelp(HelpTopic::Grammar));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand);
}

fn common_editor_help(
    ui: &mut egui::Ui,
    has_output_selector: bool,
    show_default_enter: bool,
    has_render_window: bool,
) {
    heading(ui, "Editing");
    feature(
        ui,
        "Live updates",
        "Renders and dependent windows refresh as you edit.",
    );
    if has_output_selector {
        feature(
            ui,
            "Output type",
            "Choose Pikchr or Svgbob for each diagram editor. Generated references must use the same type.",
        );
    }
    feature(
        ui,
        "Cmd/Ctrl+R",
        "Rename the focused editor. References use editor names.",
    );
    if show_default_enter {
        feature(ui, "Enter", "Insert a newline and adjust indentation.");
    }
    feature(
        ui,
        "Close",
        "Hide the window. Reopen it from the Windows menu.",
    );
    let delete_description = if has_render_window {
        "Delete the editor and its Render window from the workspace."
    } else {
        "Delete the editor from the workspace."
    };
    feature(ui, "Cmd/Ctrl+Close", delete_description);
}

fn reference_help(ui: &mut egui::Ui) {
    heading(ui, "Cross-window references");
    feature(
        ui,
        "!!NAME!!",
        "Insert the raw source of another named editor. This also works with plain-text windows.",
    );
    feature(
        ui,
        "$$NAME$$",
        "Insert generated source from a named diagram editor with the same output type.",
    );
    feature(
        ui,
        "X = NAME",
        "At the top of inserted Svgbob source, map marker X to a named editor. Its output overlays every X column by column, without adding lines to the canvas.",
    );

    ui.add_space(3.0);
    code_example(ui, "EDIT", "9 = 3320\nAAA  9\nAAA\nAAA");
    code_example(ui, "3320", "ZZ\nZZ");
    code_example(ui, "Result", "AAA  ZZ\nAAA  ZZ\nAAA");
    ui.label(
        egui::RichText::new("References may be nested through three replacement passes.").small(),
    );
}

fn topic_help(ui: &mut egui::Ui, topic: HelpTopic, tx: &Sender<Msg>) {
    match topic {
        HelpTopic::Overview | HelpTopic::Grammar => {},
        HelpTopic::Pikchr => {
            heading(ui, "Pikchr editor");
            ui.label("Write Pikchr source and preview it live in the paired Render window.");
            ui.add_space(8.0);
            grammar_link(ui, tx);
            common_editor_help(ui, true, true, true);
            reference_help(ui);
        },
        HelpTopic::Svgbob => {
            heading(ui, "Svgbob editor");
            ui.label(
                "Draw diagrams as ASCII art and preview them live in the paired Render window.",
            );

            heading(ui, "Canvas editing");
            feature(
                ui,
                "Arrow keys",
                "Move the block cursor. Right and Down extend the canvas when needed.",
            );
            feature(
                ui,
                "Insert mode",
                "Insert text at the cursor. This is the default mode.",
            );
            feature(
                ui,
                "Replace mode",
                "Overwrite a cell, then continue in the direction established by the two latest adjacent inputs.",
            );
            feature(
                ui,
                "Enter",
                "Add a canvas row without carrying indentation.",
            );
            feature(ui, "Tab", "Switch between Insert and Replace modes.");

            common_editor_help(ui, false, false, true);
            reference_help(ui);
        },
        HelpTopic::Prolog => {
            heading(ui, "Prolog editor");
            ui.label("Define a diagram//0 DCG. Its text output is rendered using the selected output type.");
            common_editor_help(ui, true, true, true);
            reference_help(ui);
        },
        HelpTopic::Tcl => {
            heading(ui, "Tcl editor");
            ui.label("Return diagram source from a Tcl script. This editor is available when Tcl 8.6 can be loaded.");
            common_editor_help(ui, true, true, true);
            reference_help(ui);
        },
        HelpTopic::Mruby => {
            heading(ui, "Ruby editor");
            ui.label("Use print or puts to produce diagram source. This editor is available when Ruby support is enabled.");
            common_editor_help(ui, true, true, true);
            reference_help(ui);
        },
        HelpTopic::PlainText => {
            heading(ui, "Plain-text editor");
            ui.label(
                "Store reusable text for !!NAME!! references. Plain-text windows have no renderer.",
            );
            common_editor_help(ui, false, true, false);
            reference_help(ui);
        },
        HelpTopic::Render => {
            heading(ui, "Render window");
            ui.label("Preview and export the output of its paired diagram editor.");
            heading(ui, "Preview and export");
            feature(
                ui,
                "Live preview",
                "Refreshes after edits and redraws when resized.",
            );
            feature(
                ui,
                "Export",
                "Save SVG, PNG, or transparent PNG; or copy generated source.",
            );
            feature(
                ui,
                "Close",
                "Hide the preview. Reopen it from the Windows menu.",
            );
            feature(
                ui,
                "Cmd/Ctrl+Close",
                "Delete this Render window. It returns when the editor renders again.",
            );
        },
    }
}

fn overview(ui: &mut egui::Ui, tx: &Sender<Msg>) {
    ui.label("A quick reference for workspaces, editors, references, and export.");
    ui.add_space(8.0);
    grammar_link(ui, tx);

    heading(ui, "Workspace");
    feature(
        ui,
        "Autosave",
        "The workspace and window layout persist between launches.",
    );
    feature(
        ui,
        "Save / Load",
        "Export or import the complete workspace as JSON.",
    );
    feature(
        ui,
        "Reset",
        "Delete every workspace window after confirmation.",
    );
    feature(
        ui,
        "Windows",
        "Show or hide workspace, Logger, and Debug windows.",
    );
    feature(ui, "View", "Change the scale of the whole interface.");

    common_editor_help(ui, true, true, true);
    reference_help(ui);

    heading(ui, "Editor types");
    feature(
        ui,
        "Pikchr",
        "Direct diagram source with Pikchr or Svgbob output.",
    );
    feature(
        ui,
        "Svgbob",
        "ASCII-art canvas with dedicated navigation and editing modes.",
    );
    feature(ui, "Prolog", "A diagram//0 DCG produces diagram source.");
    feature(
        ui,
        "Tcl",
        "A Tcl script returns diagram source when Tcl 8.6 is available.",
    );
    feature(
        ui,
        "Ruby",
        "print and puts produce diagram source when Ruby is available.",
    );
    feature(
        ui,
        "Plain text",
        "Reusable raw text with no paired Render window.",
    );

    heading(ui, "Rendering and export");
    feature(
        ui,
        "Render window",
        "A resizable live preview paired with each diagram editor.",
    );
    feature(
        ui,
        "Export",
        "Save SVG, PNG, transparent PNG, or generated source.",
    );
    feature(
        ui,
        "Errors",
        "See evaluation and rendering errors beside the editor and in Logger.",
    );
}

pub(super) fn render_guide(ui: &mut egui::Ui, topic: HelpTopic, tx: &Sender<Msg>) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(18, 12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if topic == HelpTopic::Overview {
                        overview(ui, tx);
                    } else {
                        topic_help(ui, topic, tx);
                    }
                    ui.add_space(12.0);
                });
        });
}
