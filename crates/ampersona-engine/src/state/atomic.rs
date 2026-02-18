use anyhow::{bail, Context, Result};
use std::io::Write;

/// Write content atomically: write to temp file, fsync, rename.
pub fn atomic_write(path: &str, content: &[u8]) -> Result<()> {
    let dir = std::path::Path::new(path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    let temp_path = dir.join(format!(".{}.tmp", uuid_v4_simple()));

    let mut file = std::fs::File::create(&temp_path)
        .with_context(|| format!("cannot create temp file for {path}"))?;
    file.write_all(content)?;
    file.sync_all()?;
    drop(file);

    std::fs::rename(&temp_path, path).with_context(|| format!("cannot rename temp to {path}"))?;

    Ok(())
}

/// Advisory lock for state files. Prevents concurrent writers.
///
/// Creates a .lock file alongside the state file.
/// The lock file contains the PID and timestamp.
pub struct AdvisoryLock {
    lock_path: String,
}

impl AdvisoryLock {
    /// Acquire an advisory lock. Returns error if lock is already held.
    pub fn acquire(state_path: &str) -> Result<Self> {
        let lock_path = format!("{state_path}.lock");

        // Check if lock already exists and is stale
        if std::path::Path::new(&lock_path).exists() {
            let content = std::fs::read_to_string(&lock_path).unwrap_or_default();
            if let Some(ts_str) = content.lines().nth(1) {
                if let Ok(ts) = ts_str.parse::<i64>() {
                    let now = chrono::Utc::now().timestamp();
                    // Consider lock stale after 60 seconds
                    if now - ts > 60 {
                        let _ = std::fs::remove_file(&lock_path);
                    } else {
                        bail!("state file is locked by another process (lock: {lock_path})");
                    }
                }
            }
        }

        let pid = std::process::id();
        let ts = chrono::Utc::now().timestamp();
        let lock_content = format!("{pid}\n{ts}\n");
        std::fs::write(&lock_path, lock_content)
            .with_context(|| format!("cannot acquire lock {lock_path}"))?;

        Ok(Self { lock_path })
    }

    /// Release the advisory lock.
    pub fn release(self) -> Result<()> {
        if std::path::Path::new(&self.lock_path).exists() {
            std::fs::remove_file(&self.lock_path)
                .with_context(|| format!("cannot release lock {}", self.lock_path))?;
        }
        Ok(())
    }
}

impl Drop for AdvisoryLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

/// Simple pseudo-UUID for temp file names.
fn uuid_v4_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn atomic_write_creates_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        atomic_write(path, b"hello world").unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn advisory_lock_acquire_and_release() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        let lock = AdvisoryLock::acquire(path).unwrap();
        // Lock file should exist
        assert!(std::path::Path::new(&format!("{path}.lock")).exists());

        lock.release().unwrap();
        // Lock file should be gone
        assert!(!std::path::Path::new(&format!("{path}.lock")).exists());
    }

    #[test]
    fn advisory_lock_blocks_concurrent() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        let _lock1 = AdvisoryLock::acquire(path).unwrap();
        let result = AdvisoryLock::acquire(path);
        assert!(result.is_err());
    }

    #[test]
    fn advisory_lock_drop_releases() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        {
            let _lock = AdvisoryLock::acquire(&path).unwrap();
            // Lock should exist
            assert!(std::path::Path::new(&format!("{path}.lock")).exists());
        }
        // After drop, lock should be gone
        assert!(!std::path::Path::new(&format!("{path}.lock")).exists());

        // Should be able to re-acquire
        let _lock2 = AdvisoryLock::acquire(&path).unwrap();
    }

    #[test]
    fn atomic_write_is_idempotent() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();

        atomic_write(path, b"version 1").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "version 1");

        atomic_write(path, b"version 2").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "version 2");

        // Original file still intact (overwritten atomically)
        atomic_write(path, b"version 3").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "version 3");
    }

    #[test]
    fn concurrent_writers_with_lock() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("test.state.json");
        let state_str = state_path.to_str().unwrap().to_string();

        // Write initial content
        std::fs::write(&state_path, "0").unwrap();

        let barrier = Arc::new(Barrier::new(4));
        let mut handles = Vec::new();
        let success_count = Arc::new(std::sync::atomic::AtomicU32::new(0));

        for i in 0..4 {
            let b = Arc::clone(&barrier);
            let path = state_str.clone();
            let count = Arc::clone(&success_count);

            handles.push(thread::spawn(move || {
                b.wait(); // Sync all threads to start simultaneously
                match AdvisoryLock::acquire(&path) {
                    Ok(lock) => {
                        // Simulate write
                        let _ = atomic_write(&path, format!("writer-{i}").as_bytes());
                        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let _ = lock.release();
                    }
                    Err(_) => {
                        // Expected: lock contention
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // At least one writer should have succeeded
        let successes = success_count.load(std::sync::atomic::Ordering::SeqCst);
        assert!(successes >= 1, "at least one writer must succeed");

        // Final content should be valid (from one of the writers)
        let content = std::fs::read_to_string(&state_path).unwrap();
        assert!(content.starts_with("writer-"));
    }
}
