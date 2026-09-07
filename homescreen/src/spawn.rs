//! Start nest widgets and host apps.

use std::ffi::OsStr;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

pub(crate) fn spawn_on_nest(command: &str, nest: &OsStr) -> std::io::Result<Child> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    apply_wayland_only(&mut cmd, nest);
    cmd.spawn()
}

/// Start `exec` on the host display and detach so it outlives this process.
pub(crate) fn launch_on_host(exec: &str, display: &OsStr) -> std::io::Result<()> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(exec);
    apply_wayland_only(&mut cmd, display);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            match libc::fork() {
                0 => Ok(()),
                -1 => Err(std::io::Error::last_os_error()),
                _ => libc::_exit(0),
            }
        });
    }
    let mut child = cmd.spawn()?;
    let _ = child.wait();
    Ok(())
}

pub(crate) fn apply_wayland_only(cmd: &mut Command, display: &OsStr) {
    cmd.env("WAYLAND_DISPLAY", display)
        .env("GDK_BACKEND", "wayland")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_SOCKET");
}

pub(crate) fn parse_desktop(path: &str) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let mut exec = String::new();
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line.eq_ignore_ascii_case("[Desktop Entry]");
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(rest) = line.strip_prefix("Exec=") {
            exec = strip_field_codes(rest);
        }
    }
    exec
}

fn strip_field_codes(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|t| !t.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ")
}
