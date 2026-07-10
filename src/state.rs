use eframe::egui::{self};
use parking_lot::RwLock;
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
};

use crate::{
    EditorType, identifiers,
    mini_window::{self},
    modal::Modal,
    state_serialize::AppStatePersistent,
};

/// Stable identifier for a workspace. Stored as a plain `u64` (rather than
/// `egui::Id`) so it serializes compactly and never collides with the
/// per-window `egui::Id`s that live *inside* a workspace.
pub type WorkspaceId = u64;

/// A dormant workspace: its full content (editors + dependency graph) plus
/// identity. At any moment exactly one workspace is "live" — its `windows`
/// and `editor_deps` are unpacked into the top-level `AppState` fields of the
/// same name so the rest of the codebase never has to know about the
/// workspace indirection. All *other* workspaces live here, dormant.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub windows: HashMap<egui::Id, mini_window::Window>,
    pub editor_deps: HashMap<egui::Id, HashSet<egui::Id>>,
    #[serde(default)]
    pub window_library_paths: HashMap<egui::Id, String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LibraryEntry {
    pub path: String,
    pub editor_type: EditorType,
    #[serde(default)]
    pub output_type: crate::OutputType,
    pub content: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct WindowState {
    pub debug: bool,
    pub log: bool,
    pub profiler: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiagramBackground {
    Black,
    ThemeDark,
    ThemeBright,
    #[default]
    White,
}

impl DiagramBackground {
    pub fn resolve(self, visuals: &egui::Visuals) -> egui::Color32 {
        match self {
            Self::Black => egui::Color32::BLACK,
            Self::ThemeDark => visuals.panel_fill,
            Self::ThemeBright => visuals.faint_bg_color,
            Self::White => egui::Color32::WHITE,
        }
    }

    /// Resolve to a render-time background color without access to egui::Visuals.
    /// Theme-dependent variants fall back to neutral defaults suitable for exports.
    pub fn resolve_for_export(self, visuals: &egui::Visuals) -> crate::image::RenderBackground {
        match self {
            Self::Black => crate::image::RenderBackground::Color(egui::Color32::BLACK),
            Self::White => crate::image::RenderBackground::Color(egui::Color32::WHITE),
            Self::ThemeDark => crate::image::RenderBackground::Color(visuals.panel_fill),
            Self::ThemeBright => crate::image::RenderBackground::Color(visuals.faint_bg_color),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(from = "AppStatePersistent", into = "AppStatePersistent")]
pub struct AppState {
    pub log: Vec<String>,
    pub editor_deps: HashMap<egui::Id, HashSet<egui::Id>>,
    pub window_library_paths: HashMap<egui::Id, String>,
    pub window_states: WindowState,
    pub windows: HashMap<egui::Id, mini_window::Window>,
    pub modals: VecDeque<Arc<RwLock<dyn Modal>>>,
    pub active_theme: String,
    pub diagram_background: DiagramBackground,
    pub library: BTreeMap<String, LibraryEntry>,

    // ── Multiple workspaces ────────────────────────────────────────────
    // `windows` and `editor_deps` above are always the *live* (active)
    // workspace, unpacked at the top level so the ~66 existing call sites
    // are untouched. Everything below is the workspace registry.
    /// Id of the workspace currently unpacked into `windows` / `editor_deps`.
    pub active_workspace_id: WorkspaceId,
    /// Display name of the active workspace (mirrored for cheap UI reads).
    pub active_workspace_name: String,
    /// All *dormant* workspaces (i.e. every workspace except the active one).
    /// The active workspace is flushed here on `switch_to` / serialize.
    pub workspaces: HashMap<WorkspaceId, Workspace>,
}

impl Default for AppState {
    fn default() -> Self {
        let active_workspace_id = identifiers::next_workspace_id();
        Self {
            log: Vec::new(),
            editor_deps: HashMap::new(),
            window_library_paths: HashMap::new(),
            modals: VecDeque::new(),
            windows: HashMap::new(),
            active_theme: crate::theme::DEFAULT_THEME_ID.to_owned(),
            diagram_background: DiagramBackground::default(),
            library: BTreeMap::new(),
            window_states: WindowState {
                profiler: false,
                debug: false,
                log: true,
            },
            active_workspace_id,
            active_workspace_name: String::from("Default"),
            workspaces: HashMap::new(),
        }
    }
}

impl AppState {
    /// Write the live (`windows`, `editor_deps`) fields back into the
    /// dormant registry under the active id/name. After this the live fields
    /// are unchanged but the registry is consistent, so a subsequent
    /// `switch_to` will persist the current workspace correctly.
    pub fn flush_active(&mut self) {
        let ws = Workspace {
            id: self.active_workspace_id,
            name: self.active_workspace_name.clone(),
            windows: self.windows.clone(),
            editor_deps: self.editor_deps.clone(),
            window_library_paths: self.window_library_paths.clone(),
        };
        self.workspaces.insert(ws.id, ws);
    }

    /// Flush the active workspace, then unpack the target workspace into the
    /// live fields. If `id` is unknown or equals the active id this is a no-op
    /// (aside from the flush, which keeps the registry consistent).
    pub fn switch_to(&mut self, id: WorkspaceId) {
        if id == self.active_workspace_id {
            self.flush_active();
            return;
        }
        let Some(target) = self.workspaces.remove(&id) else {
            // unknown id: just keep things consistent and bail
            self.flush_active();
            return;
        };
        // stash the currently-live workspace
        let prev = Workspace {
            id: self.active_workspace_id,
            name: self.active_workspace_name.clone(),
            windows: std::mem::take(&mut self.windows),
            editor_deps: std::mem::take(&mut self.editor_deps),
            window_library_paths: std::mem::take(&mut self.window_library_paths),
        };
        self.workspaces.insert(prev.id, prev);

        // promote the target
        self.active_workspace_id = target.id;
        self.active_workspace_name = target.name;
        self.windows = target.windows;
        self.editor_deps = target.editor_deps;
        self.window_library_paths = target.window_library_paths;
    }

    /// Create a new empty workspace with the given name, register it as
    /// dormant, and return its id. Does *not* switch to it.
    pub fn new_workspace(&mut self, name: String) -> WorkspaceId {
        let id = identifiers::next_workspace_id();
        self.workspaces.insert(
            id,
            Workspace {
                id,
                name,
                windows: HashMap::new(),
                editor_deps: HashMap::new(),
                window_library_paths: HashMap::new(),
            },
        );
        id
    }

    /// Duplicate the currently active workspace (content + deps) into a new
    /// dormant workspace with `"<name> (copy)"` and return its id. Does not
    /// switch.
    pub fn duplicate_active(&mut self) -> WorkspaceId {
        self.flush_active();
        let id = identifiers::next_workspace_id();
        let name = format!("{} (copy)", self.active_workspace_name);
        let source = self
            .workspaces
            .get(&self.active_workspace_id)
            .expect("active workspace must be in registry after flush");
        let clone = Workspace {
            id,
            name,
            windows: source.windows.clone(),
            editor_deps: source.editor_deps.clone(),
            window_library_paths: source.window_library_paths.clone(),
        };
        self.workspaces.insert(id, clone);
        id
    }

    /// Rename a workspace by id. If it is the active one, the mirrored
    /// `active_workspace_name` is updated too. Unknown ids are ignored.
    pub fn rename_workspace(&mut self, id: WorkspaceId, name: String) {
        if id == self.active_workspace_id {
            self.active_workspace_name = name.clone();
        }
        if let Some(ws) = self.workspaces.get_mut(&id) {
            ws.name = name;
        }
    }

    /// Delete a workspace. The last remaining workspace can never be deleted.
    /// Deleting the active workspace auto-switches to another one first.
    /// Returns `true` if something was actually removed.
    pub fn delete_workspace(&mut self, id: WorkspaceId) -> bool {
        // total workspace count = dormant + 1 (the active one)
        let total = self.workspaces.len() + 1;
        if total <= 1 {
            return false;
        }
        if id == self.active_workspace_id {
            // pick any other workspace to promote
            let Some(&other) = self.workspaces.keys().next() else {
                return false;
            };
            self.switch_to(other);
            // switch_to moved the old active into the registry; now drop it
            self.workspaces.remove(&id).is_some()
        } else {
            self.workspaces.remove(&id).is_some()
        }
    }

    /// Snapshot of every workspace id/name, with the active one included.
    /// Ordered active-first then by id for stable menu rendering.
    pub fn workspace_listing(&self) -> Vec<(WorkspaceId, String, bool)> {
        let mut out: Vec<(WorkspaceId, String, bool)> =
            Vec::with_capacity(self.workspaces.len() + 1);
        out.push((
            self.active_workspace_id,
            self.active_workspace_name.clone(),
            true,
        ));
        let mut dormant: Vec<_> = self
            .workspaces
            .iter()
            .map(|(id, ws)| (*id, ws.name.clone(), false))
            .collect();
        dormant.sort_by_key(|(id, _, _)| *id);
        out.extend(dormant);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagram_background_resolves_fixed_and_theme_colors() {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(10, 20, 30);
        visuals.faint_bg_color = egui::Color32::from_rgb(40, 50, 60);

        assert_eq!(
            DiagramBackground::Black.resolve(&visuals),
            egui::Color32::BLACK
        );
        assert_eq!(
            DiagramBackground::ThemeDark.resolve(&visuals),
            visuals.panel_fill
        );
        assert_eq!(
            DiagramBackground::ThemeBright.resolve(&visuals),
            visuals.faint_bg_color
        );
        assert_eq!(
            DiagramBackground::White.resolve(&visuals),
            egui::Color32::WHITE
        );
    }

    #[test]
    fn diagram_background_defaults_to_white() {
        assert_eq!(DiagramBackground::default(), DiagramBackground::White);
    }

    #[test]
    fn diagram_background_resolve_for_export_maps_correctly() {
        let visuals = egui::Visuals::dark();
        // Fixed colors map directly
        match DiagramBackground::Black.resolve_for_export(&visuals) {
            crate::image::RenderBackground::Color(c) => assert_eq!(c, egui::Color32::BLACK),
            _ => panic!("expected Color"),
        }
        match DiagramBackground::White.resolve_for_export(&visuals) {
            crate::image::RenderBackground::Color(c) => assert_eq!(c, egui::Color32::WHITE),
            _ => panic!("expected Color"),
        }
        // Theme variants resolve to reasonable defaults (not transparent)
        match DiagramBackground::ThemeDark.resolve_for_export(&visuals) {
            crate::image::RenderBackground::Color(_) => {},
            other => panic!("expected Color, got {:?}", other),
        }
        match DiagramBackground::ThemeBright.resolve_for_export(&visuals) {
            crate::image::RenderBackground::Color(_) => {},
            other => panic!("expected Color, got {:?}", other),
        }
    }

    // ── Workspace tests ───────────────────────────────────────────────

    #[test]
    fn default_state_has_one_active_workspace_named_default() {
        let state = AppState::default();
        assert_eq!(state.active_workspace_name, "Default");
        assert!(state.workspaces.is_empty());
        let listing = state.workspace_listing();
        assert_eq!(listing.len(), 1);
        assert!(listing[0].2); // active flag
    }

    #[test]
    fn new_workspace_is_dormant_until_switched() {
        let mut state = AppState::default();
        let original_active = state.active_workspace_id;
        let id = state.new_workspace("Second".into());
        assert_ne!(id, original_active);
        // active unchanged
        assert_eq!(state.active_workspace_id, original_active);
        assert_eq!(state.workspaces.len(), 1);
        assert_eq!(state.workspaces[&id].name, "Second");
    }

    #[test]
    fn switch_to_swaps_live_fields_and_preserves_content() {
        let mut state = AppState::default();
        let first = state.active_workspace_id;
        // put a marker window in the first workspace
        let marker = egui::Id::new("marker");
        state.windows.insert(
            marker,
            mini_window::Window::PlainTextEditor(crate::plain_text_editor::PlainTextEditor::new(
                marker,
            )),
        );

        let second = state.new_workspace("Second".into());
        state.switch_to(second);

        // live fields now belong to the second (empty) workspace
        assert_eq!(state.active_workspace_id, second);
        assert_eq!(state.active_workspace_name, "Second");
        assert!(state.windows.is_empty());
        // first workspace is dormant but preserved
        assert_eq!(state.workspaces.len(), 1);
        assert!(state.workspaces[&first].windows.contains_key(&marker));

        // switch back
        state.switch_to(first);
        assert_eq!(state.active_workspace_id, first);
        assert!(state.windows.contains_key(&marker));
    }

    #[test]
    fn cannot_delete_last_workspace() {
        let mut state = AppState::default();
        let only = state.active_workspace_id;
        assert!(!state.delete_workspace(only));
        assert_eq!(state.active_workspace_id, only);
    }

    #[test]
    fn delete_active_auto_switches_to_another() {
        let mut state = AppState::default();
        let second = state.new_workspace("Second".into());
        state.switch_to(second);
        assert_eq!(state.active_workspace_id, second);

        assert!(state.delete_workspace(second));
        // auto-switched away; active is no longer the deleted one
        assert_ne!(state.active_workspace_id, second);
        assert!(!state.workspaces.contains_key(&second));
        // and the original first workspace is still reachable
        assert_eq!(state.workspace_listing().len(), 1);
    }

    #[test]
    fn rename_updates_active_mirror_and_dormant_entry() {
        let mut state = AppState::default();
        let active = state.active_workspace_id;
        state.rename_workspace(active, "RenamedActive".into());
        assert_eq!(state.active_workspace_name, "RenamedActive");

        let other = state.new_workspace("Other".into());
        state.rename_workspace(other, "OtherRenamed".into());
        assert_eq!(state.workspaces[&other].name, "OtherRenamed");
    }

    #[test]
    fn duplicate_active_clones_content_under_new_id() {
        let mut state = AppState::default();
        let marker = egui::Id::new("marker");
        state.windows.insert(
            marker,
            mini_window::Window::PlainTextEditor(crate::plain_text_editor::PlainTextEditor::new(
                marker,
            )),
        );
        let dup = state.duplicate_active();
        assert_ne!(dup, state.active_workspace_id);
        assert_eq!(state.workspaces[&dup].name, "Default (copy)");
        assert!(state.workspaces[&dup].windows.contains_key(&marker));
    }

    #[test]
    fn persistence_roundtrip_preserves_all_workspaces() {
        let mut state = AppState::default();
        let first = state.active_workspace_id;
        let marker = egui::Id::new("marker");
        state.windows.insert(
            marker,
            mini_window::Window::PlainTextEditor(crate::plain_text_editor::PlainTextEditor::new(
                marker,
            )),
        );
        let second = state.new_workspace("Second".into());
        state.switch_to(second);

        // serialize (flush active first) then deserialize
        let persisted = crate::state_serialize::AppStatePersistent::from(state.clone());
        let restored = AppState::from(persisted);

        assert_eq!(restored.workspaces.len(), 1);
        assert_eq!(restored.active_workspace_id, second);
        assert_eq!(restored.active_workspace_name, "Second");
        // the dormant first workspace survived
        let first_ws = &restored.workspaces[&first];
        assert!(first_ws.windows.contains_key(&marker));
    }

    #[test]
    fn persistence_roundtrip_preserves_library_and_editor_origins() {
        let mut state = AppState::default();
        let marker = egui::Id::new("library-origin");
        state.windows.insert(
            marker,
            mini_window::Window::PlainTextEditor(crate::plain_text_editor::PlainTextEditor::new(
                marker,
            )),
        );
        state
            .window_library_paths
            .insert(marker, "docs/example".into());
        state.library.insert(
            "docs/example".into(),
            LibraryEntry {
                path: "docs/example".into(),
                editor_type: crate::EditorType::PlainText,
                output_type: crate::OutputType::Pikchr,
                content: "hello".into(),
            },
        );

        let persisted = crate::state_serialize::AppStatePersistent::from(state);
        let restored = AppState::from(persisted);

        assert_eq!(
            restored.library["docs/example"].content,
            String::from("hello")
        );
        assert_eq!(
            restored
                .window_library_paths
                .get(&marker)
                .map(String::as_str),
            Some("docs/example")
        );
    }

    #[test]
    fn output_type_defaults_for_legacy_state_and_library_json() {
        let id = egui::Id::new("legacy-output");
        let editor = crate::pikchr_editor::PikchrEditor::new(id, egui::Id::new("legacy-svg"));
        let mut value = serde_json::to_value(editor).unwrap();
        value.as_object_mut().unwrap().remove("output_type");
        let restored: crate::pikchr_editor::PikchrEditor = serde_json::from_value(value).unwrap();
        assert_eq!(restored.output_type, crate::OutputType::Pikchr);

        let entry: LibraryEntry =
            serde_json::from_str(r#"{"path":"legacy","editor_type":"Pikchr","content":"box"}"#)
                .unwrap();
        assert_eq!(entry.output_type, crate::OutputType::Pikchr);
    }

    #[test]
    fn legacy_format_without_workspaces_is_migrated_to_default() {
        // Build a pre-workspace AppStatePersistent: no `workspaces` field,
        // just top-level windows/editor_deps. serde(default) + the migration
        // in `From<AppStatePersistent>` must turn this into a single Default
        // workspace that is active.
        let mut legacy = crate::state_serialize::AppStatePersistent {
            active_workspace_name: String::from("Default"),
            ..Default::default()
        };
        let marker = egui::Id::new("legacy");
        legacy.windows.insert(
            marker,
            mini_window::Window::PlainTextEditor(crate::plain_text_editor::PlainTextEditor::new(
                marker,
            )),
        );
        legacy.workspaces = HashMap::new(); // simulate old save

        let restored = AppState::from(legacy);
        assert!(restored.workspaces.is_empty());
        assert_eq!(restored.active_workspace_name, "Default");
        assert!(restored.windows.contains_key(&marker));
    }
}
