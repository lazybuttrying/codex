use super::*;
use std::process::Command;
use tempfile::tempdir;

#[cfg(unix)]
#[test]
fn configure_trusted_git_repository_removes_abandoned_repositories() {
    use std::os::unix::ffi::OsStrExt;
    use std::time::SystemTime;

    fn age_dir(path: &Path, age: Duration) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock before epoch");
        let tv_sec = i64::try_from(now.saturating_sub(age).as_secs()).expect("timestamp range");
        let ts = libc::timespec { tv_sec, tv_nsec: 0 };
        let times = [ts, ts];
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).expect("path bytes");
        let result = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
        assert_eq!(result, 0, "{}", std::io::Error::last_os_error());
    }

    let home = tempdir().expect("tempdir");
    let staging_root = home.path().join(".tmp");
    std::fs::create_dir_all(&staging_root).expect("create staging root");

    // A process killed mid-Git leaves its GIT_DIR behind; a live one must survive the sweep.
    let abandoned = staging_root.join("git-abandoned");
    let recent = staging_root.join("git-recent");
    let unrelated = staging_root.join("plugins");
    for dir in [&abandoned, &recent, &unrelated] {
        std::fs::create_dir_all(dir).expect("create dir");
    }
    age_dir(
        &abandoned,
        TRUSTED_GIT_REPOSITORY_STALE_MAX_AGE + Duration::from_secs(60),
    );
    age_dir(&recent, Duration::ZERO);

    let mut command = Command::new("git");
    let repository =
        configure_trusted_git_repository(&mut command, home.path()).expect("configure repository");

    assert!(!abandoned.exists());
    assert!(recent.is_dir());
    assert!(unrelated.is_dir());
    assert!(repository.path().join("HEAD").is_file());
}
