//! yabai-rs-msg — send a command to the running yabai-rs daemon.
//!
//! Usage:
//!   yabai-rs-msg focus left
//!   yabai-rs-msg workspace 3
//!   yabai-rs-msg layout tiles

use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/yabai-rs.sock";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: yabai-rs-msg <command> [args...]");
        std::process::exit(1);
    }

    let command = args.join(" ");
    let mut stream = UnixStream::connect(SOCKET_PATH)
        .map_err(|e| anyhow::anyhow!("Cannot connect to yabai-rs socket at {SOCKET_PATH}: {e}"))?;

    stream.write_all(format!("{command}\n").as_bytes())?;

    let reader = BufReader::new(&stream);
    for line in reader.lines() {
        let line = line?;
        println!("{line}");
        if line.starts_with("ok") || line.starts_with("error") {
            break;
        }
    }
    Ok(())
}
