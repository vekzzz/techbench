//! The real runner for Ookla's `speedtest.exe`. It never passes `--accept-license` or
//! `--accept-gdpr`: the Technician accepts Ookla's terms themselves, once, in a terminal.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::network::OoklaFailure;

/// Keeps a console window from flashing up when the app starts the program.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Where Ookla's program is, if it is installed: on the PATH or in WinGet's links folder. A
/// Python `speedtest.exe` (speedtest-cli) shares the name, so the program must say it is Ookla's.
async fn find() -> Option<PathBuf> {
    let mut folders: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        folders.push(
            PathBuf::from(local)
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
        );
    }
    for folder in folders {
        let candidate = folder.join("speedtest.exe");
        if candidate.is_file() && says_ookla(&candidate).await {
            return Some(candidate);
        }
    }
    None
}

async fn says_ookla(program: &PathBuf) -> bool {
    let mut command = Command::new(program);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    match tokio::time::timeout(Duration::from_secs(5), command.output()).await {
        Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout).contains("Ookla"),
        _ => false,
    }
}

pub(crate) async fn installed() -> bool {
    find().await.is_some()
}

/// Runs the program, handing `on_line` each line it prints. Dropping the future ends the program.
pub(crate) async fn run(on_line: &(dyn Fn(String) + Send + Sync)) -> Result<(), OoklaFailure> {
    let program = find().await.ok_or(OoklaFailure::NotInstalled)?;
    let mut command = Command::new(program);
    command
        .args(["-f", "jsonl"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command.spawn().map_err(|error| OoklaFailure::Failed {
        message: format!("speedtest.exe could not start ({error})"),
    })?;

    let mut stderr = child.stderr.take().expect("stderr is piped");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout is piped")).lines();
    // What the program says that is not a JSON line, such as the licence prompt.
    let mut said = Vec::new();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim_start().starts_with('{') {
            on_line(line);
        } else if !line.trim().is_empty() {
            said.push(line.trim().to_string());
        }
    }
    let mut errors = String::new();
    let _ = stderr.read_to_string(&mut errors).await;
    let status = child.wait().await.map_err(|error| OoklaFailure::Failed {
        message: format!("speedtest.exe did not finish ({error})"),
    })?;
    if status.success() {
        return Ok(());
    }

    said.extend(
        errors
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty()),
    );
    let tail: Vec<&str> = said
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(String::as_str)
        .collect();
    let mut message = if tail.is_empty() {
        format!("speedtest.exe stopped with {status}")
    } else {
        tail.join(" ")
    };
    if message.to_lowercase().contains("licen") {
        message.push_str(
            " Run speedtest.exe once in a terminal and accept its licence, then try again.",
        );
    }
    Err(OoklaFailure::Failed { message })
}
