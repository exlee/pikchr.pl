use eframe::egui::{self, Ui};

#[derive(Clone, Copy)]
pub(crate) enum AppIcon {
    Render,
    PikchrOutput,
    SvgbobOutput,
    Export,
    Save,
    Help,
    #[allow(unused)]
    Delete,
    Rename,
}

#[derive(Clone, Copy)]
pub(crate) enum CustomIcon {
    ActiveDot(bool),
    Rename,
    Duplicate,
    Delete,
}

pub(crate) fn custom_icon(
    ui: &mut Ui,
    icon: CustomIcon,
    color: Option<egui::Color32>,
) -> egui::Response {
    let size = match icon {
        CustomIcon::ActiveDot(_) => egui::vec2(10.0, 18.0),
        _ => egui::vec2(18.0, 18.0),
    };
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let painter = ui.painter_at(rect);
        let stroke = egui::Stroke::new(1.35, color.unwrap_or(visuals.fg_stroke.color));
        let center = rect.center();

        match icon {
            CustomIcon::ActiveDot(active) => {
                let dot_color = if active {
                    ui.visuals().selection.stroke.color
                } else {
                    ui.visuals().weak_text_color()
                };
                if active {
                    painter.circle_filled(center, 3.6, dot_color);
                } else {
                    painter.circle_stroke(center, 3.2, egui::Stroke::new(1.1, dot_color));
                }
            },
            CustomIcon::Rename => {
                painter.rect_filled(rect, 3.0, visuals.bg_fill);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    "Aa",
                    egui::FontId::proportional(10.5),
                    stroke.color,
                );
            },
            CustomIcon::Duplicate => {
                painter.rect_filled(rect, 3.0, visuals.bg_fill);
                let back = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 4.0, rect.top() + 4.0),
                    egui::vec2(8.0, 8.0),
                );
                let front = back.translate(egui::vec2(3.5, 3.5));
                painter.rect_stroke(back, 1.5, stroke, egui::StrokeKind::Inside);
                painter.rect_filled(front, 1.5, visuals.bg_fill);
                painter.rect_stroke(front, 1.5, stroke, egui::StrokeKind::Inside);
            },
            CustomIcon::Delete => {
                painter.rect_filled(rect, 3.0, visuals.bg_fill);
                let inset = 5.0;
                painter.line_segment(
                    [
                        egui::pos2(rect.left() + inset, rect.top() + inset),
                        egui::pos2(rect.right() - inset, rect.bottom() - inset),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(rect.right() - inset, rect.top() + inset),
                        egui::pos2(rect.left() + inset, rect.bottom() - inset),
                    ],
                    stroke,
                );
            },
        }
    }

    response
}

pub(crate) fn icon_image(icon: AppIcon, tint: egui::Color32) -> egui::Image<'static> {
    egui::Image::new(match icon {
        AppIcon::Render => egui::include_image!("../assets/icons/photo.svg"),
        AppIcon::PikchrOutput => egui::include_image!("../assets/icons/circle-letter-p.svg"),
        AppIcon::SvgbobOutput => egui::include_image!("../assets/icons/circle-letter-b.svg"),
        AppIcon::Delete => egui::include_image!("../assets/icons/trash.svg"),
        AppIcon::Export => egui::include_image!("../assets/icons/file-arrow-right.svg"),
        AppIcon::Save => egui::include_image!("../assets/icons/package-export.svg"),
        AppIcon::Help => egui::include_image!("../assets/icons/help-circle.svg"),
        AppIcon::Rename => egui::include_image!("../assets/icons/writing.svg"),
    })
    .fit_to_exact_size(egui::vec2(14.0, 14.0))
    .tint(tint)
    .alt_text(icon.alt_text())
}

pub(crate) fn icon_button(ui: &mut Ui, icon: AppIcon) -> egui::Response {
    selectable_icon_button(ui, icon, false)
}

pub(crate) fn selectable_icon_button(ui: &mut Ui, icon: AppIcon, selected: bool) -> egui::Response {
    ui.add(
        egui::Button::image(icon_image(icon, ui.visuals().text_color()))
            .min_size(egui::vec2(20.0, 20.0))
            .selected(selected),
    )
}

impl AppIcon {
    fn alt_text(self) -> &'static str {
        match self {
            AppIcon::Render => "Render",
            AppIcon::PikchrOutput => "Pikchr output",
            AppIcon::SvgbobOutput => "Svgbob output",
            AppIcon::Export => "Export",
            AppIcon::Save => "Save",
            AppIcon::Help => "Help",
            AppIcon::Delete => "Delete",
            AppIcon::Rename => "Rename",
        }
    }
}
