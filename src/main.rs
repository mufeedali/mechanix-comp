#[cfg(any(feature = "backend-winit", feature = "backend-udev"))]
use compositor::backend;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Backend selection: an explicit `MECHA_BACKEND` wins; otherwise we assume
    // we're nested (winit) when a parent display server is present, and drive
    // KMS/DRM directly (udev) when running from a bare VT.
    let use_winit = match std::env::var("MECHA_BACKEND").ok().as_deref() {
        Some("winit") => true,
        Some("udev") => false,
        Some(other) => {
            eprintln!("unknown MECHA_BACKEND={other:?}, falling back to auto-detection");
            nested_session_present()
        }
        None => nested_session_present(),
    };

    if use_winit {
        #[cfg(feature = "backend-winit")]
        {
            backend::winit::run()
        }
        #[cfg(not(feature = "backend-winit"))]
        {
            Err("winit backend not compiled (enable feature backend-winit)".into())
        }
    } else {
        #[cfg(feature = "backend-udev")]
        {
            backend::udev::run()
        }
        #[cfg(not(feature = "backend-udev"))]
        {
            Err("udev backend not compiled (enable feature backend-udev)".into())
        }
    }
}

/// True when we appear to be running inside another Wayland or X11 session.
fn nested_session_present() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}
