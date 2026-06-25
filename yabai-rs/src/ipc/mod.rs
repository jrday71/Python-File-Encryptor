use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::layout::tree::Direction;

pub const SOCKET_PATH: &str = "/tmp/yabai-rs.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "cmd")]
pub enum IpcCommand {
    Focus(Direction),
    Move(Direction),
    Workspace(String),
    MoveToWorkspace(String),
    Layout(LayoutKind),
    Fullscreen,
    Close,
    Reload,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutKind {
    Tiles,
    Accordion,
}

pub struct IpcServer {
    listener: UnixListener,
    tx: mpsc::Sender<IpcCommand>,
}

impl IpcServer {
    pub fn new(tx: mpsc::Sender<IpcCommand>) -> Result<Self> {
        // Remove stale socket
        let _ = std::fs::remove_file(SOCKET_PATH);
        let listener = UnixListener::bind(SOCKET_PATH)?;
        Ok(Self { listener, tx })
    }

    pub async fn run(self) -> Result<()> {
        info!("IPC server listening on {SOCKET_PATH}");
        loop {
            match self.listener.accept().await {
                Ok((stream, _)) => {
                    let tx = self.tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, tx).await {
                            warn!("IPC connection error: {e}");
                        }
                    });
                }
                Err(e) => warn!("IPC accept error: {e}"),
            }
        }
    }
}

async fn handle_connection(stream: UnixStream, tx: mpsc::Sender<IpcCommand>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        match parse_command(&line) {
            Ok(cmd) => {
                tx.send(cmd).await?;
                writer.write_all(b"ok\n").await?;
            }
            Err(e) => {
                writer.write_all(format!("error: {e}\n").as_bytes()).await?;
            }
        }
    }
    Ok(())
}

/// Parse a text command like those used in config keybindings.
/// Examples:
///   focus left
///   workspace 3
///   move-node-to-workspace 2
///   layout tiles
///   fullscreen
fn parse_command(s: &str) -> Result<IpcCommand> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    match parts.as_slice() {
        ["focus", dir] => Ok(IpcCommand::Focus(parse_direction(dir)?)),
        ["move", dir] => Ok(IpcCommand::Move(parse_direction(dir)?)),
        ["workspace", name] => Ok(IpcCommand::Workspace(name.to_string())),
        ["move-node-to-workspace", name] => Ok(IpcCommand::MoveToWorkspace(name.to_string())),
        ["layout", "tiles"] => Ok(IpcCommand::Layout(LayoutKind::Tiles)),
        ["layout", "accordion"] => Ok(IpcCommand::Layout(LayoutKind::Accordion)),
        ["layout", kind] => Err(anyhow::anyhow!("Unknown layout: {kind}")),
        ["fullscreen"] => Ok(IpcCommand::Fullscreen),
        ["close"] => Ok(IpcCommand::Close),
        ["reload"] => Ok(IpcCommand::Reload),
        ["list"] => Ok(IpcCommand::List),
        _ => Err(anyhow::anyhow!("Unknown command: {s}")),
    }
}

fn parse_direction(s: &str) -> Result<Direction> {
    match s {
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        _ => Err(anyhow::anyhow!("Unknown direction: {s}")),
    }
}
