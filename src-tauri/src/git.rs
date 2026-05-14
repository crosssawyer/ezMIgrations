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

    pub fn switch_branch(repo_path: &str, branch: &str) -> Result<(), String> {
        if Self::local_branch_exists(repo_path, branch)? {
            Self::run_git(repo_path, &["checkout", branch])?;
            return Ok(());
        }

        if Self::remote_branch_exists(repo_path, branch)? {
            let local_name = branch
                .split_once('/')
                .map(|(_, name)| name)
                .unwrap_or(branch);

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
