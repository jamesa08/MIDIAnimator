// screenshots of the app's windows, used by the MCP server so ML clients can see the UI while debugging.
// macOS and windows use xcap, which captures the window as it's composited on screen (native title bar included), even while it's covered.
// linux uses WebKitGTK's own snapshot of the page instead, xcap needs pipewire there and that isn't on every distro

use tauri::{Manager, WebviewWindow};

use crate::state::WINDOW;

/// a captured window as PNG bytes
pub struct Screenshot {
    pub label: String,
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// every window label with whether it's focused, for picking one to capture
pub fn window_labels() -> Vec<(String, bool)> {
    let Some(main) = WINDOW.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
        return Vec::new();
    };
    let mut labels: Vec<(String, bool)> = main.app_handle().webview_windows().into_iter().map(|(label, window)| (label, window.is_focused().unwrap_or(false))).collect();
    labels.sort();
    labels
}

/// the window with `label`, otherwise the focused window, otherwise main (nothing is focused while the app is in the background)
fn pick_window(label: Option<&str>) -> Result<WebviewWindow, String> {
    let main = WINDOW.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or("the main window hasn't been created yet")?;
    let windows = main.app_handle().webview_windows();
    match label {
        Some(label) => windows.get(label).cloned().ok_or_else(|| format!("no window '{}'", label)),
        None => Ok(windows.into_values().find(|window| window.is_focused().unwrap_or(false)).unwrap_or(main)),
    }
}

/// the OS window id xcap knows the window by
#[cfg(target_os = "macos")]
fn os_window_id(window: &WebviewWindow) -> Result<u32, String> {
    let ns_window = window.ns_window().map_err(|e| e.to_string())?;
    let number = unsafe { (*ns_window.cast::<objc2_app_kit::NSWindow>()).windowNumber() };
    Ok(number as u32)
}

#[cfg(target_os = "windows")]
fn os_window_id(window: &WebviewWindow) -> Result<u32, String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    Ok(hwnd.0 as u32)
}

/// captures a window as a PNG
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub async fn capture_window(label: Option<&str>) -> Result<Screenshot, String> {
    use std::io::Cursor;

    let window = pick_window(label)?;
    let id = os_window_id(&window)?;
    let label = window.label().to_string();

    // capturing and encoding blocks, keep it off the async runtime
    tokio::task::spawn_blocking(move || {
        let target = xcap::Window::all().map_err(|e| e.to_string())?.into_iter().find(|w| w.id().ok() == Some(id)).ok_or(format!("window '{}' isn't on screen", label))?;
        let image = target.capture_image().map_err(|e| e.to_string())?;
        let mut png = Vec::new();
        image.write_to(&mut Cursor::new(&mut png), xcap::image::ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(Screenshot {
            label,
            png,
            width: image.width(),
            height: image.height(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// captures a window's page as a PNG (no title bar, the page only)
#[cfg(target_os = "linux")]
pub async fn capture_window(label: Option<&str>) -> Result<Screenshot, String> {
    use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};

    let window = pick_window(label)?;
    let label = window.label().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(Vec<u8>, u32, u32), String>>();

    // the snapshot runs on the gtk main thread, cairo surfaces can't leave it so the PNG is encoded there too
    window
        .with_webview(move |webview| {
            webview.inner().snapshot(SnapshotRegion::Visible, SnapshotOptions::NONE, None::<&webkit2gtk::gio::Cancellable>, move |result| {
                let encoded = result.map_err(|e| e.to_string()).and_then(|surface| {
                    let image = cairo::ImageSurface::try_from(surface).map_err(|_| "the snapshot isn't an image surface".to_string())?;
                    let mut png = Vec::new();
                    image.write_to_png(&mut png).map_err(|e| e.to_string())?;
                    Ok((png, image.width() as u32, image.height() as u32))
                });
                let _ = tx.send(encoded);
            });
        })
        .map_err(|e| e.to_string())?;

    let (png, width, height) = rx.await.map_err(|_| format!("window '{}' closed before the snapshot finished", label))??;
    Ok(Screenshot { label, png, width, height })
}
