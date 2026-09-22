use crate::stale_temp_dirs::remove_stale_temp_dirs;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

const TRUSTED_GIT_REPOSITORY_PREFIX: &str = "git-";
// Not every caller bounds its Git command -- `loader::run_git_output` clones without a timeout --
// so this stays far above any plausible clone rather than just above the timeouts we do set.
// Abandoned repositories survive for months, so a day costs nothing to reclaim.
const TRUSTED_GIT_REPOSITORY_STALE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

// These variables can redirect Git to an untrusted repository or inject command-scoped settings.
pub(crate) const REPOSITORY_LOCAL_GIT_ENVIRONMENT_VARIABLES: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CEILING_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIR",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_GRAFT_FILE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_NAMESPACE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_PREFIX",
    "GIT_REPLACE_REF_BASE",
    "GIT_SHALLOW_FILE",
    "GIT_WORK_TREE",
];

/// Distinguishes isolated background Git operations from explicitly requested user operations.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PluginGitMode {
    /// Ignores repository-scoped Git configuration inherited from the launch project.
    Automatic,
    /// Preserves the configuration explicitly available to user-requested Git commands.
    Manual,
}

impl PluginGitMode {
    /// Creates a Git command with the repository-environment policy for this operation.
    /// Automatic remote lookups and initial clones must also select a trusted repository.
    pub(crate) fn command(self, git_binary: &Path) -> Command {
        let mut command = Command::new(git_binary);
        command.args(["-c", codex_git_utils::SAFE_BARE_REPOSITORY_CONFIG]);
        if matches!(self, Self::Automatic) {
            command.env("GIT_OPTIONAL_LOCKS", "0");
            for name in REPOSITORY_LOCAL_GIT_ENVIRONMENT_VARIABLES {
                command.env_remove(name);
            }
        }
        command
    }
}

/// Isolates an automatic Git command from repository discovery in the launch project.
///
/// The command must already have repository-scoped environment variables removed. Keep the
/// returned guard alive until the Git process exits because it owns the selected repository.
pub(crate) fn configure_trusted_git_repository(
    command: &mut Command,
    codex_home: &Path,
) -> Result<TempDir, String> {
    let canonical_home = AbsolutePathBuf::from_absolute_path(codex_home)
        .and_then(|path| path.canonicalize())
        .map_err(|err| format!("failed to resolve trusted Codex home: {err}"))?;
    let staging_root = canonical_home.join(".tmp");
    std::fs::create_dir_all(staging_root.as_path())
        .map_err(|err| format!("failed to create trusted Git repository root: {err}"))?;
    let staging_root = staging_root
        .canonicalize()
        .map_err(|err| format!("failed to resolve trusted Git repository root: {err}"))?;
    if !staging_root.as_path().starts_with(canonical_home.as_path()) {
        return Err("trusted Git repository root escapes Codex home".to_string());
    }

    remove_stale_temp_dirs(
        staging_root.as_path(),
        TRUSTED_GIT_REPOSITORY_PREFIX,
        TRUSTED_GIT_REPOSITORY_STALE_MAX_AGE,
    );

    let repository = tempfile::Builder::new()
        .prefix(TRUSTED_GIT_REPOSITORY_PREFIX)
        .tempdir_in(staging_root.as_path())
        .map_err(|err| format!("failed to create trusted Git repository: {err}"))?;
    for directory in ["objects", "refs"] {
        std::fs::create_dir(repository.path().join(directory))
            .map_err(|err| format!("failed to initialize trusted Git repository: {err}"))?;
    }
    std::fs::write(repository.path().join("HEAD"), "ref: refs/heads/main\n")
        .map_err(|err| format!("failed to initialize trusted Git repository HEAD: {err}"))?;
    command.env("GIT_DIR", repository.path());
    Ok(repository)
}

#[cfg(test)]
#[path = "git_policy_tests.rs"]
mod tests;
