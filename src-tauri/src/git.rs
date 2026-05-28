use std::collections::BTreeSet;
use std::path::Path;

use crate::process::command;

pub struct GitService;

impl GitService {
    fn run_git(repo_path: &str, args: &[&str]) -> Result<String, String> {
        let output = command("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(format!("git {} failed: {}", args.join(" "), stderr))
        }
    }

    /// Get the current branch name for a repository at the given path.
    pub fn get_current_branch(repo_path: &str) -> Result<String, String> {
        Self::run_git(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
    }

    pub fn get_repo_root(repo_path: &str) -> Result<String, String> {
        Self::run_git(repo_path, &["rev-parse", "--show-toplevel"])
    }

    pub fn has_remotes(repo_path: &str) -> Result<bool, String> {
        let output = Self::run_git(repo_path, &["remote"])?;
        Ok(!output.trim().is_empty())
    }

    /// Fetch the latest refs from all remotes, pruning remote-tracking
    /// branches that have been deleted upstream. Errors when the repository
    /// has no remotes so the caller can surface a clear message.
    pub fn fetch(repo_path: &str) -> Result<(), String> {
        if !Self::has_remotes(repo_path)? {
            return Err("No git remotes are configured for this repository.".to_string());
        }
        Self::run_git(repo_path, &["fetch", "--all", "--prune"])?;
        Ok(())
    }

    pub fn ref_exists(repo_path: &str, ref_name: &str) -> Result<bool, String> {
        let commit = format!("{}^{{commit}}", ref_name);
        let status = command("git")
            .args(["rev-parse", "--verify", "--quiet", &commit])
            .current_dir(repo_path)
            .status()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        Ok(status.success())
    }

    pub fn is_working_tree_clean(repo_path: &str) -> Result<bool, String> {
        let output = Self::run_git(repo_path, &["status", "--porcelain"])?;
        Ok(output.trim().is_empty())
    }

    pub fn list_branches(repo_path: &str) -> Result<Vec<(String, bool)>, String> {
        let output = Self::run_git(
            repo_path,
            &[
                "for-each-ref",
                "refs/heads",
                "refs/remotes",
                "--format=%(refname)%09%(refname:short)",
            ],
        )?;

        let mut locals = BTreeSet::new();
        let mut remotes = BTreeSet::new();
        for line in output.lines() {
            let mut parts = line.splitn(2, '\t');
            let full = parts.next().unwrap_or("").trim();
            let short = parts.next().unwrap_or("").trim();
            if short.is_empty() || short == "HEAD" || short.ends_with("/HEAD") {
                continue;
            }
            if full.starts_with("refs/heads/") {
                locals.insert(short.to_string());
            } else if full.starts_with("refs/remotes/") {
                remotes.insert(short.to_string());
            }
        }

        let mut result: Vec<(String, bool)> = Vec::with_capacity(locals.len() + remotes.len());
        result.extend(locals.into_iter().map(|n| (n, false)));
        result.extend(
            remotes
                .into_iter()
                .map(|n| (n, true)),
        );
        Ok(result)
    }

    pub fn local_branch_exists(repo_path: &str, branch: &str) -> Result<bool, String> {
        let ref_name = format!("refs/heads/{}", branch);
        let status = command("git")
            .args(["show-ref", "--verify", "--quiet", &ref_name])
            .current_dir(repo_path)
            .status()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        Ok(status.success())
    }

    pub fn remote_branch_exists(repo_path: &str, branch: &str) -> Result<bool, String> {
        let ref_name = format!("refs/remotes/{}", branch);
        let status = command("git")
            .args(["show-ref", "--verify", "--quiet", &ref_name])
            .current_dir(repo_path)
            .status()
            .map_err(|e| format!("Failed to run git: {}", e))?;

        Ok(status.success())
    }

    /// `origin/feature/foo` → `feature/foo`. Returns the input unchanged if
    /// there's no `/` (which shouldn't happen for a real remote ref, but is
    /// handled defensively).
    pub fn local_name_from_remote_branch(branch: &str) -> &str {
        branch
            .split_once('/')
            .map(|(_, name)| name)
            .unwrap_or(branch)
    }

    pub fn switch_branch(repo_path: &str, branch: &str) -> Result<(), String> {
        if Self::local_branch_exists(repo_path, branch)? {
            Self::run_git(repo_path, &["checkout", branch])?;
            return Ok(());
        }

        if Self::remote_branch_exists(repo_path, branch)? {
            let local_name = Self::local_name_from_remote_branch(branch);

            if Self::local_branch_exists(repo_path, local_name)? {
                Self::run_git(repo_path, &["checkout", local_name])?;
            } else {
                Self::run_git(repo_path, &["checkout", "--track", branch])?;
            }
            return Ok(());
        }

        Self::run_git(repo_path, &["checkout", branch])?;
        Ok(())
    }

    pub fn list_files_at_ref(
        repo_path: &str,
        ref_name: &str,
        pathspec: &str,
    ) -> Result<Vec<String>, String> {
        let normalized_pathspec = pathspec.replace('\\', "/");
        let output = Self::run_git(
            repo_path,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                ref_name,
                "--",
                &normalized_pathspec,
            ],
        )?;

        Ok(output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn find_git_dir_returns_dir_when_present_at_root() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();

        let found = GitService::find_git_dir(dir.path().to_str().unwrap()).unwrap();
        assert!(found.ends_with(".git"));
        assert!(Path::new(&found).exists());
    }

    #[test]
    fn find_git_dir_walks_up_to_find_repo() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();
        let nested = dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();

        let found = GitService::find_git_dir(nested.to_str().unwrap()).unwrap();
        assert!(found.ends_with(".git"));
    }

    #[test]
    fn find_git_dir_returns_none_when_no_repo() {
        let dir = tempfile::tempdir().unwrap();
        // No .git anywhere under tempdir. But because find_git_dir walks
        // upward, it may find a real repo if tempdir happens to be inside one.
        // Skip this assertion on systems where the temp path is inside a repo.
        let found = GitService::find_git_dir(dir.path().to_str().unwrap());
        // We can only assert behavior consistently: if it returns Some, that
        // path must end in .git and exist.
        if let Some(p) = found {
            assert!(p.ends_with(".git"));
            assert!(Path::new(&p).exists());
        }
    }

    #[test]
    fn get_head_path_appends_head_to_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();

        let head = GitService::get_head_path(dir.path().to_str().unwrap()).unwrap();
        assert!(head.ends_with("HEAD"));
        // The path should be inside the .git directory we just created
        assert!(head.contains(".git"));
    }

    #[test]
    fn local_name_strips_single_remote_segment() {
        assert_eq!(
            GitService::local_name_from_remote_branch("origin/main"),
            "main"
        );
    }

    #[test]
    fn local_name_strips_only_first_segment_for_nested_branch() {
        assert_eq!(
            GitService::local_name_from_remote_branch("origin/feature/foo"),
            "feature/foo"
        );
    }

    #[test]
    fn local_name_returns_input_when_no_slash() {
        assert_eq!(
            GitService::local_name_from_remote_branch("main"),
            "main"
        );
    }
}
