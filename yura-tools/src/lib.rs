//! # yura-tools
//!
//! The actions Yura can take on the user's machine, and the walls around them.
//!
//! Two rules decide everything in this crate:
//!
//! * **One folder.** Every file operation resolves against a single workspace
//!   root, canonicalised, and is refused if the real path escapes it. Symlinks
//!   are resolved before the check, not after.
//! * **Nothing runs silently.** [`run_python`] is a pure function that executes
//!   a script and returns what happened; the decision to run it at all belongs to
//!   the caller, which is where the approval gate lives.
//!
//! There is no sandbox here beyond the workspace and the timeout. That is stated
//! plainly rather than implied, because a user turning on "Run Python" deserves
//! to know it is their own interpreter, with their own permissions, and that the
//! only guard rails are the folder, the clock and the approval prompt.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("path is outside the workspace: {0}")]
    Outside(String),

    #[error("no workspace folder has been chosen")]
    NoWorkspace,

    #[error("python is not installed or not on PATH")]
    NoPython,

    #[error("the script timed out after {0} seconds")]
    Timeout(u64),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("process error: {0}")]
    Process(String),
}

pub type Result<T, E = ToolError> = std::result::Result<T, E>;

/* ── workspace ─────────────────────────────────────────────────── */

/// A single folder the user granted. All paths are relative to it.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
    pub bytes: u64,
}

impl Workspace {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        if root.as_os_str().is_empty() {
            return Err(ToolError::NoWorkspace);
        }
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a user-supplied relative path, refusing anything that escapes.
    ///
    /// Two checks, both needed: reject `..` components *lexically* (so a
    /// non-existent escape is still refused), then canonicalise the parent so a
    /// symlink pointing outside cannot smuggle a write out of the folder.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf> {
        let rel = rel.trim().trim_start_matches('/').trim_start_matches('\\');
        let candidate = Path::new(rel);

        for component in candidate.components() {
            match component {
                Component::ParentDir => return Err(ToolError::Outside(rel.to_owned())),
                Component::Prefix(_) | Component::RootDir => {
                    return Err(ToolError::Outside(rel.to_owned()))
                }
                _ => {}
            }
        }

        let joined = self.root.join(candidate);

        // The file may not exist yet; canonicalise the deepest existing ancestor.
        let mut anchor = joined.clone();
        while !anchor.exists() {
            match anchor.parent() {
                Some(parent) => anchor = parent.to_path_buf(),
                None => break,
            }
        }
        let real_anchor = anchor.canonicalize()?;
        if !real_anchor.starts_with(&self.root) {
            return Err(ToolError::Outside(rel.to_owned()));
        }

        Ok(joined)
    }

    pub fn list(&self) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            let name = entry.file_name().to_string_lossy().to_string();
            out.push(Entry {
                path: name.clone(),
                name,
                dir: meta.is_dir(),
                bytes: meta.len(),
            });
        }
        out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.cmp(&b.name)));
        Ok(out)
    }

    pub fn read(&self, rel: &str) -> Result<String> {
        let path = self.resolve(rel)?;
        let bytes = std::fs::read(&path)?;
        if bytes.len() > 4 * 1024 * 1024 {
            let head = String::from_utf8_lossy(&bytes[..4 * 1024 * 1024]).to_string();
            return Ok(format!("{head}\n[…truncated at 4 MB]"));
        }
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    pub fn write(&self, rel: &str, contents: &str) -> Result<PathBuf> {
        let path = self.resolve(rel)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
        Ok(path)
    }
}

/* ── python ────────────────────────────────────────────────────── */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
}

/// Locate a Python interpreter. Windows first, then the POSIX names.
pub fn python_command() -> Option<&'static str> {
    const CANDIDATES: [&str; 3] = ["python", "python3", "py"];
    for candidate in CANDIDATES {
        let probe = if cfg!(windows) {
            std::process::Command::new("where").arg(candidate).output()
        } else {
            std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("command -v {candidate}"))
                .output()
        };
        if let Ok(out) = probe {
            if out.status.success() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Run a script in the workspace.
///
/// `-I` runs in isolated mode so the user's `PYTHONPATH`, `PYTHONSTARTUP` and
/// site-packages cannot leak into the run — the same script should behave the
/// same way tomorrow. Output is capped so a runaway loop cannot wedge the app.
pub fn run_python(workspace: &Path, code: &str, timeout: Duration) -> Result<RunResult> {
    let python = python_command().ok_or(ToolError::NoPython)?;
    let started = Instant::now();

    let mut child = std::process::Command::new(python)
        .arg("-I")
        .arg("-c")
        .arg(code)
        .current_dir(workspace)
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Poll rather than block, so the timeout is real on every platform.
    let deadline = started + timeout;
    loop {
        match child.try_wait()? {
            Some(status) => {
                let output = child.wait_with_output()?;
                return Ok(RunResult {
                    stdout: cap(&String::from_utf8_lossy(&output.stdout)),
                    stderr: cap(&String::from_utf8_lossy(&output.stderr)),
                    code: status.code(),
                    duration_ms: started.elapsed().as_millis() as u64,
                    timed_out: false,
                });
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let output = child.wait_with_output()?;
                return Ok(RunResult {
                    stdout: cap(&String::from_utf8_lossy(&output.stdout)),
                    stderr: cap(&String::from_utf8_lossy(&output.stderr)),
                    code: None,
                    duration_ms: started.elapsed().as_millis() as u64,
                    timed_out: true,
                });
            }
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

fn cap(text: &str) -> String {
    const MAX: usize = 8_000;
    if text.chars().count() <= MAX {
        return text.to_owned();
    }
    let head: String = text.chars().take(MAX).collect();
    format!("{head}\n[…output truncated at {MAX} characters]")
}

/* ── timers & tasks ───────────────────────────────────────────── */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timer {
    pub id: String,
    pub label: String,
    /// Unix milliseconds.
    pub due: u64,
    pub fired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub text: String,
    pub done: bool,
    pub created: u64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Timers and a to-do list, held in one small JSON file.
///
/// A companion that forgets your reminder when the window closes is worse than
/// no reminder, so these persist; firing is the scheduler's job in the app.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Agenda {
    #[serde(default)]
    pub timers: Vec<Timer>,
    #[serde(default)]
    pub tasks: Vec<Task>,
}

impl Agenda {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(|e| ToolError::Process(e.to_string()))?;
        std::fs::write(path, body)?;
        Ok(())
    }

    pub fn add_timer(&mut self, label: impl Into<String>, seconds: u64) -> Timer {
        let timer = Timer {
            id: format!("t{:x}", now_ms()),
            label: label.into(),
            due: now_ms() + seconds.saturating_mul(1000),
            fired: false,
        };
        self.timers.push(timer.clone());
        timer
    }

    pub fn add_task(&mut self, text: impl Into<String>) -> Task {
        let task = Task {
            id: format!("k{:x}", now_ms()),
            text: text.into(),
            done: false,
            created: now_ms(),
        };
        self.tasks.push(task.clone());
        task
    }

    pub fn due_now(&mut self) -> Vec<Timer> {
        let now = now_ms();
        let mut fired = Vec::new();
        for timer in &mut self.timers {
            if !timer.fired && timer.due <= now {
                timer.fired = true;
                fired.push(timer.clone());
            }
        }
        fired
    }

    pub fn prune(&mut self) {
        self.timers.retain(|t| !t.fired);
    }

    /// Prompt-ready summary of what is pending.
    pub fn summary(&self) -> String {
        let open: Vec<&Task> = self.tasks.iter().filter(|t| !t.done).collect();
        if open.is_empty() {
            return "No open tasks.".to_owned();
        }
        let mut out = String::from("Open tasks:\n");
        for task in open {
            out.push_str(&format!("- [{}] {}\n", task.id, task.text));
        }
        out
    }

    pub fn by_id(&self) -> HashMap<&str, &Task> {
        self.tasks.iter().map(|t| (t.id.as_str(), t)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(name: &str) -> Workspace {
        let dir = std::env::temp_dir().join(format!("yura-ws-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Workspace::new(dir).unwrap()
    }

    #[test]
    fn refuses_parent_traversal() {
        let ws = ws("traversal");
        assert!(matches!(ws.resolve("../secrets.txt"), Err(ToolError::Outside(_))));
        assert!(matches!(ws.resolve("/etc/passwd"), Err(ToolError::Outside(_))));
        assert!(matches!(ws.resolve("a/../../b"), Err(ToolError::Outside(_))));
    }

    #[test]
    fn allows_paths_inside() {
        let ws = ws("inside");
        let path = ws.resolve("notes/lecture 1.md").unwrap();
        assert!(path.starts_with(ws.root()));
    }

    #[test]
    fn writes_and_reads_back() {
        let ws = ws("rw");
        ws.write("notes/a.md", "hello").unwrap();
        assert_eq!(ws.read("notes/a.md").unwrap(), "hello");
        let listed = ws.list().unwrap();
        assert!(listed.iter().any(|e| e.name == "notes" && e.dir));
    }

    #[test]
    fn agenda_round_trips() {
        let dir = std::env::temp_dir().join(format!("yura-agenda-{}", std::process::id()));
        let path = dir.join("agenda.json");
        let mut agenda = Agenda::default();
        agenda.add_timer("water", 60);
        agenda.add_task("read chapter 4");
        agenda.save(&path).unwrap();

        let loaded = Agenda::load(&path);
        assert_eq!(loaded.timers.len(), 1);
        assert_eq!(loaded.tasks.len(), 1);
        assert!(loaded.summary().contains("read chapter 4"));
    }

    #[test]
    fn timers_fire_once() {
        let mut agenda = Agenda::default();
        let mut timer = agenda.add_timer("now", 0);
        timer.due = 0;
        agenda.timers[0] = timer;

        assert_eq!(agenda.due_now().len(), 1);
        assert_eq!(agenda.due_now().len(), 0, "must not fire twice");
    }

    #[test]
    fn output_is_capped() {
        let long = "x".repeat(20_000);
        let capped = cap(&long);
        assert!(capped.contains("truncated"));
        assert!(capped.chars().count() < 9_000);
    }

    #[test]
    fn python_runs_when_available() {
        if python_command().is_none() {
            return; // environment without python; nothing to assert
        }
        let ws = ws("python");
        let result = run_python(ws.root(), "print(2 + 2)", Duration::from_secs(10)).unwrap();
        assert!(!result.timed_out);
        assert_eq!(result.stdout.trim(), "4");
    }
}
