use std::path::Path;

pub trait IconExtractor {
    fn extract_and_save(
        &self,
        _path: &Path,
        _icons_dir: &Path,
        _name_hint: &str,
    ) -> Option<std::path::PathBuf> {
        None
    }
}

pub struct DummyExtractor;
impl IconExtractor for DummyExtractor {}

#[cfg(windows)]
pub struct WindowsIconExtractor;

#[cfg(windows)]
impl IconExtractor for WindowsIconExtractor {
    fn extract_and_save(
        &self,
        _path: &Path,
        _icons_dir: &Path,
        _name_hint: &str,
    ) -> Option<std::path::PathBuf> {
        // Full HICON→RGBA extraction deferred — use default icons for now.
        None
    }
}
