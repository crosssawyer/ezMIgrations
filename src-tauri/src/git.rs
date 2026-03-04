use std::path::Path;

use crate::process::command;

pub struct GitService;

impl GitService {
    /// Get the current branch name for a repository at the given path.
    pub fn get_current_branch(repo_path: &str) -> Result<String, String> {
        let output = command("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(format!(
                "git rev-parse failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Find the .git directory for a given path (walks up to find it).
    pub fn find_git_dir(start_path: &str) -> Option<String> {
        let mut current = Path::new(start_path).to_path_buf();
        loop {
            let git_dir = current.join(".git");
            if git_dir.exists() {
                return Some(git_dir.to_string_lossy().to_string());
            }
            if !current.pop() {
                return None;
            }
        }
    }

    /// Get the git HEAD file path for watching branch changes.
    pub fn get_head_path(repo_path: &str) -> Option<String> {
        Self::find_git_dir(repo_path).map(|git_dir| {
            Path::new(&git_dir)
                .join("HEAD")
                .to_string_lossy()
                .to_string()
        })
    }

    /// Get list of files that changed between two branches (migration-relevant files only).
    pub fn get_migration_diff(
        repo_path: &str,
        from_branch: &str,
        to_branch: &str,
    ) -> Result<Vec<String>, String> {
        let range = format!("{}...{}", from_branch, to_branch);
        let output = command("git")
            .args(["diff", "--name-only", &range, "--", "*/Migrations/*"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("Failed to run git diff: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect())
        } else {
            // Non-fatal: just return empty if comparison fails
            Ok(Vec::new())
        }
    }
}
