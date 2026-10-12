use crate::{Error, Result};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{fs, path::Path};

#[derive(Clone, Copy, Debug)]
pub struct ArtifactLimits {
    pub files: usize,
    pub bytes: usize,
}
impl ArtifactLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.files == 0 || self.files > 128 || self.bytes == 0 || self.bytes > 16 * 1024 * 1024 {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct Artifact {
    pub path: String,
    pub text: String,
    /// True for a deletion; an empty replacement has false and empty text.
    pub deleted: bool,
}

pub(crate) fn relative(path: &str) -> bool {
    !path.is_empty()
        && !path.ends_with('/')
        && path.split('/').all(|s| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s != ".git"
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        })
}
pub(crate) fn permits(scope: &[String], path: &str) -> bool {
    relative(path)
        && scope
            .iter()
            .any(|s| s == path || s.ends_with('/') && path.starts_with(s))
}

pub(crate) fn regular(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.is_symlink() {
        return Err(Error::Scope);
    }
    #[cfg(unix)]
    if m.nlink() != 1 {
        return Err(Error::Scope);
    }
    Ok(())
}

/// No task process may be alive when this trusted-host traversal runs.
pub(crate) fn tree(root: &Path, max_entries: usize) -> Result<Vec<String>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    let mut entries = 0;
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            entries += 1;
            if entries > max_entries {
                return Err(Error::Scope);
            }
            let entry = entry?;
            let p = entry.path();
            let rel = p.strip_prefix(root).map_err(|_| Error::Scope)?;
            if rel == Path::new(".git") {
                continue;
            }
            let s = rel.to_str().ok_or(Error::Scope)?;
            if !relative(s) {
                return Err(Error::Scope);
            }
            let m = fs::symlink_metadata(&p)?;
            if m.is_symlink() {
                return Err(Error::Scope);
            }
            if m.is_dir() {
                pending.push(p);
            } else {
                regular(&p)?;
                files.push(s.to_owned());
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(crate) fn collect(
    root: &Path,
    scope: &[String],
    changed: &[String],
    limits: ArtifactLimits,
) -> Result<Vec<Artifact>> {
    limits.validate()?;
    if changed.len() > limits.files {
        return Err(Error::Scope);
    }
    tree(root, 100_000)?;
    let mut bytes = 0usize;
    let mut result = Vec::new();
    for path in changed {
        if !permits(scope, path) {
            return Err(Error::Scope);
        }
        let p = root.join(path);
        if !p.exists() {
            result.push(Artifact {
                path: path.clone(),
                text: String::new(),
                deleted: true,
            });
            continue;
        }
        regular(&p)?;
        let size = usize::try_from(fs::metadata(&p)?.len()).map_err(|_| Error::Scope)?;
        bytes = bytes.checked_add(size).ok_or(Error::Scope)?;
        if bytes > limits.bytes {
            return Err(Error::Scope);
        }
        let text = fs::read_to_string(&p)?;
        if text.len() != size {
            return Err(Error::Scope);
        }
        result.push(Artifact {
            path: path.clone(),
            text,
            deleted: false,
        });
    }
    Ok(result)
}
