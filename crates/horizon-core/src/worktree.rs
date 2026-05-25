use std::fs;
use std::path::{Path, PathBuf};

use git2::{
    ApplyLocation, BranchType, Diff, DiffFormat, DiffOptions, Repository, WorktreeAddOptions, WorktreePruneOptions,
};

use crate::error::{Error, Result};

const WORKTREE_NAME_PREFIX: &str = "horizon-squad";

/// Git worktree helpers used by Agent Squad isolation.
pub struct WorktreeManager;

impl WorktreeManager {
    /// Create a performer worktree under `scratch_root` for `slot_id`.
    ///
    /// The new worktree is checked out on a dedicated local branch created from
    /// `base_ref`. Pass `HEAD` for the current checkout state.
    ///
    /// # Errors
    ///
    /// Returns an error if `repo` is not inside a Git repository, `base_ref`
    /// cannot be resolved to a commit, the slot path already exists, or libgit2
    /// cannot create the branch/worktree.
    pub fn create(repo: &Path, base_ref: &str, scratch_root: &Path, slot_id: &str) -> Result<PathBuf> {
        let repository = open_repo(repo)?;
        fs::create_dir_all(scratch_root)?;

        let worktree_path = scratch_root.join(path_component(slot_id));
        if worktree_path.exists() {
            return Err(Error::Git(format!(
                "worktree path already exists: {}",
                worktree_path.display()
            )));
        }

        let branch_name = worktree_name(scratch_root, slot_id);
        let base_commit = resolve_commit(&repository, base_ref)?;
        let branch = repository
            .branch(&branch_name, &base_commit, false)
            .map_err(|error| git_error("create worktree branch", &error))?;
        let branch_ref = branch.into_reference();

        let mut options = WorktreeAddOptions::new();
        options.reference(Some(&branch_ref));

        match repository.worktree(&branch_name, &worktree_path, Some(&options)) {
            Ok(_) => Ok(worktree_path),
            Err(error) => {
                remove_branch(&repository, &branch_name);
                Err(git_error("create worktree", &error))
            }
        }
    }

    /// Remove a worktree directory and prune its Git metadata.
    ///
    /// Missing paths are treated as already removed.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` exists but is not an openable worktree, or if
    /// libgit2 cannot prune the worktree metadata and working tree.
    pub fn remove(path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let repository = Repository::open(path)
            .or_else(|_| Repository::discover(path))
            .map_err(|error| {
                Error::Git(format!(
                    "open worktree repository at {}: {}",
                    path.display(),
                    error.message()
                ))
            })?;
        let worktree = git2::Worktree::open_from_repository(&repository)
            .map_err(|error| git_error("open worktree metadata", &error))?;
        let mut options = WorktreePruneOptions::new();
        options.valid(true).working_tree(true);
        worktree
            .prune(Some(&mut options))
            .map_err(|error| git_error("prune worktree", &error))
    }

    /// Return the unified diff for changes in a worktree.
    ///
    /// The diff includes staged, unstaged, and untracked files relative to
    /// `HEAD`.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` is not inside a Git repository or if libgit2
    /// cannot compute or format the diff.
    pub fn diff(path: &Path) -> Result<String> {
        let repository = open_repo(path)?;
        let head_tree = repository.head().ok().and_then(|head| head.peel_to_tree().ok());

        let mut options = DiffOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .show_untracked_content(true);

        let diff = repository
            .diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut options))
            .map_err(|error| git_error("compute worktree diff", &error))?;
        format_diff(&diff)
    }

    /// Apply a unified diff to `dest` without staging the result.
    ///
    /// Empty diffs are accepted and leave `dest` unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if `dest` is not inside a Git repository, the diff
    /// cannot be parsed, or libgit2 cannot apply it cleanly.
    pub fn apply_to(diff: &str, dest: &Path) -> Result<()> {
        if diff.trim().is_empty() {
            return Ok(());
        }

        let repository = open_repo(dest)?;
        let parsed = Diff::from_buffer(diff.as_bytes()).map_err(|error| git_error("parse diff", &error))?;
        repository
            .apply(&parsed, ApplyLocation::WorkDir, None)
            .map_err(|error| git_error("apply diff", &error))
    }
}

fn open_repo(path: &Path) -> Result<Repository> {
    Repository::discover(path).map_err(|error| {
        Error::Git(format!(
            "discover repository at {}: {}",
            path.display(),
            error.message()
        ))
    })
}

fn resolve_commit<'repo>(repository: &'repo Repository, base_ref: &str) -> Result<git2::Commit<'repo>> {
    let reference = if base_ref.trim().is_empty() { "HEAD" } else { base_ref };

    repository
        .revparse_single(reference)
        .and_then(|object| object.peel_to_commit())
        .map_err(|error| Error::Git(format!("resolve base ref {reference}: {}", error.message())))
}

fn format_diff(diff: &Diff<'_>) -> Result<String> {
    let mut output = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        if matches!(line.origin(), ' ' | '+' | '-') {
            output.push(line.origin());
        }
        output.push_str(&String::from_utf8_lossy(line.content()));
        true
    })
    .map_err(|error| git_error("format worktree diff", &error))?;
    Ok(output)
}

fn remove_branch(repository: &Repository, branch_name: &str) {
    if let Ok(mut branch) = repository.find_branch(branch_name, BranchType::Local) {
        let _ = branch.delete();
    }
}

fn worktree_name(scratch_root: &Path, slot_id: &str) -> String {
    let run = scratch_root
        .file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| "run".to_string(), path_component);
    let slot = path_component(slot_id);
    let suffix = stable_path_suffix(scratch_root);
    format!("{WORKTREE_NAME_PREFIX}-{run}-{slot}-{suffix:08x}")
}

fn path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "slot".to_string()
    } else {
        trimmed.to_string()
    }
}

fn stable_path_suffix(path: &Path) -> u32 {
    let mut hash = 0x811c_9dc5_u32;
    for byte in path.as_os_str().to_string_lossy().as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn git_error(context: &str, error: &git2::Error) -> Error {
    Error::Git(format!("{context}: {}", error.message()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use git2::{IndexAddOption, Repository, Signature};
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn create_checks_out_base_ref_in_slot_worktree() {
        let fixture = TestRepo::new();
        let scratch_root = fixture.root.path().join("squad").join("run-a");

        let worktree_path = WorktreeManager::create(fixture.repo_root(), "HEAD", &scratch_root, "s1").unwrap();

        assert_eq!(worktree_path, scratch_root.join("s1"));
        assert_eq!(fs::read_to_string(worktree_path.join("README.md")).unwrap(), "base\n");

        let worktree_repo = Repository::open(&worktree_path).unwrap();
        assert!(
            worktree_repo
                .head()
                .unwrap()
                .shorthand()
                .unwrap()
                .starts_with("horizon-squad-run-a-s1-")
        );
    }

    #[test]
    fn diff_and_apply_to_move_slot_changes_into_review_worktree() {
        let fixture = TestRepo::new();
        let scratch_root = fixture.root.path().join("squad").join("run-b");
        let source = WorktreeManager::create(fixture.repo_root(), "HEAD", &scratch_root, "s1").unwrap();
        let review = WorktreeManager::create(fixture.repo_root(), "HEAD", &scratch_root, "_review").unwrap();

        fs::write(source.join("README.md"), "changed\n").unwrap();
        fs::write(source.join("new-file.txt"), "new\n").unwrap();

        let diff = WorktreeManager::diff(&source).unwrap();
        assert!(diff.contains("+changed"));
        assert!(diff.contains("new-file.txt"));

        WorktreeManager::apply_to(&diff, &review).unwrap();

        assert_eq!(fs::read_to_string(review.join("README.md")).unwrap(), "changed\n");
        assert_eq!(fs::read_to_string(review.join("new-file.txt")).unwrap(), "new\n");
    }

    #[test]
    fn remove_prunes_worktree_directory() {
        let fixture = TestRepo::new();
        let scratch_root = fixture.root.path().join("squad").join("run-c");
        let worktree_path = WorktreeManager::create(fixture.repo_root(), "HEAD", &scratch_root, "s1").unwrap();

        assert!(worktree_path.exists());

        WorktreeManager::remove(&worktree_path).unwrap();

        assert!(!worktree_path.exists());
    }

    #[test]
    fn empty_diff_applies_as_noop() {
        let fixture = TestRepo::new();
        let scratch_root = fixture.root.path().join("squad").join("run-d");
        let review = WorktreeManager::create(fixture.repo_root(), "HEAD", &scratch_root, "_review").unwrap();

        WorktreeManager::apply_to("", &review).unwrap();

        assert_eq!(fs::read_to_string(review.join("README.md")).unwrap(), "base\n");
    }

    struct TestRepo {
        root: TempDir,
        repo: Repository,
    }

    impl TestRepo {
        fn new() -> Self {
            let root = TempDir::new().unwrap();
            let repo = Repository::init(root.path()).unwrap();
            fs::write(root.path().join("README.md"), "base\n").unwrap();
            commit_all(&repo, "initial");
            Self { root, repo }
        }

        fn repo_root(&self) -> &Path {
            self.repo.workdir().unwrap()
        }
    }

    fn commit_all(repo: &Repository, message: &str) {
        let mut index = repo.index().unwrap();
        index.add_all(["*"], IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = Signature::now("Horizon Test", "horizon@example.invalid").unwrap();
        repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])
            .unwrap();
    }
}
