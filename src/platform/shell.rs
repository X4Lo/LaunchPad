//! Shell integration: launching items and loading their icons.
//!
//! Icons are produced on a background thread so the window opens instantly no matter
//! how many items there are:
//! - App / folder icons come from the Windows shell (`IShellItemImageFactory`). It is
//!   what Explorer uses: it resolves `.lnk` shortcuts, picks the best size the file
//!   offers (up to 256 px) and returns proper alpha, including for old 24-bit icons.
//! - Custom icons (PNG / ICO / JPG / BMP) are decoded with the `image` crate.

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

/// Pixel size icons are produced at: crisp up to 128 px tiles at 100 %, 85 px at 150 %.
pub const ICON_PX: u32 = 128;

pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IconSource {
    /// The icon Explorer shows for this file or folder.
    Shell(PathBuf),
    /// An image file chosen by the user.
    File(PathBuf),
}

/// Start the icon threads (a few, since shell extraction is mostly waiting on disk).
/// `on_ready` is called on a worker thread for every request, with `None` when no icon
/// could be produced.
pub fn spawn_icon_worker(
    on_ready: impl Fn(IconSource, Option<Rgba>) + Send + Sync + 'static,
) -> mpsc::Sender<IconSource> {
    let (tx, rx) = mpsc::channel::<IconSource>();
    let rx = Arc::new(Mutex::new(rx));
    let on_ready = Arc::new(on_ready);
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 4));
    for n in 0..workers {
        let (rx, on_ready) = (rx.clone(), on_ready.clone());
        let spawned = std::thread::Builder::new().name(format!("launchpad-icons-{n}")).spawn(move || {
            let _com = com::init();
            loop {
                // Hold the lock only while taking the next request.
                let next = rx.lock().ok().and_then(|r| r.recv().ok());
                let Some(source) = next else { break };
                let icon = match &source {
                    IconSource::Shell(p) => shell_icon(p, ICON_PX),
                    // Custom icons may also be executables / DLLs / shortcuts: ask the shell.
                    IconSource::File(p) if is_image(p) => image_file(p, ICON_PX),
                    IconSource::File(p) => shell_icon(p, ICON_PX),
                };
                if icon.is_none() {
                    log::debug!("no icon for {source:?}");
                }
                on_ready(source, icon);
            }
        });
        if let Err(e) = spawned {
            log::error!("cannot start icon thread: {e}");
        }
    }
    tx
}

fn is_image(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "png" | "ico" | "jpg" | "jpeg" | "bmp")
}

fn image_file(path: &Path, size: u32) -> Option<Rgba> {
    let img = image::open(path).map_err(|e| log::warn!("icon {}: {e}", path.display())).ok()?;
    // Keep the aspect ratio; only shrink (small images are scaled by the UI).
    let img = if img.width() > size || img.height() > size {
        img.resize(size, size, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    Some(Rgba { width: rgba.width(), height: rgba.height(), pixels: rgba.into_raw() })
}

// ─── Windows ─────────────────────────────────────────────

#[cfg(windows)]
mod com {
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };

    /// COM for the current thread (the shell APIs need an STA). Uninitialises on drop.
    pub struct Com(bool);

    pub fn init() -> Com {
        let ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }.is_ok();
        Com(ok)
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }
}

#[cfg(windows)]
fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// The icon Explorer shows for `path`, as straight-alpha RGBA.
///
/// 1. `IShellItemImageFactory`: best quality, handles shortcuts / folders / any file.
///    It may answer `E_PENDING` while the shell's icon cache warms up, so retry a bit.
/// 2. If that fails, or hands back a bitmap without alpha (legacy 24-bit icons, which
///    would otherwise turn into black squares), take the icon handle itself and use its
///    transparency mask.
#[cfg(windows)]
fn shell_icon(path: &Path, size: u32) -> Option<Rgba> {
    let from_factory = (0..4u64).find_map(|attempt| match factory_image(path, size) {
        Ok(img) => Some(Some(img)),
        Err(e) if e.code() == windows::core::HRESULT(0x8000_000A_u32 as i32) && attempt < 3 => {
            std::thread::sleep(std::time::Duration::from_millis(40 * (attempt + 1)));
            None
        }
        Err(e) => {
            log::debug!("shell icon {}: {e}", path.display());
            Some(None)
        }
    });
    let icon = match from_factory.flatten() {
        Some((img, true)) => Some(img),
        other => hicon_image(path, size).or(other.map(|(img, _)| img)),
    };
    icon.map(trim_small)
}

/// Returns the image and whether it carried real alpha.
#[cfg(windows)]
fn factory_image(path: &Path, size: u32) -> windows::core::Result<(Rgba, bool)> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{DeleteObject, HGDIOBJ};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY};

    let path_w = wide(path.as_os_str());
    unsafe {
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(PCWSTR(path_w.as_ptr()), None)?;
        let side = size as i32;
        let hbmp = factory.GetImage(SIZE { cx: side, cy: side }, SIIGBF_ICONONLY)?;
        let read = read_bitmap(hbmp);
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        let (w, h, mut px) = read.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
        let has_alpha = px.chunks_exact(4).any(|p| p[3] != 0);
        bgra_to_straight_rgba(&mut px);
        Ok((Rgba { width: w, height: h, pixels: px }, has_alpha))
    }
}

/// The file's icon handle (closest size the file offers to `size`), alpha taken from
/// the icon's own alpha channel or, for legacy icons, its AND mask.
#[cfg(windows)]
fn hicon_image(path: &Path, size: u32) -> Option<Rgba> {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{DeleteObject, HGDIOBJ};
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::UI::Shell::{SHDefExtractIconW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

    let path_w = wide(path.as_os_str());
    unsafe {
        // Icon resources (exe / dll / ico): pick the closest size the file has.
        let mut hicon = HICON::default();
        let hr = SHDefExtractIconW(PCWSTR(path_w.as_ptr()), 0, 0, Some(&mut hicon), None, size & 0xffff);
        if hr.0 != 0 || hicon.is_invalid() {
            // Anything else (shortcuts, documents, folders): the shell's 32 px icon.
            let mut info = SHFILEINFOW::default();
            let ok = SHGetFileInfoW(
                PCWSTR(path_w.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            if ok == 0 || info.hIcon.is_invalid() {
                return None;
            }
            hicon = info.hIcon;
        }

        let mut ii = ICONINFO::default();
        let got_info = GetIconInfo(hicon, &mut ii).is_ok();
        let _ = DestroyIcon(hicon);
        if !got_info {
            return None;
        }
        let color = if ii.hbmColor.is_invalid() { None } else { read_bitmap(ii.hbmColor) };
        let mask = if ii.hbmMask.is_invalid() { None } else { read_bitmap(ii.hbmMask) };
        let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));

        let (w, h, mut px) = color?;
        if !px.chunks_exact(4).any(|p| p[3] != 0) {
            // No alpha channel: opaque wherever the AND mask is black.
            let (_, mh, m) = mask?;
            if mh < h {
                return None;
            }
            for (p, mp) in px.chunks_exact_mut(4).zip(m.chunks_exact(4)) {
                p[3] = if mp[0] == 0 { 255 } else { 0 };
            }
        }
        bgra_to_straight_rgba(&mut px);
        Some(Rgba { width: w, height: h, pixels: px })
    }
}

/// Read any GDI bitmap as 32-bit top-down BGRA.
#[cfg(windows)]
unsafe fn read_bitmap(hbmp: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::Graphics::Gdi::{
        GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    unsafe {
        let mut bm = BITMAP::default();
        let got = GetObjectW(
            HGDIOBJ(hbmp.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut BITMAP as *mut std::ffi::c_void),
        );
        let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
        if got == 0 || w <= 0 || h <= 0 {
            return None;
        }
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut px = vec![0u8; (w * h * 4) as usize];
        let hdc = GetDC(None);
        let lines = GetDIBits(
            hdc,
            hbmp,
            0,
            h as u32,
            Some(px.as_mut_ptr() as *mut std::ffi::c_void),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        (lines != 0).then_some((w as u32, h as u32, px))
    }
}

/// The shell pads icons that only exist at small sizes into the middle of a large
/// transparent canvas. Crop such padding (keeping it square) so the icon fills its tile.
fn trim_small(img: Rgba) -> Rgba {
    let (w, h) = (img.width as usize, img.height as usize);
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if img.pixels[(y * w + x) * 4 + 3] > 8 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return img;
    }
    let content = (x1 - x0).max(y1 - y0);
    // Normal icons already fill most of the canvas; leave those alone.
    if content * 10 > w.min(h) * 6 {
        return img;
    }
    let side = (content + content / 8).min(w).min(h); // keep a little breathing room
    let cx = (x0 + x1) / 2;
    let cy = (y0 + y1) / 2;
    let left = cx.saturating_sub(side / 2).min(w - side);
    let top = cy.saturating_sub(side / 2).min(h - side);
    let mut pixels = Vec::with_capacity(side * side * 4);
    for y in top..top + side {
        pixels.extend_from_slice(&img.pixels[(y * w + left) * 4..(y * w + left + side) * 4]);
    }
    Rgba { width: side as u32, height: side as u32, pixels }
}

/// GDI bitmaps are BGRA, usually premultiplied. Convert to straight RGBA, and treat a
/// bitmap without any alpha as fully opaque.
fn bgra_to_straight_rgba(px: &mut [u8]) {
    let has_alpha = px.chunks_exact(4).any(|p| p[3] != 0);
    let premultiplied = has_alpha && px.chunks_exact(4).all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]);
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        } else if premultiplied && p[3] != 0 && p[3] != 255 {
            let a = p[3] as u32;
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

/// Open a file or folder the way Explorer would (ShellExecute "open"), on a helper
/// thread so a slow target never stalls the UI. `on_error` runs on that thread.
#[cfg(windows)]
pub fn open(path: PathBuf, on_error: impl FnOnce(String) + Send + 'static) {
    let _ = std::thread::Builder::new().name("launchpad-open".into()).spawn(move || {
        use windows::core::{w, PCWSTR};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let _com = com::init();
        let file = wide(path.as_os_str());
        // Run apps from their own folder, like a shortcut would.
        let dir = path.parent().filter(|_| path.is_file()).map(|p| wide(p.as_os_str()));
        let dir_ptr = dir.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr()));
        let result =
            unsafe { ShellExecuteW(None, w!("open"), PCWSTR(file.as_ptr()), PCWSTR::null(), dir_ptr, SW_SHOWNORMAL) };
        // Values <= 32 are error codes.
        if result.0 as usize <= 32 {
            let why = if path.exists() { "Windows couldn't open it" } else { "it no longer exists" };
            log::warn!("open {} failed ({:?})", path.display(), result.0);
            on_error(format!("Couldn't open {}: {why}", display_name(&path)));
        }
    });
}

/// Show the item selected in an Explorer window.
#[cfg(windows)]
pub fn reveal(path: &Path) {
    let mut arg = std::ffi::OsString::from("/select,");
    arg.push(path.as_os_str());
    if let Err(e) = std::process::Command::new("explorer").arg(arg).spawn() {
        log::warn!("reveal {}: {e}", path.display());
    }
}

// ─── Other platforms (development builds only) ───────────

#[cfg(not(windows))]
mod com {
    pub struct Com;
    pub fn init() -> Com {
        Com
    }
}

#[cfg(not(windows))]
fn shell_icon(_path: &Path, _size: u32) -> Option<Rgba> {
    None
}

#[cfg(not(windows))]
pub fn open(path: PathBuf, on_error: impl FnOnce(String) + Send + 'static) {
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    if std::process::Command::new(opener).arg(&path).spawn().is_err() {
        on_error(format!("Couldn't open {}", display_name(&path)));
    }
}

#[cfg(not(windows))]
pub fn reveal(path: &Path) {
    if let Some(dir) = path.parent() {
        open(dir.to_path_buf(), |_| {});
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// Open an http(s) URL in the default browser.
pub fn open_url(url: &str) {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return;
    }
    #[cfg(windows)]
    let result = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(not(windows))]
    let result =
        std::process::Command::new(if cfg!(target_os = "macos") { "open" } else { "xdg-open" }).arg(url).spawn();
    if let Err(e) = result {
        log::warn!("open {url}: {e}");
    }
}

/// Open a folder in the file manager (for "Open settings folder").
pub fn open_folder(path: &Path) {
    let _ = std::fs::create_dir_all(path);
    open(path.to_path_buf(), |e| log::warn!("{e}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas_with_square(from: u32, to: u32) -> Rgba {
        let (w, h) = (128u32, 128u32);
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        for y in from..to {
            for x in from..to {
                pixels[((y * w + x) * 4 + 3) as usize] = 255;
            }
        }
        Rgba { width: w, height: h, pixels }
    }

    #[test]
    fn small_icon_on_big_canvas_is_trimmed() {
        let t = trim_small(canvas_with_square(48, 80));
        assert_eq!((t.width, t.height), (36, 36));
    }

    #[test]
    fn full_size_icon_is_not_trimmed() {
        let t = trim_small(canvas_with_square(10, 118));
        assert_eq!((t.width, t.height), (128, 128));
    }

    #[test]
    fn opaque_bitmap_without_alpha_becomes_visible() {
        // The old extractor's bug: 24-bit icons came back with alpha 0 and vanished.
        let mut px = vec![10, 20, 30, 0, 40, 50, 60, 0];
        bgra_to_straight_rgba(&mut px);
        assert_eq!(px, vec![30, 20, 10, 255, 60, 50, 40, 255]);
    }

    #[test]
    fn premultiplied_alpha_is_undone() {
        let mut px = vec![0, 0, 64, 128, 255, 255, 255, 255];
        bgra_to_straight_rgba(&mut px);
        assert_eq!(&px[..4], &[128, 0, 0, 128]);
        assert_eq!(&px[4..], &[255, 255, 255, 255]);
    }

    #[test]
    fn straight_alpha_is_left_alone() {
        // A colour channel above alpha means the data can't be premultiplied.
        let mut px = vec![200, 100, 50, 100];
        bgra_to_straight_rgba(&mut px);
        assert_eq!(px, vec![50, 100, 200, 100]);
    }
}
