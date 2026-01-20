use std::path::PathBuf;

use iced::{keyboard::Modifiers, widget::text_editor};
use pikchr_pro::types::PikchrCode;

use crate::{ApplicationError, OperatingMode};

#[derive(Debug, Clone)]
pub enum Message {
    Edit(text_editor::Action),
    Ignore,
    LoadFileSelected(Option<PathBuf>),
    LoadRequested,
    ModifiersChanged(Modifiers),
    NewRequested,
    PerformAction(text_editor::Action),
    PerformActions(bool, Vec<text_editor::Action>),
    PikchrFinished(Option<Result<String, ApplicationError>>),
    PrologFinished(Result<PikchrCode, ApplicationError>),
    RadioSelected(OperatingMode),
    RefreshTick,
    RunLogic,
    RunPikchr(PikchrCode),
    RunProlog(String),
    SaveFileSelected(Option<PathBuf>),
    SaveFinished,
    SaveRequested,
    ShowError(ApplicationError),
    ShowPikchr(PikchrCode),
}
