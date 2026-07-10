use eframe::egui;
use tokio::sync::mpsc::Sender;

use crate::Msg;

use super::HelpTopic;

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(text)
            .monospace()
            .size(18.0)
            .color(ui.visuals().hyperlink_color),
    );
}

fn feature(ui: &mut egui::Ui, name: &str, description: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(name)
                .monospace()
                .color(ui.visuals().hyperlink_color),
        );
        ui.label(description);
    });
}

/// A hyperlink-styled, keyboard-focusable label that opens the Pikchr Grammar
/// reference in its own help window.
fn grammar_link(ui: &mut egui::Ui, tx: &Sender<Msg>) {
    let accent = ui.visuals().hyperlink_color;
    let resp = ui.add(
        egui::Label::new(
            egui::RichText::new("Open Pikchr Grammar reference \u{2192}")
                .monospace()
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

fn common_editor_help(ui: &mut egui::Ui) {
    heading(ui, "Editing");
    feature(
        ui,
        "Live updates",
        "The render and dependent windows update automatically after you edit source.",
    );
    feature(
        ui,
        "Output Type",
        "Choose Pikchr or Svgbob per diagram editor. Generated references only compose matching output types.",
    );
    feature(
        ui,
        "Cmd/Ctrl+R",
        "Renames the focused editor. Names are used by cross-window references.",
    );
    feature(
        ui,
        "Enter",
        "Inserts a newline and automatically carries or adjusts indentation.",
    );
    feature(
        ui,
        "Close button",
        "Hides the window. Reopen it from Windows in the main menu.",
    );
    feature(
        ui,
        "Cmd/Ctrl + close button",
        "Permanently deletes the editor and its render window from the workspace.",
    );

    heading(ui, "Cross-window references");
    feature(
        ui,
        "!!NAME!!",
        "Includes the raw source text of another named editor. Plain text windows can be included this way.",
    );
    feature(
        ui,
        "$$NAME$$",
        "Includes generated source from another named diagram editor when both editors use the same Output Type.",
    );
    ui.label("References can be nested up to three replacement passes.");
}

fn topic_help(ui: &mut egui::Ui, topic: HelpTopic, tx: &Sender<Msg>) {
    match topic {
        HelpTopic::Overview | HelpTopic::Grammar => {},
        HelpTopic::Pikchr => {
            common_editor_help(ui);
            heading(ui, "Pikchr");
            ui.label(
                "Write Pikchr directly. Valid source is rendered live in the paired Render window.",
            );
            ui.add_space(4.0);
            grammar_link(ui, tx);
        },
        HelpTopic::Svgbob => {
            common_editor_help(ui);
            heading(ui, "Svgbob");
            ui.label(
                "Write ASCII art directly. Svgbob output is rendered live in the paired Render window.",
            );
            ui.label(
                "This dedicated editor uses a block cursor and canvas-style arrows: Right and Down can extend ragged lines or add rows. Its Mode menu offers Insert (the default) and Replace; Replace currently retains Insert behavior while dedicated replacement bindings are developed.",
            );
            ui.label(
                "It also keeps its own bindings: Enter does not carry indentation, and Tab/Cmd-Ctrl-R are available for Svgbob-specific bindings.",
            );
        },
        HelpTopic::Prolog => {
            common_editor_help(ui);
            heading(ui, "Prolog");
            ui.label("Define a diagram//0 DCG. Its text output is interpreted using the selected Output Type.");
        },
        HelpTopic::Tcl => {
            common_editor_help(ui);
            heading(ui, "Tcl");
            ui.label("Return source for the selected Output Type. The Tcl editor is available only when Tcl 8.6 can be loaded.");
        },
        HelpTopic::Mruby => {
            common_editor_help(ui);
            heading(ui, "Ruby");
            ui.label("Text written with print or puts becomes source for the selected Output Type. The editor is available only when Ruby support is available.");
        },
        HelpTopic::PlainText => {
            common_editor_help(ui);
            heading(ui, "Plain text");
            ui.label("Stores reusable raw text. Include it from another editor with !!NAME!!; it has no generated Pikchr output or Render window.");
        },
        HelpTopic::Render => {
            heading(ui, "Render window");
            feature(
                ui,
                "Automatic preview",
                "Displays the paired editor's generated output and redraws as the window is resized.",
            );
            feature(
                ui,
                "Export",
                "Exports SVG, PNG, transparent PNG, or copies generated Pikchr or Svgbob source to the clipboard.",
            );
            feature(
                ui,
                "Close button",
                "Hides the preview. Reopen it from Windows in the main menu.",
            );
            feature(
                ui,
                "Cmd/Ctrl + close button",
                "Permanently deletes only this Render window. It is recreated when its editor next renders.",
            );
        },
    }
}

/// The User Guide body: a context-specific section (if any) followed by the
/// full feature guide.
pub(super) fn render_guide(ui: &mut egui::Ui, topic: HelpTopic, tx: &Sender<Msg>) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if topic != HelpTopic::Overview {
                topic_help(ui, topic, tx);
                ui.separator();
                heading(ui, "Full feature guide");
            } else {
                grammar_link(ui, tx);
                ui.add_space(4.0);
            }

            heading(ui, "Workspace");
            feature(
                ui,
                "Autosave",
                "The current workspace and window layout persist between app launches.",
            );
            feature(
                ui,
                "Save / Load Workspace",
                "Exports or imports the complete workspace as JSON.",
            );
            feature(
                ui,
                "Reset Workspace",
                "Deletes all workspace windows after confirmation.",
            );
            feature(
                ui,
                "Windows",
                "Shows or hides existing windows, plus the diagnostic Logger and Debug windows.",
            );
            feature(ui, "View", "Changes the scale of the complete interface.");

            common_editor_help(ui);

            heading(ui, "Editor types");
            feature(ui, "Pikchr", "Direct source editor with Pikchr or Svgbob output.");
            feature(ui, "Svgbob", "Dedicated ASCII-art editor with Svgbob output and independent bindings.");
            feature(ui, "Prolog", "A diagram//0 DCG generates source for the selected output type.");
            feature(
                ui,
                "Tcl",
                "A Tcl script returns source for the selected output type when Tcl 8.6 is available.",
            );
            feature(
                ui,
                "Ruby",
                "print and puts output becomes source for the selected output type when Ruby support is available.",
            );
            feature(
                ui,
                "Plain text",
                "Reusable raw text for !!NAME!! references; no paired render.",
            );

            heading(ui, "Rendering and export");
            feature(
                ui,
                "Live Render window",
                "Diagram editors own a paired, resizable preview window.",
            );
            feature(
                ui,
                "Export",
                "Render windows export SVG, PNG, transparent PNG, and generated source for their selected output type.",
            );
            feature(
                ui,
                "Errors",
                "Evaluation and rendering errors appear next to their editor and in the Logger.",
            );
        });
}
