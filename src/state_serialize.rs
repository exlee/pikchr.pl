use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    io::Write as _,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use eframe::egui::{self};
use parking_lot::RwLock;
use tokio::sync::mpsc;

use crate::{
    DiagramIDE, Msg, identifiers, logger, mini_window,
    state::{AppState, DiagramBackground, LibraryEntry, WindowState, Workspace, WorkspaceId},
};

#[cfg(not(test))]
const APP_ID: &str = "sh.axk.diagramide";
const BACKUP_KEYS: [&str; 3] = [
    "diagramide.app.backup.1",
    "diagramide.app.backup.2",
    "diagramide.app.backup.3",
];
const UNREADABLE_KEY: &str = "diagramide.app.unreadable";
const PREVIOUS_UNREADABLE_KEY: &str = "diagramide.app.unreadable.previous";

#[derive(Clone, Debug, Default)]
pub(crate) enum PersistenceStatus {
    #[default]
    Clean,
    Recovered {
        unreadable_primary: String,
        error: String,
        recovery_path: Option<PathBuf>,
    },
    Blocked {
        unreadable_primary: String,
        error: String,
        recovery_path: Option<PathBuf>,
    },
}

impl PersistenceStatus {
    pub(crate) fn warning(&self) -> Option<String> {
        match self {
            Self::Clean => None,
            Self::Recovered {
                error,
                recovery_path,
                ..
            } => Some(format!(
                "The primary workspace save was unreadable ({error}). A validated backup was recovered. The unreadable RON was preserved{}.",
                recovery_path
                    .as_ref()
                    .map(|path| format!(" at {}", path.display()))
                    .unwrap_or_default()
            )),
            Self::Blocked {
                error,
                recovery_path,
                ..
            } => Some(format!(
                "WORKSPACE SAVING IS BLOCKED: saved data is unreadable ({error}) and no validated backup exists. The original RON will not be overwritten and was copied{}.",
                recovery_path
                    .as_ref()
                    .map(|path| format!(" to {}", path.display()))
                    .unwrap_or_else(|| " in memory; filesystem recovery copy failed".to_owned())
            )),
        }
    }
}

pub(crate) enum PersistenceLoad {
    Missing,
    Loaded(DiagramIDEPersistent),
    Recovered(DiagramIDEPersistent, PersistenceStatus),
    Blocked(PersistenceStatus),
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct AppStatePersistent {
    #[serde(skip_serializing, default)]
    pub log: Vec<String>,
    pub editor_deps: HashMap<egui::Id, HashSet<egui::Id>>,
    #[serde(default)]
    pub window_library_paths: HashMap<egui::Id, String>,
    pub window_states: WindowState,
    pub windows: HashMap<egui::Id, mini_window::Window>,
    #[serde(default = "default_theme")]
    pub active_theme: String,
    #[serde(default)]
    pub diagram_background: DiagramBackground,
    #[serde(default)]
    pub library: BTreeMap<String, LibraryEntry>,

    // ── Multiple workspaces ───────────────────────────────────────────
    #[serde(default)]
    pub active_workspace_id: WorkspaceId,
    #[serde(default = "default_workspace_name")]
    pub active_workspace_name: String,
    /// Dormant workspaces. Absent (or empty) on pre-workspace save files,
    /// which triggers migration in `From<AppStatePersistent>`.
    #[serde(default)]
    pub workspaces: HashMap<WorkspaceId, Workspace>,
}

fn default_theme() -> String {
    crate::theme::DEFAULT_THEME_ID.to_owned()
}

fn default_workspace_name() -> String {
    String::from("Default")
}

impl From<AppState> for AppStatePersistent {
    fn from(mut value: AppState) -> Self {
        // Flush the live workspace into the dormant registry so the active
        // workspace is captured in `workspaces` alongside the others.
        value.flush_active();
        let active_ws = value
            .workspaces
            .get(&value.active_workspace_id)
            .cloned()
            .unwrap_or_default();
        Self {
            log: value.log,
            editor_deps: active_ws.editor_deps,
            window_library_paths: active_ws.window_library_paths,
            window_states: value.window_states,
            windows: active_ws.windows,
            active_theme: value.active_theme,
            diagram_background: value.diagram_background,
            active_workspace_id: value.active_workspace_id,
            active_workspace_name: value.active_workspace_name,
            workspaces: value.workspaces,
            library: value.library,
        }
    }
}
impl From<AppStatePersistent> for AppState {
    fn from(value: AppStatePersistent) -> Self {
        // Migration: pre-workspace save files have no `workspaces` map (and
        // default `active_workspace_id == 0`). Fold their legacy
        // `windows`/`editor_deps` into a single freshly-id'd "Default"
        // workspace and make it active (live fields). The dormant map stays
        // empty because there is only this one workspace.
        if value.workspaces.is_empty() {
            let id = identifiers::next_workspace_id();
            return Self {
                log: value.log,
                editor_deps: value.editor_deps,
                window_library_paths: value.window_library_paths,
                window_states: value.window_states,
                windows: value.windows,
                modals: VecDeque::new(),
                active_theme: value.active_theme,
                diagram_background: value.diagram_background,
                active_workspace_id: id,
                active_workspace_name: value.active_workspace_name,
                workspaces: HashMap::new(),
                library: value.library,
            };
        }

        // Normal path: promote the active workspace to the live fields.
        let active_id = value.active_workspace_id;
        let mut workspaces = value.workspaces;
        let active = workspaces.remove(&active_id).unwrap_or_else(|| {
            // active id missing from map — fall back to first remaining,
            // or synthesize an empty default workspace.
            if let Some((&id, ws)) = workspaces.iter().next() {
                let ws = ws.clone();
                let _ = id;
                ws
            } else {
                Workspace {
                    id: active_id,
                    name: value.active_workspace_name.clone(),
                    windows: value.windows.clone(),
                    editor_deps: value.editor_deps.clone(),
                    window_library_paths: value.window_library_paths.clone(),
                }
            }
        });

        Self {
            log: value.log,
            editor_deps: active.editor_deps,
            window_library_paths: active.window_library_paths,
            window_states: value.window_states,
            windows: active.windows,
            modals: VecDeque::new(),
            active_theme: value.active_theme,
            diagram_background: value.diagram_background,
            active_workspace_id: active.id,
            active_workspace_name: active.name,
            workspaces,
            library: value.library,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DiagramIDEPersistent {
    state: AppStatePersistent,
    window_size: egui::Vec2,
}

pub(crate) fn load_persistent(storage: &dyn eframe::Storage) -> PersistenceLoad {
    let Some(primary) = storage.get_string(eframe::APP_KEY) else {
        #[cfg(not(test))]
        if let Some(blocked) = unreadable_storage_file() {
            return PersistenceLoad::Blocked(blocked);
        }
        return BACKUP_KEYS
            .iter()
            .find_map(|key| parse_stored(storage, key))
            .map(PersistenceLoad::Loaded)
            .unwrap_or(PersistenceLoad::Missing);
    };

    match ron::from_str(&primary) {
        Ok(value) => PersistenceLoad::Loaded(value),
        Err(error) => {
            let error = error.to_string();
            let recovery_path = write_recovery_fixture(&primary).ok();
            if let Some(value) = BACKUP_KEYS
                .iter()
                .find_map(|key| parse_stored(storage, key))
            {
                PersistenceLoad::Recovered(
                    value,
                    PersistenceStatus::Recovered {
                        unreadable_primary: primary,
                        error,
                        recovery_path,
                    },
                )
            } else {
                PersistenceLoad::Blocked(PersistenceStatus::Blocked {
                    unreadable_primary: primary,
                    error,
                    recovery_path,
                })
            }
        },
    }
}

fn parse_stored(storage: &dyn eframe::Storage, key: &str) -> Option<DiagramIDEPersistent> {
    storage
        .get_string(key)
        .and_then(|stored| ron::from_str(&stored).ok())
}

pub(crate) fn save_persistent(
    storage: &mut dyn eframe::Storage,
    persistent: &DiagramIDEPersistent,
    status: &mut PersistenceStatus,
) -> Result<(), String> {
    let serialized = ron::to_string(persistent)
        .map_err(|error| format!("workspace serialization failed: {error}"))?;
    ron::from_str::<DiagramIDEPersistent>(&serialized)
        .map_err(|error| format!("workspace self-validation failed: {error}"))?;

    match status {
        PersistenceStatus::Blocked {
            unreadable_primary, ..
        } => {
            preserve_unreadable(storage, unreadable_primary);
            storage.flush();
            return Err(
                "refusing to overwrite unreadable workspace data without a validated backup"
                    .to_owned(),
            );
        },
        PersistenceStatus::Recovered {
            unreadable_primary, ..
        } => preserve_unreadable(storage, unreadable_primary),
        PersistenceStatus::Clean => {
            if let Some(current) = storage.get_string(eframe::APP_KEY) {
                if ron::from_str::<DiagramIDEPersistent>(&current).is_err() {
                    let recovery_path = write_recovery_fixture(&current).ok();
                    preserve_unreadable(storage, &current);
                    *status = PersistenceStatus::Blocked {
                        unreadable_primary: current,
                        error: "primary became unreadable before save".to_owned(),
                        recovery_path,
                    };
                    storage.flush();
                    return Err("refusing to overwrite unreadable workspace data".to_owned());
                }
                rotate_backups(storage, current);
            }
        },
    }

    storage.set_string(eframe::APP_KEY, serialized);
    storage.flush();
    *status = PersistenceStatus::Clean;
    Ok(())
}

fn rotate_backups(storage: &mut dyn eframe::Storage, current: String) {
    for index in (1..BACKUP_KEYS.len()).rev() {
        if let Some(previous) = storage.get_string(BACKUP_KEYS[index - 1]) {
            storage.set_string(BACKUP_KEYS[index], previous);
        }
    }
    storage.set_string(BACKUP_KEYS[0], current);
}

fn preserve_unreadable(storage: &mut dyn eframe::Storage, unreadable: &str) {
    if storage.get_string(UNREADABLE_KEY).as_deref() == Some(unreadable) {
        return;
    }
    if let Some(previous) = storage.get_string(UNREADABLE_KEY) {
        storage.set_string(PREVIOUS_UNREADABLE_KEY, previous);
    }
    storage.set_string(UNREADABLE_KEY, unreadable.to_owned());
}

fn write_recovery_fixture(unreadable: &str) -> std::io::Result<PathBuf> {
    #[cfg(not(test))]
    let directory = eframe::storage_dir(APP_ID)
        .ok_or_else(|| std::io::Error::other("application storage directory is unavailable"))?
        .join("recovery");
    #[cfg(test)]
    let directory = std::env::temp_dir().join("diagramide-persistence-recovery-tests");
    write_recovery_bytes(&directory, "unreadable", unreadable.as_bytes())
}

fn write_recovery_bytes(
    directory: &std::path::Path,
    prefix: &str,
    bytes: &[u8],
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(directory)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..100 {
        let path = directory.join(format!("{prefix}-{timestamp}-{attempt}.ron"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(bytes)?;
                file.sync_all()?;
                return Ok(path);
            },
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique recovery fixture name",
    ))
}

#[cfg(not(test))]
fn unreadable_storage_file() -> Option<PersistenceStatus> {
    let path = eframe::storage_dir(APP_ID)?.join("app.ron");
    let bytes = std::fs::read(&path).ok()?;
    if ron::de::from_bytes::<HashMap<String, String>>(&bytes).is_ok() {
        return None;
    }

    let directory = path.parent()?.join("recovery");
    let recovery_path = write_recovery_bytes(&directory, "unreadable-storage", &bytes).ok();

    Some(PersistenceStatus::Blocked {
        unreadable_primary: String::from_utf8_lossy(&bytes).into_owned(),
        error: "the outer Eframe storage file is unreadable".to_owned(),
        recovery_path,
    })
}

impl From<DiagramIDEPersistent> for DiagramIDE {
    fn from(value: DiagramIDEPersistent) -> Self {
        let (tx, _rx) = mpsc::channel::<Msg>(100);
        let app_state = AppState::from(value.state);
        let seen_workspace_id = app_state.active_workspace_id;
        let state = Arc::new(RwLock::new(app_state));
        let window_size = value.window_size;
        DiagramIDE {
            tx,
            state,
            window_size,
            first_frame: true,
            logger: logger::init_logger(),
            seen_workspace_id,
            persistence_status: PersistenceStatus::Clean,
        }
    }
}
impl From<DiagramIDE> for DiagramIDEPersistent {
    fn from(value: DiagramIDE) -> Self {
        let v = value.state.read().clone();
        DiagramIDEPersistent {
            state: AppStatePersistent::from(v),
            window_size: value.window_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::Storage as _;

    #[derive(Default)]
    struct MemoryStorage(HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn flush(&mut self) {}
    }

    fn sample_persistent() -> DiagramIDEPersistent {
        let mut state = AppState {
            active_workspace_name: "Recovered workspace".to_owned(),
            ..AppState::default()
        };
        let editor_id = egui::Id::new("fixture-editor");
        state.windows.insert(
            editor_id,
            mini_window::Window::PlainTextEditor(crate::plain_text_editor::PlainTextEditor::new(
                editor_id,
            )),
        );
        for (name, window) in [
            (
                "prolog",
                mini_window::Window::PrologEditor(crate::prolog_editor::PrologEditor::new(
                    egui::Id::new("fixture-prolog"),
                    egui::Id::new("fixture-prolog-svg"),
                )),
            ),
            (
                "tcl",
                mini_window::Window::TclEditor(crate::tcl_editor::TclEditor::new(
                    egui::Id::new("fixture-tcl"),
                    egui::Id::new("fixture-tcl-svg"),
                )),
            ),
            (
                "ruby",
                mini_window::Window::MrubyEditor(crate::mruby_editor::MrubyEditor::new(
                    egui::Id::new("fixture-ruby"),
                    egui::Id::new("fixture-ruby-svg"),
                )),
            ),
        ] {
            state.windows.insert(egui::Id::new(name), window);
        }
        DiagramIDEPersistent {
            state: AppStatePersistent::from(state),
            window_size: egui::vec2(1024.0, 768.0),
        }
    }

    fn assert_exact_recovery_fixture(status: &PersistenceStatus, expected: &str) {
        let path = match status {
            PersistenceStatus::Recovered { recovery_path, .. }
            | PersistenceStatus::Blocked { recovery_path, .. } => recovery_path
                .as_ref()
                .expect("an exact filesystem recovery fixture"),
            PersistenceStatus::Clean => panic!("expected recovery status"),
        };
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
    }

    #[test]
    fn pre_refactor_workspace_fixture_remains_loadable() {
        let fixture = include_str!("../tests/fixtures/persistence/pre-refactor-workspaces.ron");
        let persistent: DiagramIDEPersistent = ron::from_str(fixture).unwrap();
        let state = AppState::from(persistent.state);

        assert_eq!(state.active_workspace_name, "Recovered workspace");
        assert_eq!(state.windows.len(), 4);
        assert!(
            state
                .windows
                .values()
                .any(|window| matches!(window, mini_window::Window::PrologEditor(_)))
        );
        assert!(
            state
                .windows
                .values()
                .any(|window| matches!(window, mini_window::Window::TclEditor(_)))
        );
        assert!(
            state
                .windows
                .values()
                .any(|window| matches!(window, mini_window::Window::MrubyEditor(_)))
        );
    }

    #[test]
    fn unreadable_primary_is_preserved_and_valid_backup_is_recovered() {
        let unreadable = include_str!("../tests/fixtures/persistence/unreadable-primary.ron");
        let backup = ron::to_string(&sample_persistent()).unwrap();
        let mut storage = MemoryStorage(HashMap::from([
            (eframe::APP_KEY.to_owned(), unreadable.to_owned()),
            (BACKUP_KEYS[0].to_owned(), backup),
        ]));

        let PersistenceLoad::Recovered(persistent, mut status) = load_persistent(&storage) else {
            panic!("expected backup recovery");
        };
        assert_exact_recovery_fixture(&status, unreadable);
        save_persistent(&mut storage, &persistent, &mut status).unwrap();

        assert_eq!(
            storage.get_string(UNREADABLE_KEY).as_deref(),
            Some(unreadable)
        );
        assert!(
            ron::from_str::<DiagramIDEPersistent>(&storage.get_string(eframe::APP_KEY).unwrap())
                .is_ok()
        );
    }

    #[test]
    fn unreadable_primary_without_backup_blocks_overwrite() {
        let unreadable = include_str!("../tests/fixtures/persistence/unreadable-primary.ron");
        let mut storage = MemoryStorage(HashMap::from([(
            eframe::APP_KEY.to_owned(),
            unreadable.to_owned(),
        )]));
        let PersistenceLoad::Blocked(mut status) = load_persistent(&storage) else {
            panic!("expected blocked persistence");
        };
        assert_exact_recovery_fixture(&status, unreadable);

        assert!(save_persistent(&mut storage, &sample_persistent(), &mut status).is_err());
        assert_eq!(
            storage.get_string(eframe::APP_KEY).as_deref(),
            Some(unreadable)
        );
        assert_eq!(
            storage.get_string(UNREADABLE_KEY).as_deref(),
            Some(unreadable)
        );
    }

    #[test]
    fn validated_save_rotates_the_previous_primary_into_backup() {
        let previous = include_str!("../tests/fixtures/persistence/pre-refactor-workspaces.ron");
        let mut storage = MemoryStorage(HashMap::from([(
            eframe::APP_KEY.to_owned(),
            previous.to_owned(),
        )]));
        let mut status = PersistenceStatus::Clean;

        save_persistent(&mut storage, &sample_persistent(), &mut status).unwrap();

        assert_eq!(
            storage.get_string(BACKUP_KEYS[0]).as_deref(),
            Some(previous)
        );
        assert!(
            ron::from_str::<DiagramIDEPersistent>(&storage.get_string(eframe::APP_KEY).unwrap())
                .is_ok()
        );
    }
}
