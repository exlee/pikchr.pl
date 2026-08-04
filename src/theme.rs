use std::{
    collections::BTreeMap,
    fs,
    io::BufReader,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::{LazyLock, RwLock},
};

use eframe::egui::{self, Color32};
use serde::Deserialize;
use syntect::highlighting::{
    Color, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSet, ThemeSettings,
};

pub const DEFAULT_THEME_ID: &str = "builtin:shades-of-purple";

const README: &str = "\
Diagramide themes
=================

Place themes in this directory, then choose Themes > Reload Themes in Diagramide.

Supported formats:
- TextMate .tmTheme files
- VS Code color-theme .json files, including JSONC comments and trailing commas

For VS Code themes, Diagramide uses common workbench colors and TextMate tokenColors.
Unsupported properties and semanticTokenColors are ignored.
";

#[derive(Clone)]
pub struct DiagramTheme {
    pub id: String,
    pub name: String,
    pub built_in: bool,
    pub visuals: egui::Visuals,
    pub syntax: Theme,
}

#[derive(Default)]
struct ThemeCatalog {
    themes: BTreeMap<String, DiagramTheme>,
    active_id: String,
}

static CATALOG: LazyLock<RwLock<ThemeCatalog>> = LazyLock::new(|| {
    let mut catalog = ThemeCatalog::default();
    reload_catalog(&mut catalog);
    catalog.active_id = DEFAULT_THEME_ID.to_owned();
    RwLock::new(catalog)
});

pub fn initialize(selected_id: &str, ctx: &egui::Context) -> String {
    let selected = if set_active(selected_id, ctx) {
        selected_id
    } else {
        let _ = set_active(DEFAULT_THEME_ID, ctx);
        DEFAULT_THEME_ID
    };
    selected.to_owned()
}

pub fn list() -> Vec<(String, String, bool)> {
    CATALOG
        .read()
        .expect("theme catalog poisoned")
        .themes
        .values()
        .map(|theme| (theme.id.clone(), theme.name.clone(), theme.built_in))
        .collect()
}

pub fn active_id() -> String {
    CATALOG
        .read()
        .expect("theme catalog poisoned")
        .active_id
        .clone()
}

pub fn active_syntax() -> Theme {
    let catalog = CATALOG.read().expect("theme catalog poisoned");
    catalog
        .themes
        .get(&catalog.active_id)
        .or_else(|| catalog.themes.get(DEFAULT_THEME_ID))
        .expect("default theme missing")
        .syntax
        .clone()
}

pub fn set_active(id: &str, ctx: &egui::Context) -> bool {
    let mut catalog = CATALOG.write().expect("theme catalog poisoned");
    let Some(visuals) = catalog.themes.get(id).map(|theme| theme.visuals.clone()) else {
        return false;
    };
    catalog.active_id = id.to_owned();
    drop(catalog);
    ctx.set_visuals(visuals);
    ctx.request_repaint();
    true
}

pub fn reload(ctx: &egui::Context) -> Vec<String> {
    let active_id = active_id();
    let mut catalog = CATALOG.write().expect("theme catalog poisoned");
    let errors = reload_catalog(&mut catalog);
    drop(catalog);
    if !set_active(&active_id, ctx) {
        let _ = set_active(DEFAULT_THEME_ID, ctx);
    }
    errors
}

pub fn themes_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/diagramide/themes"))
        .ok_or_else(|| "HOME is not set; cannot locate the themes directory".to_owned())
}

pub fn ensure_themes_dir() -> Result<PathBuf, String> {
    let path = themes_dir()?;
    ensure_themes_dir_at(&path)?;
    Ok(path)
}

fn ensure_themes_dir_at(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|err| format!("Could not create {}: {err}", path.display()))?;
    let readme = path.join("README.txt");
    if !readme.exists() {
        fs::write(&readme, README)
            .map_err(|err| format!("Could not write {}: {err}", readme.display()))?;
    }
    Ok(())
}

pub fn open_themes_dir() -> Result<(), String> {
    let path = ensure_themes_dir()?;
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");

    command
        .arg(&path)
        .spawn()
        .map_err(|err| format!("Could not open {}: {err}", path.display()))?;
    Ok(())
}

fn reload_catalog(catalog: &mut ThemeCatalog) -> Vec<String> {
    let active_id = catalog.active_id.clone();
    catalog.themes = built_in_themes()
        .into_iter()
        .map(|theme| (theme.id.clone(), theme))
        .collect();
    let mut errors = Vec::new();
    if let Ok(path) = themes_dir()
        && path.exists()
    {
        match fs::read_dir(&path) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    match load_external_theme(&path) {
                        Ok(Some(theme)) => {
                            catalog.themes.insert(theme.id.clone(), theme);
                        },
                        Ok(None) => {},
                        Err(err) => errors.push(err),
                    }
                }
            },
            Err(err) => errors.push(format!("Could not read {}: {err}", path.display())),
        }
    }
    catalog.active_id = active_id;
    errors
}

fn built_in_themes() -> Vec<DiagramTheme> {
    vec![
        shades_of_purple(),
        catppuccin("latte", "Catppuccin Latte", catppuccin_egui::LATTE),
        catppuccin("frappe", "Catppuccin Frappe", catppuccin_egui::FRAPPE),
        catppuccin(
            "macchiato",
            "Catppuccin Macchiato",
            catppuccin_egui::MACCHIATO,
        ),
        catppuccin("mocha", "Catppuccin Mocha", catppuccin_egui::MOCHA),
    ]
}

fn catppuccin(id: &str, name: &str, palette: catppuccin_egui::Theme) -> DiagramTheme {
    let mut style = egui::Style::default();
    catppuccin_egui::set_style_theme(&mut style, palette);
    let syntax = syntax_theme(
        name,
        palette.text,
        palette.base,
        &[
            ("comment", palette.overlay2, FontStyle::ITALIC),
            ("string", palette.green, FontStyle::empty()),
            ("keyword, storage", palette.mauve, FontStyle::BOLD),
            ("constant", palette.peach, FontStyle::empty()),
            (
                "entity.name.type, support.type",
                palette.yellow,
                FontStyle::empty(),
            ),
            ("entity.name.function", palette.blue, FontStyle::empty()),
            ("variable", palette.text, FontStyle::empty()),
            ("keyword.operator", palette.sky, FontStyle::empty()),
            ("invalid", palette.red, FontStyle::empty()),
        ],
    );
    DiagramTheme {
        id: format!("builtin:catppuccin-{id}"),
        name: name.to_owned(),
        built_in: true,
        visuals: style.visuals,
        syntax,
    }
}

fn shades_of_purple() -> DiagramTheme {
    let bg = hex("#1E1E3F").unwrap();
    let fg = hex("#A599E9").unwrap();
    let accent = hex("#7857FE").unwrap();
    let muted = hex("#504B82").unwrap();
    let bright = hex("#EBE4F6").unwrap();
    let secondary = hex("#322E6E").unwrap();
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(fg);
    visuals.weak_text_color = Some(muted);
    visuals.hyperlink_color = fg;
    visuals.faint_bg_color = secondary;
    visuals.extreme_bg_color = bg;
    visuals.text_edit_bg_color = Some(bg);
    visuals.code_bg_color = bg;
    visuals.warn_fg_color = hex("#FF8C00").unwrap();
    visuals.error_fg_color = hex("#EC3A37").unwrap();
    visuals.window_fill = bg;
    visuals.panel_fill = bg;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, secondary);
    visuals.window_shadow = egui::epaint::Shadow {
        color: Color32::from_black_alpha(48),
        ..visuals.window_shadow
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        color: Color32::from_black_alpha(48),
        ..visuals.popup_shadow
    };
    visuals.selection.bg_fill = accent.linear_multiply(0.55);
    visuals.selection.stroke.color = bright;
    set_widget_colors(&mut visuals.widgets.noninteractive, bg, fg, secondary);
    set_widget_colors(&mut visuals.widgets.inactive, bg, fg, secondary);
    set_widget_colors(&mut visuals.widgets.hovered, secondary, bright, muted);
    set_widget_colors(&mut visuals.widgets.active, accent, bright, accent);
    set_widget_colors(&mut visuals.widgets.open, secondary, bright, muted);

    let syntax = syntax_theme(
        "Shades of Purple",
        fg,
        bg,
        &[
            ("comment", hex("#B362FF").unwrap(), FontStyle::ITALIC),
            ("string", hex("#A5FF90").unwrap(), FontStyle::empty()),
            ("keyword, storage", hex("#FF9D00").unwrap(), FontStyle::BOLD),
            ("constant", hex("#FF628C").unwrap(), FontStyle::empty()),
            (
                "entity.name.type, support.type",
                hex("#FB94FF").unwrap(),
                FontStyle::empty(),
            ),
            ("entity.name.function", bright, FontStyle::empty()),
            ("variable", accent, FontStyle::empty()),
            (
                "keyword.operator",
                hex("#FF6666").unwrap(),
                FontStyle::empty(),
            ),
            (
                "entity.other.attribute-name",
                hex("#3AD900").unwrap(),
                FontStyle::empty(),
            ),
            ("meta, support", hex("#9EFFFF").unwrap(), FontStyle::empty()),
            ("invalid", hex("#EC3A37").unwrap(), FontStyle::empty()),
        ],
    );
    DiagramTheme {
        id: DEFAULT_THEME_ID.to_owned(),
        name: "Shades of Purple".to_owned(),
        built_in: true,
        visuals,
        syntax,
    }
}

fn set_widget_colors(
    widget: &mut egui::style::WidgetVisuals,
    background: Color32,
    foreground: Color32,
    border: Color32,
) {
    widget.bg_fill = background;
    widget.weak_bg_fill = background;
    widget.fg_stroke.color = foreground;
    widget.bg_stroke.color = border;
}

fn syntax_theme(
    name: &str,
    foreground: Color32,
    background: Color32,
    rules: &[(&str, Color32, FontStyle)],
) -> Theme {
    Theme {
        name: Some(name.to_owned()),
        settings: ThemeSettings {
            foreground: Some(to_syntect(foreground)),
            background: Some(to_syntect(background)),
            ..Default::default()
        },
        scopes: rules
            .iter()
            .filter_map(|(scope, color, font_style)| {
                ScopeSelectors::from_str(scope).ok().map(|scope| ThemeItem {
                    scope,
                    style: StyleModifier {
                        foreground: Some(to_syntect(*color)),
                        font_style: Some(*font_style),
                        ..Default::default()
                    },
                })
            })
            .collect(),
        ..Default::default()
    }
}

fn load_external_theme(path: &Path) -> Result<Option<DiagramTheme>, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let (syntax, visuals) = if extension.eq_ignore_ascii_case("tmTheme") {
        let file = fs::File::open(path)
            .map_err(|err| format!("Could not open {}: {err}", path.display()))?;
        let mut reader = BufReader::new(file);
        let syntax = ThemeSet::load_from_reader(&mut reader)
            .map_err(|err| format!("Could not parse {}: {err}", path.display()))?;
        let visuals = visuals_from_syntax(&syntax);
        (syntax, visuals)
    } else if extension.eq_ignore_ascii_case("json") {
        load_vscode_theme(path)?
    } else {
        return Ok(None);
    };
    let id = format!("external:{}", path.to_string_lossy());
    let name = syntax
        .name
        .clone()
        .or_else(|| {
            path.file_stem()
                .map(|value| value.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "Unnamed theme".to_owned());
    Ok(Some(DiagramTheme {
        id,
        name,
        built_in: false,
        visuals,
        syntax,
    }))
}

#[derive(Deserialize)]
struct VscodeTheme {
    name: Option<String>,
    #[serde(rename = "type")]
    theme_type: Option<String>,
    #[serde(default)]
    colors: BTreeMap<String, String>,
    #[serde(rename = "tokenColors", default)]
    token_colors: Vec<VscodeToken>,
}

#[derive(Deserialize)]
struct VscodeToken {
    scope: Option<VscodeScope>,
    #[serde(default)]
    settings: VscodeTokenSettings,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum VscodeScope {
    One(String),
    Many(Vec<String>),
}

#[derive(Default, Deserialize)]
struct VscodeTokenSettings {
    foreground: Option<String>,
    background: Option<String>,
    #[serde(rename = "fontStyle")]
    font_style: Option<String>,
}

fn load_vscode_theme(path: &Path) -> Result<(Theme, egui::Visuals), String> {
    let source = fs::read_to_string(path)
        .map_err(|err| format!("Could not read {}: {err}", path.display()))?;
    let vscode: VscodeTheme = serde_json::from_str(&source)
        .or_else(|_| json5::from_str(&source))
        .map_err(|err| format!("Could not parse {}: {err}", path.display()))?;
    let visuals = visuals_from_vscode(&vscode);
    let foreground = vscode_color(&vscode.colors, &["editor.foreground", "foreground"]);
    let background = vscode_color(&vscode.colors, &["editor.background"]);
    let selection = vscode_color(
        &vscode.colors,
        &["editor.selectionBackground", "selection.background"],
    );
    let mut scopes = Vec::new();
    for token in vscode.token_colors {
        let Some(scope) = token.scope else { continue };
        let scope = match scope {
            VscodeScope::One(scope) => scope,
            VscodeScope::Many(scopes) => scopes.join(","),
        };
        let Ok(scope) = ScopeSelectors::from_str(&scope) else {
            continue;
        };
        scopes.push(ThemeItem {
            scope,
            style: StyleModifier {
                foreground: token
                    .settings
                    .foreground
                    .as_deref()
                    .and_then(hex)
                    .map(to_syntect),
                background: token
                    .settings
                    .background
                    .as_deref()
                    .and_then(hex)
                    .map(to_syntect),
                font_style: token.settings.font_style.as_deref().map(parse_font_style),
            },
        });
    }
    let syntax = Theme {
        name: vscode.name,
        settings: ThemeSettings {
            foreground: foreground.map(to_syntect),
            background: background.map(to_syntect),
            selection: selection.map(to_syntect),
            ..Default::default()
        },
        scopes,
        ..Default::default()
    };
    Ok((syntax, visuals))
}

fn visuals_from_syntax(theme: &Theme) -> egui::Visuals {
    let background = theme
        .settings
        .background
        .map(from_syntect)
        .unwrap_or_else(|| hex("#1E1E3F").unwrap());
    let foreground = theme
        .settings
        .foreground
        .map(from_syntect)
        .unwrap_or_else(|| hex("#EBE4F6").unwrap());
    let dark = luminance(background) < 0.5;
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.override_text_color = Some(foreground);
    visuals.window_fill = background;
    visuals.panel_fill = background;
    visuals.extreme_bg_color = background;
    visuals.text_edit_bg_color = Some(background);
    visuals.code_bg_color = background;
    if let Some(selection) = theme.settings.selection {
        visuals.selection.bg_fill = from_syntect(selection);
    }
    visuals
}

fn visuals_from_vscode(theme: &VscodeTheme) -> egui::Visuals {
    let background = vscode_color(&theme.colors, &["editor.background"])
        .unwrap_or_else(|| hex("#1E1E3F").unwrap());
    let foreground = vscode_color(&theme.colors, &["editor.foreground", "foreground"])
        .unwrap_or_else(|| hex("#EBE4F6").unwrap());
    let dark = theme.theme_type.as_deref() != Some("light") && luminance(background) < 0.5;
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let panel = vscode_color(
        &theme.colors,
        &[
            "panel.background",
            "sideBar.background",
            "editor.background",
        ],
    )
    .unwrap_or(background);
    let window = vscode_color(
        &theme.colors,
        &[
            "editorWidget.background",
            "menu.background",
            "editor.background",
        ],
    )
    .unwrap_or(background);
    let input = vscode_color(&theme.colors, &["input.background", "editor.background"])
        .unwrap_or(background);
    let border = vscode_color(
        &theme.colors,
        &["focusBorder", "widget.border", "panel.border"],
    )
    .unwrap_or(foreground);
    let hover = vscode_color(
        &theme.colors,
        &["list.hoverBackground", "toolbar.hoverBackground"],
    )
    .unwrap_or(panel);
    let active = vscode_color(
        &theme.colors,
        &["list.activeSelectionBackground", "button.background"],
    )
    .unwrap_or(hover);

    visuals.override_text_color = Some(foreground);
    visuals.window_fill = window;
    visuals.panel_fill = panel;
    visuals.faint_bg_color = hover;
    visuals.extreme_bg_color = input;
    visuals.text_edit_bg_color = Some(input);
    visuals.code_bg_color = input;
    visuals.hyperlink_color =
        vscode_color(&theme.colors, &["textLink.foreground"]).unwrap_or(foreground);
    visuals.error_fg_color =
        vscode_color(&theme.colors, &["errorForeground"]).unwrap_or(visuals.error_fg_color);
    visuals.warn_fg_color = vscode_color(
        &theme.colors,
        &[
            "editorWarning.foreground",
            "notificationsWarningIcon.foreground",
        ],
    )
    .unwrap_or(visuals.warn_fg_color);
    visuals.window_stroke.color = border;
    if let Some(selection) = vscode_color(
        &theme.colors,
        &["editor.selectionBackground", "selection.background"],
    ) {
        visuals.selection.bg_fill = selection;
    }
    set_widget_colors(
        &mut visuals.widgets.noninteractive,
        window,
        foreground,
        border,
    );
    set_widget_colors(&mut visuals.widgets.inactive, panel, foreground, border);
    set_widget_colors(&mut visuals.widgets.hovered, hover, foreground, border);
    set_widget_colors(&mut visuals.widgets.active, active, foreground, border);
    set_widget_colors(&mut visuals.widgets.open, active, foreground, border);
    visuals
}

fn vscode_color(colors: &BTreeMap<String, String>, keys: &[&str]) -> Option<Color32> {
    keys.iter()
        .find_map(|key| colors.get(*key).and_then(|value| hex(value)))
}

fn parse_font_style(value: &str) -> FontStyle {
    value
        .split_whitespace()
        .fold(FontStyle::empty(), |style, part| {
            style
                | match part {
                    "bold" => FontStyle::BOLD,
                    "italic" => FontStyle::ITALIC,
                    "underline" => FontStyle::UNDERLINE,
                    _ => FontStyle::empty(),
                }
        })
}

fn hex(value: &str) -> Option<Color32> {
    let value = value.strip_prefix('#')?;
    let expanded;
    let value = match value.len() {
        3 | 4 => {
            expanded = value.chars().flat_map(|ch| [ch, ch]).collect::<String>();
            expanded.as_str()
        },
        6 | 8 => value,
        _ => return None,
    };
    let r = u8::from_str_radix(&value[0..2], 16).ok()?;
    let g = u8::from_str_radix(&value[2..4], 16).ok()?;
    let b = u8::from_str_radix(&value[4..6], 16).ok()?;
    let a = value
        .get(6..8)
        .and_then(|a| u8::from_str_radix(a, 16).ok())
        .unwrap_or(255);
    Some(Color32::from_rgba_unmultiplied(r, g, b, a))
}

fn to_syntect(color: Color32) -> Color {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    Color { r, g, b, a }
}

fn from_syntect(color: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

fn luminance(color: Color32) -> f32 {
    let [r, g, b, _] = color.to_array();
    (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_supported_hex_colors() {
        assert_eq!(hex("#abc").unwrap().to_array(), [0xaa, 0xbb, 0xcc, 0xff]);
        assert_eq!(hex("#11223344").unwrap().a(), 0x44);
    }

    #[test]
    fn built_ins_include_shades_of_purple() {
        let themes = built_in_themes();
        assert!(themes.iter().any(|theme| theme.id == DEFAULT_THEME_ID));
    }

    #[test]
    fn shades_of_purple_uses_the_default_background_for_editors() {
        let theme = shades_of_purple();
        let background = hex("#1E1E3F").unwrap();
        assert_eq!(theme.visuals.panel_fill, background);
        assert_eq!(theme.visuals.window_fill, background);
        assert_eq!(theme.visuals.text_edit_bg_color, Some(background));
        assert_eq!(theme.visuals.widgets.inactive.bg_fill, background);
        assert_ne!(theme.visuals.selection.bg_fill, hex("#FAD000").unwrap());
    }

    #[test]
    fn loads_vscode_workbench_and_token_colors() {
        let path = temp_path("theme.json");
        fs::write(
            &path,
            r##"{
                "name": "Test Purple",
                "type": "dark",
                "colors": {
                    "editor.background": "#112233",
                    "editor.foreground": "#ddeeff",
                    "focusBorder": "#abcdef"
                },
                "tokenColors": [{
                    "scope": ["comment"],
                    "settings": {"foreground": "#998877", "fontStyle": "italic"}
                }]
            }"##,
        )
        .unwrap();
        let theme = load_external_theme(&path).unwrap().unwrap();
        assert_eq!(theme.name, "Test Purple");
        assert_eq!(
            theme.visuals.panel_fill.to_array(),
            [0x11, 0x22, 0x33, 0xff]
        );
        assert_eq!(theme.syntax.scopes.len(), 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn loads_vscode_jsonc_with_trailing_commas() {
        let path = temp_path("theme.json");
        fs::write(
            &path,
            r##"{
                // VS Code color themes commonly use JSONC.
                "name": "JSONC Theme",
                "type": "dark",
                "colors": {
                    "editor.background": "#112233",
                },
                "tokenColors": [{
                    "scope": ["comment",],
                    "settings": {"foreground": "#998877",},
                }],
            }"##,
        )
        .unwrap();
        let theme = load_external_theme(&path).unwrap().unwrap();
        assert_eq!(theme.name, "JSONC Theme");
        assert_eq!(theme.syntax.scopes.len(), 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn loads_installed_synthwave_theme_when_present() {
        let Ok(path) = themes_dir().map(|path| path.join("synthwave-color-theme.json")) else {
            return;
        };
        if !path.exists() {
            return;
        }
        let theme = load_external_theme(&path).unwrap().unwrap();
        assert_eq!(theme.name, "SynthWave 84");
    }

    #[test]
    fn creates_readme_without_overwriting_it() {
        let path = temp_path("themes");
        ensure_themes_dir_at(&path).unwrap();
        let readme = path.join("README.txt");
        assert!(fs::read_to_string(&readme).unwrap().contains(".tmTheme"));
        fs::write(&readme, "custom").unwrap();
        ensure_themes_dir_at(&path).unwrap();
        assert_eq!(fs::read_to_string(&readme).unwrap(), "custom");
        fs::remove_dir_all(path).unwrap();
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("diagramide-theme-test-{unique}-{name}"))
    }
}
