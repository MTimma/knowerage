use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::types::KnowerageError;

pub fn validate_path(workspace_root: &Path, input_path: &str) -> Result<PathBuf, KnowerageError> {
    if input_path.is_empty() {
        return Err(KnowerageError::PathTraversal(
            "E_PATH_TRAVERSAL: empty path".into(),
        ));
    }

    for segment in input_path.split(['/', '\\']) {
        if segment == ".." {
            return Err(KnowerageError::PathTraversal(
                "E_PATH_TRAVERSAL: path contains '..' segment".into(),
            ));
        }
    }

    let candidate = if Path::new(input_path).is_absolute() {
        PathBuf::from(input_path)
    } else {
        workspace_root.join(input_path)
    };

    let canonical_root = std::fs::canonicalize(workspace_root).map_err(|e| {
        KnowerageError::PathTraversal(format!(
            "E_PATH_TRAVERSAL: cannot canonicalize workspace root: {e}"
        ))
    })?;

    let canonical_candidate = if candidate.exists() {
        std::fs::canonicalize(&candidate).map_err(|e| {
            KnowerageError::PathTraversal(format!("E_PATH_TRAVERSAL: cannot resolve path: {e}"))
        })?
    } else {
        let parent = candidate.parent().ok_or_else(|| {
            KnowerageError::PathTraversal("E_PATH_TRAVERSAL: path has no parent".into())
        })?;
        let file_name = candidate.file_name().ok_or_else(|| {
            KnowerageError::PathTraversal("E_PATH_TRAVERSAL: path has no filename".into())
        })?;
        let canonical_parent = std::fs::canonicalize(parent).map_err(|e| {
            KnowerageError::PathTraversal(format!(
                "E_PATH_TRAVERSAL: cannot resolve parent dir: {e}"
            ))
        })?;
        canonical_parent.join(file_name)
    };

    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(KnowerageError::PathTraversal(
            "E_PATH_TRAVERSAL: resolved path is outside workspace root".into(),
        ));
    }

    Ok(canonical_candidate)
}

pub fn atomic_write(target: &Path, content: &[u8]) -> Result<(), KnowerageError> {
    let tmp_path = target.with_extension("tmp");

    let write_result = std::fs::write(&tmp_path, content).map_err(|e| {
        KnowerageError::RegistryIo(format!(
            "E_REGISTRY_IO: failed to write temp file {}: {e}",
            tmp_path.display()
        ))
    });

    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    if let Err(e) = std::fs::rename(&tmp_path, target) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(KnowerageError::RegistryIo(format!(
            "E_REGISTRY_IO: failed to rename temp file to {}: {e}",
            target.display()
        )));
    }

    Ok(())
}

pub fn sanitize_string(input: &str, max_len: usize) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_ascii_control() || *c == '\n' || *c == '\t')
        .collect();

    let truncated = if cleaned.len() > max_len {
        match cleaned.char_indices().nth(max_len) {
            Some((byte_idx, _)) => &cleaned[..byte_idx],
            None => &cleaned,
        }
    } else {
        &cleaned
    };

    truncated.trim().to_string()
}

pub fn looks_like_secret(value: &str) -> bool {
    let lower = value.to_lowercase();

    const CREDENTIAL_PATTERNS: &[&str] = &[
        "password=",
        "passwd=",
        "token=",
        "secret=",
        "api_key=",
        "apikey=",
        "access_key=",
        "private_key=",
        "auth_token=",
    ];

    if CREDENTIAL_PATTERNS.iter().any(|p| lower.contains(p)) {
        return true;
    }

    if value.starts_with("AKIA") && value.len() >= 20 {
        return true;
    }

    if value.starts_with("eyJ") && value.contains('.') {
        return true;
    }

    let base64_chars: usize = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '=')
        .count();
    if base64_chars > 40 && base64_chars as f64 / value.len().max(1) as f64 > 0.85 {
        return true;
    }

    false
}

pub struct RegistryLock {
    inner: Mutex<()>,
}

impl RegistryLock {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(()),
        }
    }

    pub fn acquire(&self) -> Result<RegistryGuard<'_>, KnowerageError> {
        let guard = self.inner.lock().map_err(|e| {
            KnowerageError::RegistryIo(format!("E_REGISTRY_IO: lock poisoned: {e}"))
        })?;
        Ok(RegistryGuard { _guard: guard })
    }
}

impl Default for RegistryLock {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RegistryGuard<'a> {
    _guard: MutexGuard<'a, ()>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    fn test_path_validator_rejects_dotdot() {
        let tmp = TempDir::new().unwrap();
        let err = validate_path(tmp.path(), "../../etc/passwd").unwrap_err();
        assert_eq!(err.code(), "E_PATH_TRAVERSAL");
    }

    #[test]
    fn test_path_validator_rejects_out_of_root() {
        let tmp = TempDir::new().unwrap();
        let err = validate_path(tmp.path(), "/etc/passwd").unwrap_err();
        assert_eq!(err.code(), "E_PATH_TRAVERSAL");
    }

    #[test]
    fn test_path_validator_rejects_dotdot_in_middle() {
        let tmp = TempDir::new().unwrap();
        let err = validate_path(tmp.path(), "foo/../../../etc/passwd").unwrap_err();
        assert_eq!(err.code(), "E_PATH_TRAVERSAL");
    }

    #[test]
    fn test_path_validator_accepts_valid_path() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("subdir")).unwrap();
        let result = validate_path(tmp.path(), "subdir/file.txt").unwrap();
        assert!(result.starts_with(std::fs::canonicalize(tmp.path()).unwrap()));
        assert!(result.ends_with("file.txt"));
    }

    #[test]
    fn test_atomic_write_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("output.txt");
        let content = b"hello, world";

        atomic_write(&target, content).unwrap();

        let read_back = std::fs::read(&target).unwrap();
        assert_eq!(read_back, content);

        assert!(!tmp.path().join("output.tmp").exists());
    }

    #[test]
    fn test_concurrent_registry_lock() {
        let lock = Arc::new(RegistryLock::new());
        let counter = Arc::new(Mutex::new(0u32));
        let mut handles = vec![];

        for _ in 0..2 {
            let lock_clone = Arc::clone(&lock);
            let counter_clone = Arc::clone(&counter);
            handles.push(std::thread::spawn(move || {
                let _guard = lock_clone.acquire().unwrap();
                let mut val = counter_clone.lock().unwrap();
                let old = *val;
                std::thread::sleep(std::time::Duration::from_millis(10));
                *val = old + 1;
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(*counter.lock().unwrap(), 2);
    }

    #[test]
    fn test_crash_during_write_original_intact() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("registry.txt");
        std::fs::write(&target, b"original").unwrap();

        let nonexistent_dir = tmp.path().join("no_such_dir").join("file.txt");
        let result = atomic_write(&nonexistent_dir, b"new content");
        assert!(result.is_err());

        let original = std::fs::read_to_string(&target).unwrap();
        assert_eq!(original, "original");
    }

    #[test]
    fn test_looks_like_secret_detects_credentials() {
        assert!(looks_like_secret("password=hunter2"));
        assert!(looks_like_secret("token=abc123xyz"));
        assert!(looks_like_secret("api_key=sk_live_abc123"));
        assert!(looks_like_secret("secret=mysecretvalue"));

        assert!(looks_like_secret("AKIAIOSFODNN7EXAMPLE"));

        assert!(looks_like_secret(
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIn0.sig"
        ));

        let long_b64 = "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXoxMjM0NTY3ODk=";
        assert!(looks_like_secret(long_b64));
    }

    #[test]
    fn test_looks_like_secret_passes_normal_strings() {
        assert!(!looks_like_secret("hello world"));
        assert!(!looks_like_secret("src/main/java/App.java"));
        assert!(!looks_like_secret("covered_lines: [[1, 50]]"));
        assert!(!looks_like_secret(""));
    }

    #[test]
    fn test_sanitize_removes_control_chars() {
        let input = "hello\x00world\x07test\nkeep\ttabs";
        let result = sanitize_string(input, 1000);
        assert_eq!(result, "helloworldtest\nkeep\ttabs");
    }

    #[test]
    fn test_sanitize_truncates_to_max_len() {
        let input = "abcdefghij";
        let result = sanitize_string(input, 5);
        assert_eq!(result, "abcde");
    }

    #[test]
    fn test_sanitize_trims_whitespace() {
        let input = "  hello world  ";
        let result = sanitize_string(input, 100);
        assert_eq!(result, "hello world");
    }
}
