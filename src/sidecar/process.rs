use anyhow::{bail, Result};
use std::path::PathBuf;

pub fn find_python_path() -> PathBuf {
    // 1. Check virtualenv in project root: .venv/Scripts/python.exe or .venv/bin/python
    let venv_win = PathBuf::from(".venv").join("Scripts").join("python.exe");
    if venv_win.is_file() {
        return venv_win;
    }
    let venv_unix = PathBuf::from(".venv").join("bin").join("python");
    if venv_unix.is_file() {
        return venv_unix;
    }
    // 2. Check if VIRTUAL_ENV env var is set
    if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
        let p_win = PathBuf::from(&venv).join("Scripts").join("python.exe");
        if p_win.is_file() {
            return p_win;
        }
        let p_unix = PathBuf::from(&venv).join("bin").join("python");
        if p_unix.is_file() {
            return p_unix;
        }
    }
    // 3. Fallback to system python
    PathBuf::from(if cfg!(windows) { "python.exe" } else { "python3" })
}

pub fn find_sidecar_script() -> Result<PathBuf> {
    // Check relative to cwd
    let candidate = PathBuf::from("sidecar").join("embed.py");
    if candidate.is_file() {
        return Ok(candidate);
    }
    // Check relative to current exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let c2 = parent.join("../../sidecar/embed.py");
            if c2.is_file() {
                return Ok(c2);
            }
            let c3 = parent.join("sidecar/embed.py");
            if c3.is_file() {
                return Ok(c3);
            }
        }
    }
    bail!("Could not find sidecar/embed.py in current working directory or binary directory");
}
