pub mod display;
pub mod window;
pub mod workspace;

use anyhow::Result;
use tracing::{info, warn, debug};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::ipc::{IpcServer, IpcCommand};
use crate::layout::{Gaps, tree::Direction};

pub use window::Window;
pub use workspace::Workspace;
pub use display::Display;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub struct WindowManager {
    config: Config,
    workspaces: Vec<Workspace>,
    active_workspace: usize,
    displays: Vec<Display>,
    focused_window: Option<u32>,
}

impl WindowManager {
    pub fn new(config: Config) -> Result<Self> {
        let workspace_count = config.workspaces.len().max(9);
        let workspaces = (0..workspace_count)
            .map(|i| {
                let name = config.workspaces.get(i)
                    .map(|w| w.name.clone())
                    .unwrap_or_else(|| (i + 1).to_string());
                Workspace::new(name)
            })
            .collect();

        Ok(Self {
            config,
            workspaces,
            active_workspace: 0,
            displays: vec![],
            focused_window: None,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("yabai-rs starting up");

        // Discover displays
        self.displays = Display::list()?;
        info!("Found {} display(s)", self.displays.len());

        // Scan existing windows
        self.scan_windows()?;

        // Start IPC server
        let (tx, mut rx) = mpsc::channel::<IpcCommand>(64);
        let server = IpcServer::new(tx)?;
        let _server_handle = tokio::spawn(async move { server.run().await });

        info!("yabai-rs running. IPC socket at /tmp/yabai-rs.sock");

        loop {
            tokio::select! {
                Some(cmd) = rx.recv() => {
                    self.handle_command(cmd);
                }
            }
        }
    }

    fn scan_windows(&mut self) -> Result<()> {
        let windows = Window::list_all()?;
        info!("Found {} windows on startup", windows.len());
        for win in windows {
            self.workspaces[self.active_workspace].add_window(win);
        }
        self.retile();
        Ok(())
    }

    fn handle_command(&mut self, cmd: IpcCommand) {
        debug!("IPC command: {:?}", cmd);
        match cmd {
            IpcCommand::Focus(dir) => self.focus(dir),
            IpcCommand::Move(dir) => self.move_window(dir),
            IpcCommand::Workspace(name) => self.switch_workspace(&name),
            IpcCommand::MoveToWorkspace(name) => self.move_to_workspace(&name),
            IpcCommand::Layout(kind) => self.set_layout(kind),
            IpcCommand::Fullscreen => self.toggle_fullscreen(),
            IpcCommand::Close => self.close_focused(),
            IpcCommand::Reload => { let _ = self.reload_config(); }
            IpcCommand::List => self.print_state(),
        }
    }

    fn gaps(&self) -> Gaps {
        let g = &self.config.gaps;
        Gaps {
            inner_h: g.inner_horizontal,
            inner_v: g.inner_vertical,
            outer_left: g.outer_left,
            outer_right: g.outer_right,
            outer_top: g.outer_top,
            outer_bottom: g.outer_bottom,
        }
    }

    fn active_display(&self) -> Rect {
        self.displays.first()
            .map(|d| d.frame)
            .unwrap_or(Rect { x: 0, y: 0, width: 1920, height: 1080 })
    }

    pub fn retile(&mut self) {
        let area = self.active_display();
        let gaps = self.gaps();
        let ws = &mut self.workspaces[self.active_workspace];
        let frames = ws.compute_layout(area, &gaps, self.focused_window);
        for (window_id, rect) in frames {
            if let Err(e) = Window::set_frame(window_id, rect) {
                warn!("Failed to set frame for window {window_id}: {e}");
            }
        }
    }

    fn focus(&mut self, dir: Direction) {
        let ws = &self.workspaces[self.active_workspace];
        if let Some(focused) = self.focused_window {
            if let Some(target_node) = ws.tree().find_window(focused) {
                if let Some(neighbor) = ws.tree().neighbor(target_node, dir) {
                    if let Some(node) = ws.tree().get(neighbor) {
                        if let crate::layout::tree::NodeKind::Window { window_id } = node.kind {
                            let _ = Window::focus(window_id);
                            self.focused_window = Some(window_id);
                        }
                    }
                }
            }
        }
    }

    fn move_window(&mut self, dir: Direction) {
        // Swap focused window with neighbor in the given direction
        let ws = &mut self.workspaces[self.active_workspace];
        if let Some(focused) = self.focused_window {
            ws.swap_with_neighbor(focused, dir);
        }
        self.retile();
    }

    fn switch_workspace(&mut self, name: &str) {
        let idx = self.workspaces.iter().position(|w| w.name() == name);
        if let Some(idx) = idx {
            self.active_workspace = idx;
            self.retile();
            info!("Switched to workspace '{name}'");
        } else {
            warn!("No workspace named '{name}'");
        }
    }

    fn move_to_workspace(&mut self, name: &str) {
        let target_idx = self.workspaces.iter().position(|w| w.name() == name);
        if let Some(target_idx) = target_idx {
            if let Some(wid) = self.focused_window {
                let window = self.workspaces[self.active_workspace].remove_window(wid);
                if let Some(win) = window {
                    self.workspaces[target_idx].add_window(win);
                    self.retile();
                }
            }
        }
    }

    fn set_layout(&mut self, kind: crate::ipc::LayoutKind) {
        self.workspaces[self.active_workspace].set_layout(kind);
        self.retile();
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(wid) = self.focused_window {
            let area = self.active_display();
            let _ = Window::set_frame(wid, area);
        }
    }

    fn close_focused(&mut self) {
        if let Some(wid) = self.focused_window {
            let _ = Window::close(wid);
            self.workspaces[self.active_workspace].remove_window(wid);
            self.focused_window = None;
            self.retile();
        }
    }

    fn reload_config(&mut self) -> Result<()> {
        info!("Reloading config");
        // re-read from same path
        Ok(())
    }

    fn print_state(&self) {
        for (i, ws) in self.workspaces.iter().enumerate() {
            let marker = if i == self.active_workspace { "*" } else { " " };
            println!("{marker} workspace '{}': {} windows", ws.name(), ws.window_count());
        }
    }
}
