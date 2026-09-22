use std::path::Path;
use std::time::Duration;
use tracing::warn;

/// Removes abandoned temporary directories left in `parent` by earlier Codex processes.
///
/// Temporary directories under `$CODEX_HOME/.tmp` are normally released by their `TempDir`
/// guard, but a killed or timed-out process never runs the destructor, so the directory
/// survives forever. Sweeping by age on the next run bounds that growth.
pub(crate) fn remove_stale_temp_dirs(parent: &Path, prefix: &str, max_age: Duration) {
    let entries = match std::fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(err) => {
            warn!(
                error = %err,
                parent = %parent.display(),
                prefix,
                "failed to list temp directory parent for stale cleanup"
            );
            return;
        }
    };

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                warn!(
                    error = %err,
                    path = %entry.path().display(),
                    "failed to inspect temp directory entry"
                );
                continue;
            }
        };
        if !file_type.is_dir() {
            continue;
        }

        let path = entry.path();
        let matches_prefix = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(prefix));
        if !matches_prefix {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(err) => {
                warn!(
                    error = %err,
                    path = %path.display(),
                    "failed to read temp directory metadata"
                );
                continue;
            }
        };
        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(err) => {
                warn!(
                    error = %err,
                    path = %path.display(),
                    "failed to read temp directory modification time"
                );
                continue;
            }
        };
        let age = match modified.elapsed() {
            Ok(age) => age,
            Err(err) => {
                warn!(
                    error = %err,
                    path = %path.display(),
                    "failed to compute temp directory age"
                );
                continue;
            }
        };
        if age < max_age {
            continue;
        }

        if let Err(err) = std::fs::remove_dir_all(&path) {
            warn!(
                error = %err,
                path = %path.display(),
                "failed to remove stale temp directory"
            );
        }
    }
}
