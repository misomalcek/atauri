use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: String,
    pub modified: Vec<String>,
    pub staged: Vec<String>,
    pub untracked: Vec<String>,
    pub ahead: u32,
    pub behind: u32,
    pub is_clean: bool,
    pub has_remote: bool,
    pub remote_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogEntry {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiff {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub diff_text: String,
}

/// Run a git command in the given directory
fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "Git command failed".to_string()
        } else {
            stderr
        })
    }
}

/// Check if directory is a git repository
pub fn is_git_repo(root: &Path) -> bool {
    run_git(root, &["rev-parse", "--is-inside-work-tree"]).is_ok()
}

/// Get comprehensive git status
pub fn status(root: &Path) -> Result<GitStatus, String> {
    if !is_git_repo(root) {
        return Err("Not a git repository".into());
    }

    // Current branch
    let branch = run_git(root, &["branch", "--show-current"])
        .unwrap_or_else(|_| "HEAD".into());

    // Porcelain status for structured parsing
    let porcelain = run_git(root, &["status", "--porcelain"])?;

    let mut modified = Vec::new();
    let mut staged = Vec::new();
    let mut untracked = Vec::new();

    for line in porcelain.lines() {
        if line.len() < 4 { continue; }
        let index = line.chars().nth(0).unwrap_or(' ');
        let worktree = line.chars().nth(1).unwrap_or(' ');
        let file = line[3..].to_string();

        if index == '?' {
            untracked.push(file);
        } else {
            if index != ' ' && index != '?' {
                staged.push(file.clone());
            }
            if worktree != ' ' && worktree != '?' {
                modified.push(file);
            }
        }
    }

    // Ahead/behind
    let (ahead, behind) = get_ahead_behind(root);

    // Remote URL
    let remote_url = run_git(root, &["remote", "get-url", "origin"])
        .unwrap_or_default();
    let has_remote = !remote_url.is_empty();

    let is_clean = modified.is_empty() && staged.is_empty() && untracked.is_empty();

    Ok(GitStatus {
        branch,
        modified,
        staged,
        untracked,
        ahead,
        behind,
        is_clean,
        has_remote,
        remote_url,
    })
}

fn get_ahead_behind(root: &Path) -> (u32, u32) {
    let output = run_git(root, &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"]);
    match output {
        Ok(text) => {
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() == 2 {
                let ahead = parts[0].parse().unwrap_or(0);
                let behind = parts[1].parse().unwrap_or(0);
                (ahead, behind)
            } else {
                (0, 0)
            }
        }
        Err(_) => (0, 0),
    }
}

/// Get git log entries
pub fn log(root: &Path, count: u32) -> Result<Vec<GitLogEntry>, String> {
    let format = "%H|%h|%s|%an|%ci";
    let output = run_git(root, &[
        "log",
        &format!("--format={format}"),
        &format!("-n{count}"),
    ])?;

    let entries = output.lines().filter_map(|line| {
        let parts: Vec<&str> = line.splitn(5, '|').collect();
        if parts.len() == 5 {
            Some(GitLogEntry {
                hash: parts[0].to_string(),
                short_hash: parts[1].to_string(),
                message: parts[2].to_string(),
                author: parts[3].to_string(),
                date: parts[4].to_string(),
            })
        } else {
            None
        }
    }).collect();

    Ok(entries)
}

/// Get git diff
pub fn diff(root: &Path, staged: bool) -> Result<GitDiff, String> {
    let args = if staged {
        vec!["diff", "--staged"]
    } else {
        vec!["diff"]
    };

    let diff_text = run_git(root, &args)?;

    // Get stat
    let stat_args = if staged {
        vec!["diff", "--staged", "--stat"]
    } else {
        vec!["diff", "--stat"]
    };
    let stat = run_git(root, &stat_args).unwrap_or_default();

    // Parse stat summary line (e.g., "3 files changed, 10 insertions(+), 5 deletions(-)")
    let mut files_changed = 0u32;
    let mut insertions = 0u32;
    let mut deletions = 0u32;

    if let Some(summary) = stat.lines().last() {
        for part in summary.split(',') {
            let part = part.trim();
            if part.contains("file") {
                files_changed = part.split_whitespace().next()
                    .and_then(|n| n.parse().ok()).unwrap_or(0);
            } else if part.contains("insertion") {
                insertions = part.split_whitespace().next()
                    .and_then(|n| n.parse().ok()).unwrap_or(0);
            } else if part.contains("deletion") {
                deletions = part.split_whitespace().next()
                    .and_then(|n| n.parse().ok()).unwrap_or(0);
            }
        }
    }

    Ok(GitDiff {
        files_changed,
        insertions,
        deletions,
        diff_text,
    })
}

/// Stage specific files
pub fn add(root: &Path, files: &[String]) -> Result<String, String> {
    if files.is_empty() {
        return run_git(root, &["add", "-A"]);
    }
    let mut args = vec!["add"];
    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    args.extend(file_refs);
    run_git(root, &args)
}

/// Commit with message (stages all changes first)
pub fn commit(root: &Path, message: &str) -> Result<String, String> {
    // Stage all changes
    run_git(root, &["add", "-A"])?;
    run_git(root, &["commit", "-m", message])
}

/// Push to remote
pub fn push(root: &Path) -> Result<String, String> {
    run_git(root, &["push"])
}

/// Pull from remote
pub fn pull(root: &Path) -> Result<String, String> {
    run_git(root, &["pull"])
}
