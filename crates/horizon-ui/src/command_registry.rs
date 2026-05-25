use horizon_core::{AppShortcuts, PanelId, WorkspaceId};

/// Every dispatchable action in Horizon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandId {
    // Navigation
    SwitchWorkspace(WorkspaceId),
    FocusPanel(PanelId),
    FocusActiveWorkspace,
    FitActiveWorkspace,

    // View
    ToggleSidebar,
    ToggleHud,
    ToggleMinimap,
    ToggleFullscreenWindow,
    ToggleFullscreenPanel,
    ZoomReset,
    ZoomIn,
    ZoomOut,
    AlignWorkspacesHorizontally,

    // Workspace / panel
    NewPanel,
    OpenSquad,
    OpenRemoteHosts,
    ToggleSessions,
    CreatePanelFromPreset(usize),

    // Settings
    ToggleSettings,

    // Search
    ToggleSearch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Workspace,
    Panel,
    Preset,
    Action,
}

impl Category {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Workspace => "WORKSPACES",
            Self::Panel => "PANELS",
            Self::Preset => "PRESETS",
            Self::Action => "ACTIONS",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommandEntry {
    pub id: CommandId,
    pub label: String,
    pub shortcut: Option<String>,
    /// Extra terms matched during filtering but not displayed.
    pub keywords: Vec<String>,
}

fn command_entry(id: CommandId, label: &str, shortcut: Option<String>, keywords: &[&str]) -> CommandEntry {
    CommandEntry {
        id,
        label: label.into(),
        shortcut,
        keywords: keywords.iter().map(|keyword| (*keyword).into()).collect(),
    }
}

/// Build the static list of action commands (not workspace/panel -- those are
/// dynamic and assembled at query time by the palette).
pub fn action_commands(shortcuts: &AppShortcuts, primary_label: &str) -> Vec<CommandEntry> {
    let mut commands = workspace_commands(shortcuts, primary_label);
    commands.extend(view_commands(shortcuts, primary_label));
    commands.extend(global_commands(shortcuts, primary_label));
    commands
}

fn workspace_commands(shortcuts: &AppShortcuts, primary_label: &str) -> Vec<CommandEntry> {
    vec![
        command_entry(
            CommandId::NewPanel,
            "New Panel",
            Some(shortcuts.new_terminal.display_label(primary_label)),
            &["create", "terminal", "add"],
        ),
        command_entry(
            CommandId::OpenSquad,
            "Agent Squad",
            None,
            &["squad", "agent", "run", "fanout", "parallel"],
        ),
        command_entry(
            CommandId::FocusActiveWorkspace,
            "Focus Active Workspace",
            Some(shortcuts.focus_active_workspace.display_label(primary_label)),
            &["workspace", "focus", "pan", "center"],
        ),
        command_entry(
            CommandId::FitActiveWorkspace,
            "Fit Active Workspace",
            Some(shortcuts.fit_active_workspace.display_label(primary_label)),
            &["workspace", "fit", "zoom", "frame"],
        ),
        command_entry(
            CommandId::OpenRemoteHosts,
            "Remote Hosts",
            Some(shortcuts.open_remote_hosts.display_label(primary_label)),
            &["ssh", "tailscale", "remote", "hosts", "nodes"],
        ),
        command_entry(
            CommandId::ToggleSessions,
            "Sessions",
            Some(shortcuts.toggle_sessions.display_label(primary_label)),
            &["session", "switch", "resume", "restore"],
        ),
        command_entry(
            CommandId::AlignWorkspacesHorizontally,
            "Align Workspaces",
            Some(shortcuts.align_workspaces_horizontally.display_label(primary_label)),
            &["arrange", "horizontal", "layout", "row"],
        ),
    ]
}

fn view_commands(shortcuts: &AppShortcuts, primary_label: &str) -> Vec<CommandEntry> {
    vec![
        command_entry(
            CommandId::ToggleSidebar,
            "Toggle Sidebar",
            Some(shortcuts.toggle_sidebar.display_label(primary_label)),
            &["sidebar", "hide", "show"],
        ),
        command_entry(
            CommandId::ToggleHud,
            "Toggle HUD",
            Some(shortcuts.toggle_hud.display_label(primary_label)),
            &["heads", "up", "display", "info"],
        ),
        command_entry(
            CommandId::ToggleMinimap,
            "Toggle Minimap",
            Some(shortcuts.toggle_minimap.display_label(primary_label)),
            &["overview", "map"],
        ),
        command_entry(
            CommandId::ToggleFullscreenWindow,
            "Toggle Fullscreen (Window)",
            Some(shortcuts.fullscreen_window.display_label(primary_label)),
            &["maximize", "window", "fullscreen"],
        ),
        command_entry(
            CommandId::ToggleFullscreenPanel,
            "Toggle Fullscreen (Panel)",
            Some(shortcuts.fullscreen_panel.display_label(primary_label)),
            &["maximize", "panel", "fullscreen", "focus"],
        ),
        command_entry(
            CommandId::ZoomReset,
            "Reset Zoom",
            Some(shortcuts.zoom_reset.display_label(primary_label)),
            &["zoom", "reset", "100", "percent"],
        ),
        command_entry(
            CommandId::ZoomIn,
            "Zoom In",
            Some(shortcuts.zoom_in.display_label(primary_label)),
            &["zoom", "bigger", "enlarge"],
        ),
        command_entry(
            CommandId::ZoomOut,
            "Zoom Out",
            Some(shortcuts.zoom_out.display_label(primary_label)),
            &["zoom", "smaller", "shrink"],
        ),
    ]
}

fn global_commands(shortcuts: &AppShortcuts, primary_label: &str) -> Vec<CommandEntry> {
    vec![
        command_entry(
            CommandId::ToggleSettings,
            "Settings",
            Some(shortcuts.toggle_settings.display_label(primary_label)),
            &["settings", "config", "preferences"],
        ),
        command_entry(
            CommandId::ToggleSearch,
            "Search Terminals",
            Some(shortcuts.search.display_label(primary_label)),
            &["find", "search", "grep", "text"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use horizon_core::{AppShortcuts, ShortcutBinding, ShortcutKey, ShortcutModifiers};

    use super::{CommandId, action_commands};

    #[test]
    fn action_commands_have_unique_labels() {
        let entries = action_commands(&AppShortcuts::default(), "Ctrl");
        let labels: Vec<&str> = entries.iter().map(|e| e.label.as_str()).collect();
        let mut deduped = labels.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(labels.len(), deduped.len(), "duplicate labels found");
    }

    #[test]
    fn action_commands_all_have_shortcuts() {
        for entry in action_commands(&AppShortcuts::default(), "Ctrl") {
            if entry.id != CommandId::OpenSquad {
                assert!(entry.shortcut.is_some(), "entry '{}' has no shortcut", entry.label);
            }
        }
    }

    #[test]
    fn action_commands_include_agent_squad() {
        let entries = action_commands(&AppShortcuts::default(), "Ctrl");
        let entry = entries
            .iter()
            .find(|entry| entry.id == CommandId::OpenSquad)
            .expect("agent squad command");

        assert_eq!(entry.label, "Agent Squad");
        assert_eq!(entry.shortcut, None);
        assert!(entry.keywords.iter().any(|keyword| keyword == "parallel"));
    }

    #[test]
    fn action_commands_include_workspace_alignment() {
        let entries = action_commands(&AppShortcuts::default(), "Ctrl");
        let entry = entries
            .iter()
            .find(|entry| entry.id == CommandId::AlignWorkspacesHorizontally)
            .expect("workspace alignment command");

        assert_eq!(entry.label, "Align Workspaces");
        assert_eq!(entry.shortcut.as_deref(), Some("Ctrl+Shift+A"));
    }

    #[test]
    fn action_commands_include_workspace_focus_and_fit() {
        let entries = action_commands(&AppShortcuts::default(), "Ctrl");

        let focus = entries
            .iter()
            .find(|entry| entry.id == CommandId::FocusActiveWorkspace)
            .expect("workspace focus command");
        let fit = entries
            .iter()
            .find(|entry| entry.id == CommandId::FitActiveWorkspace)
            .expect("workspace fit command");

        assert_eq!(focus.shortcut.as_deref(), Some("Ctrl+Shift+W"));
        assert_eq!(fit.shortcut.as_deref(), Some("Ctrl+Shift+9"));
    }

    #[test]
    fn action_commands_reflect_custom_shortcuts() {
        let shortcuts = AppShortcuts {
            toggle_sidebar: ShortcutBinding::new(ShortcutModifiers::ALT, ShortcutKey::Letter('S')),
            ..AppShortcuts::default()
        };

        let entries = action_commands(&shortcuts, "Cmd");
        let entry = entries
            .iter()
            .find(|entry| entry.id == CommandId::ToggleSidebar)
            .expect("toggle sidebar command");

        assert_eq!(entry.shortcut.as_deref(), Some("Alt+S"));
    }
}
