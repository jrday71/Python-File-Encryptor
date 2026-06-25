use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(default)]
    pub gaps: GapsConfig,

    #[serde(default)]
    pub mode: ModeConfig,

    #[serde(default)]
    pub workspaces: Vec<WorkspaceConfig>,

    #[serde(default)]
    pub keybindings: Vec<Keybinding>,

    #[serde(default)]
    pub on_window_detected: Vec<OnWindowDetected>,

    #[serde(default)]
    pub exec_on_workspace_change: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct GapsConfig {
    #[serde(default = "default_inner_horizontal")]
    pub inner_horizontal: i32,
    #[serde(default = "default_inner_vertical")]
    pub inner_vertical: i32,
    #[serde(default = "default_outer")]
    pub outer_left: i32,
    #[serde(default = "default_outer")]
    pub outer_right: i32,
    #[serde(default = "default_outer")]
    pub outer_top: i32,
    #[serde(default = "default_outer")]
    pub outer_bottom: i32,
}

fn default_inner_horizontal() -> i32 { 10 }
fn default_inner_vertical() -> i32 { 10 }
fn default_outer() -> i32 { 10 }

impl Default for GapsConfig {
    fn default() -> Self {
        Self {
            inner_horizontal: default_inner_horizontal(),
            inner_vertical: default_inner_vertical(),
            outer_left: default_outer(),
            outer_right: default_outer(),
            outer_top: default_outer(),
            outer_bottom: default_outer(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ModeConfig {
    #[serde(default = "default_layout")]
    pub default_layout: LayoutKind,

    #[serde(default = "default_accordion_padding")]
    pub accordion_padding: i32,
}

fn default_layout() -> LayoutKind { LayoutKind::Tiles }
fn default_accordion_padding() -> i32 { 30 }

impl Default for ModeConfig {
    fn default() -> Self {
        Self {
            default_layout: default_layout(),
            accordion_padding: default_accordion_padding(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutKind {
    Tiles,
    Accordion,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct WorkspaceConfig {
    pub name: String,
    #[serde(default)]
    pub display: Option<u32>,
    #[serde(default)]
    pub auto_back_and_forth: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Keybinding {
    pub key: String,
    pub command: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct OnWindowDetected {
    pub app: Option<String>,
    pub title: Option<String>,
    pub command: String,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        if !Path::new(path).exists() {
            tracing::warn!("Config file not found at {path}, using defaults");
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {path}"))?;
        let cfg: Config = toml::from_str(&text)
            .with_context(|| format!("Failed to parse config file: {path}"))?;
        Ok(cfg)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gaps: GapsConfig::default(),
            mode: ModeConfig::default(),
            workspaces: (1..=9)
                .map(|i| WorkspaceConfig {
                    name: i.to_string(),
                    display: None,
                    auto_back_and_forth: false,
                })
                .collect(),
            keybindings: default_keybindings(),
            on_window_detected: vec![],
            exec_on_workspace_change: vec![],
        }
    }
}

fn default_keybindings() -> Vec<Keybinding> {
    vec![
        // Focus movement
        Keybinding { key: "alt-h".into(), command: "focus left".into() },
        Keybinding { key: "alt-j".into(), command: "focus down".into() },
        Keybinding { key: "alt-k".into(), command: "focus up".into() },
        Keybinding { key: "alt-l".into(), command: "focus right".into() },
        // Move windows
        Keybinding { key: "alt-shift-h".into(), command: "move left".into() },
        Keybinding { key: "alt-shift-j".into(), command: "move down".into() },
        Keybinding { key: "alt-shift-k".into(), command: "move up".into() },
        Keybinding { key: "alt-shift-l".into(), command: "move right".into() },
        // Workspace switching
        Keybinding { key: "alt-1".into(), command: "workspace 1".into() },
        Keybinding { key: "alt-2".into(), command: "workspace 2".into() },
        Keybinding { key: "alt-3".into(), command: "workspace 3".into() },
        Keybinding { key: "alt-4".into(), command: "workspace 4".into() },
        Keybinding { key: "alt-5".into(), command: "workspace 5".into() },
        Keybinding { key: "alt-6".into(), command: "workspace 6".into() },
        Keybinding { key: "alt-7".into(), command: "workspace 7".into() },
        Keybinding { key: "alt-8".into(), command: "workspace 8".into() },
        Keybinding { key: "alt-9".into(), command: "workspace 9".into() },
        // Move window to workspace
        Keybinding { key: "alt-shift-1".into(), command: "move-node-to-workspace 1".into() },
        Keybinding { key: "alt-shift-2".into(), command: "move-node-to-workspace 2".into() },
        Keybinding { key: "alt-shift-3".into(), command: "move-node-to-workspace 3".into() },
        Keybinding { key: "alt-shift-4".into(), command: "move-node-to-workspace 4".into() },
        Keybinding { key: "alt-shift-5".into(), command: "move-node-to-workspace 5".into() },
        Keybinding { key: "alt-shift-6".into(), command: "move-node-to-workspace 6".into() },
        Keybinding { key: "alt-shift-7".into(), command: "move-node-to-workspace 7".into() },
        Keybinding { key: "alt-shift-8".into(), command: "move-node-to-workspace 8".into() },
        Keybinding { key: "alt-shift-9".into(), command: "move-node-to-workspace 9".into() },
        // Layout toggling
        Keybinding { key: "alt-slash".into(), command: "layout tiles horizontal vertical".into() },
        Keybinding { key: "alt-comma".into(), command: "layout accordion horizontal vertical".into() },
        // Fullscreen
        Keybinding { key: "alt-f".into(), command: "fullscreen".into() },
        // Close
        Keybinding { key: "alt-shift-q".into(), command: "close".into() },
    ]
}
