use std::path::PathBuf;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use directories::ProjectDirs;

use crate::editor_state::{Editor, EditorSaveState};

#[derive(Error, Debug)]
pub enum SaveStateError {
  #[error("Project dir not found")]
	ProjectDirNotFound,
  #[error("Save state not found")]
	SaveStateNotFound,
  #[error("IO Error: {0}")]
  IoError(#[from] std::io::Error),
  #[error("Deserialization error: {0:?}")]
  DeserializationError(#[from] postcard::Error),
}
pub trait Stateful {
    type SaveStateType: Serialize + DeserializeOwned + Clone;

		fn save_write(save_data: Self::SaveStateType) -> Result<(), SaveStateError> {
        let cache_file = get_cache_project_file()?;

        let serialized = postcard::to_stdvec(&save_data)?;
        std::fs::write(cache_file, serialized)?;
        Ok(())
		}
		fn to_save_state(&self) -> Self::SaveStateType;
		fn from_save_state(&mut self, state: Self::SaveStateType) -> &Self;

    fn state_save(&self) -> Result<(), SaveStateError> {
        let save_data = self.to_save_state();
        Self::save_write(save_data)

    }
    fn state_load(&mut self) -> Result<(),SaveStateError> {
        let cache_file = get_cache_project_file()?;
        if !cache_file.exists() {
            return Err(SaveStateError::SaveStateNotFound)
        }

        let serialized_byte = std::fs::read(cache_file)?;
        let deserialized: Self::SaveStateType = postcard::from_bytes::<Self::SaveStateType>(serialized_byte.as_slice())?;
        self.from_save_state(deserialized);
        Ok(())
    }
}

fn get_cache_project_file() -> Result<PathBuf, SaveStateError> {
    let project_dir = ProjectDirs::from("sh", "axk", "pikchr_pl").ok_or(SaveStateError::ProjectDirNotFound)?;
    let cache_dir = project_dir.cache_dir();

    if !cache_dir.exists() {
        std::fs::create_dir_all(cache_dir)?
    }
    Ok(cache_dir.join("state.json"))
}

impl Stateful for Editor {
    type SaveStateType = EditorSaveState;

    fn to_save_state(&self) -> Self::SaveStateType {
        EditorSaveState {
            current_file: self.current_file.clone(),
            file_watch_mode: self.file_watch_mode,
            show_debug: self.show_debug,
            operating_mode: self.operating_mode,
        }
    }

    fn from_save_state(&mut self, state: Self::SaveStateType) -> &Self {
            self.current_file = state.current_file;
            self.file_watch_mode = state.file_watch_mode;
            self.show_debug = state.show_debug;
            self.operating_mode = state.operating_mode;
            self
    }
}

