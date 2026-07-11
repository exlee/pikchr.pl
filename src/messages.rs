use std::time::Duration;

use eframe::egui::{self, Context};

use crate::{help::HelpTopic, mini_window::WindowType, state};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum ExportType {
    Svg,
    Png,
    PngTransparent,
    Source(OutputType),
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum Msg {
    Batch(Vec<Msg>),
    Debounce(Duration, egui::Id, Box<Msg>),
    PopModal,
    CheckDependencies,
    ShowHelp(HelpTopic),
    SelectTheme(#[serde(skip)] Context, String),
    ReloadThemes(#[serde(skip)] Context),
    OpenThemesFolder,
    SetDiagramBackground(#[serde(skip)] Context, state::DiagramBackground),
    ExportModal(egui::Id, String, ExportType),
    Export(egui::Id, String, ExportType, Box<egui::Visuals>),
    CopyExport(
        #[serde(skip)] Context,
        egui::Id,
        ExportType,
        Box<egui::Visuals>,
    ),
    FontSizeModal(egui::Id),
    SaveEditorToLibraryRequest(#[serde(skip)] Context, egui::Id),
    SaveEditorToLibrary {
        editor_id: egui::Id,
        path: String,
        overwrite: bool,
    },
    ExportEditorLibraryEntry(egui::Id),
    RequestRename(#[serde(skip)] Context, egui::Id),
    RenameWindow(egui::Id, String),
    RequestRedraw(#[serde(skip)] Context, egui::Id),
    UpdateRender(#[serde(skip)] Context, egui::Id, String),
    UpdateProlog(#[serde(skip)] Context, egui::Id, String),
    UpdateTcl(#[serde(skip)] Context, egui::Id, String),
    UpdateMruby(#[serde(skip)] Context, egui::Id, String),
    UpdatePlainText(#[serde(skip)] Context, egui::Id),
    ResetError(egui::Id),
    UpdateGeneratedContent(egui::Id, String),
    SetRenderEnabled(#[serde(skip)] Context, egui::Id, bool),
    SetOutputType(#[serde(skip)] Context, egui::Id, OutputType),
    SetSvgbobEditMode(egui::Id, SvgbobEditMode),
    DeleteWindow(egui::Id),
    ToggleWindow(Window),
    ToggleWindowById(egui::Id),
    NewWindow(#[serde(skip)] Context, WindowType),
    RecreateSvg(#[serde(skip)] Context, egui::Id),
    ReloadSvgs(#[serde(skip)] Context),
    Refresh(#[serde(skip)] Context, egui::Id),
    RefreshWorkspace(#[serde(skip)] Context),
    ResetWorkspaceRequest,
    ResetWorkspace,
    SaveWorkspace,
    LoadWorkspaceRequest,
    LoadWorkspace(String),
    OpenLibraryEntry(#[serde(skip)] Context, String),
    DeleteLibraryEntryRequest(String),
    DeleteLibraryEntry(String),
    ExportLibraryEntry(String),
    ImportLibraryEntries,
    ImportLibraryEntry(state::LibraryEntry, bool),
    SwitchWorkspace(state::WorkspaceId),
    NewWorkspaceRequest,
    NewWorkspace(String),
    RenameWorkspaceRequest(state::WorkspaceId),
    RenameWorkspace(state::WorkspaceId, String),
    DuplicateWorkspace(state::WorkspaceId),
    DeleteWorkspaceRequest(state::WorkspaceId),
}

#[derive(Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum SvgbobEditMode {
    #[default]
    Insert,
    Replace,
}

impl SvgbobEditMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Insert => "Insert",
            Self::Replace => "Replace",
        }
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::Insert => Self::Replace,
            Self::Replace => Self::Insert,
        }
    }
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum EditorType {
    Prolog,
    Pikchr,
    Svgbob,
    Tcl,
    Mruby,
    PlainText,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Clone, Copy, Default)]
pub enum OutputType {
    #[default]
    Pikchr,
    Svgbob,
}

impl OutputType {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pikchr => "Pikchr",
            Self::Svgbob => "Svgbob",
        }
    }

    pub const fn source_extension(self) -> &'static str {
        match self {
            Self::Pikchr => "pikchr",
            Self::Svgbob => "txt",
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum Window {
    Logger,
    Debugger,
}
