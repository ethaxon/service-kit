use service_kit_core::{Error, ErrorCode, Result};
use std::{fs, io::Write, path::Path};
pub(crate) fn reject_links(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        for p in path.ancestors() {
            if let Ok(metadata) = fs::symlink_metadata(p)
                && metadata.file_attributes() & 0x400 != 0
            {
                return Err(Error::new(
                    ErrorCode::UnsafePath,
                    "Windows reparse points refused",
                ));
            }
        }
    }
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::new(
                    ErrorCode::UnsafePath,
                    format!("symlink path refused: {}", p.display()),
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub fn verify_file(path: &Path, content: &str) -> Result<()> {
    reject_links(path)?;
    verify_content(path, content)
}
fn verify_content(path: &Path, content: &str) -> Result<()> {
    if fs::read(path)? != content.as_bytes() {
        return Err(Error::new(
            ErrorCode::Conflict,
            format!(
                "definition differs or is unrelated; refusing {}",
                path.display()
            ),
        ));
    }
    Ok(())
}
pub fn ensure_file(path: &Path, content: &str, make_executable: bool) -> Result<bool> {
    reject_links(path)?;
    if path.try_exists()? {
        verify_content(path, content)?;
        return Ok(false);
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(ErrorCode::InvalidSpec, "missing parent"))?;
    fs::create_dir_all(parent)?;
    reject_links(path)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(content.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp.as_file()
            .set_permissions(fs::Permissions::from_mode(if make_executable {
                0o755
            } else {
                0o600
            }))?;
    }
    #[cfg(not(unix))]
    let _ = make_executable;
    tmp.as_file().sync_all()?;
    tmp.persist_noclobber(path)
        .map_err(|e| Error::new(ErrorCode::Conflict, e.to_string()))?;
    Ok(true)
}
pub fn remove_file(path: &Path, expected: &str) -> Result<()> {
    reject_links(path)?;
    if path.try_exists()? {
        verify_content(path, expected)?;
        fs::remove_file(path)?;
    }
    Ok(())
}
