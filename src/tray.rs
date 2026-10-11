use std::thread;
use std::time::Duration;
use tracing::info;

use crate::http::shutdown::ShutdownTrigger;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    Icon, TrayIconBuilder, TrayIconEvent,
};

/// The Seula "S" in the UI's accent colour (assets/tray-icon.png, 64x64, from the app icon's
/// alpha). Coloured rather than black so it shows on a dark taskbar as well as a light one.
const TRAY_ICON_PNG: &[u8] = include_bytes!("../assets/tray-icon.png");

fn create_tray_icon() -> Result<Icon, Box<dyn std::error::Error + Send + Sync>> {
    let mut decoder = png::Decoder::new(TRAY_ICON_PNG);
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info()?;
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba)?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err("assets/tray-icon.png must be 8-bit RGBA".into());
    }
    rgba.truncate(info.buffer_size());
    Ok(Icon::from_rgba(rgba, info.width, info.height)?)
}

pub struct TrayApp {
    _tray_icon: tray_icon::TrayIcon,
    quit_id: MenuId,
    // Shared with `POST /system/shutdown` (ADR-0076): Quit fires it, and the loop below ends
    // when either Quit or the route has. Ending the loop drops the icon, removing it.
    shutdown: ShutdownTrigger,
}

impl TrayApp {
    pub fn new(
        shutdown: ShutdownTrigger,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let menu = Menu::new();

        let quit = MenuItem::new("Quit", true, None);

        // Store the menu item ID
        let quit_id = quit.id().clone();

        menu.append(&quit)?;

        // Create icon - multiple approaches available
        let icon = create_tray_icon()?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Seula")
            .with_icon(icon)
            .build()?;

        Ok(TrayApp {
            _tray_icon: tray_icon,
            quit_id,
            shutdown,
        })
    }

    pub fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("System tray initialized, server running in background");

        let quit_id = self.quit_id.clone();
        // Set up event handlers
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| match event {
            TrayIconEvent::Click { button, .. } => {
                info!("Tray icon clicked with button: {:?}", button);
            }
            TrayIconEvent::DoubleClick { button, .. } => {
                info!("Tray icon double-clicked with button: {:?}", button);
            }
            _ => {}
        }));

        let shutdown = self.shutdown.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id() == &quit_id {
                info!("Quit requested from tray menu");
                shutdown.trigger();
            }
        }));

        // Main event loop
        loop {
            // Check for shutdown signal
            if self.shutdown.is_triggered() {
                break;
            }

            // On Windows, we need to pump messages for the tray icon to work properly
            #[cfg(target_os = "windows")]
            {
                use std::ptr;
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
                };

                unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    // Non-blocking message pump
                    if PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }

            // Small delay to prevent busy waiting
            thread::sleep(Duration::from_millis(10));
        }

        info!("Shutting down Seula");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded PNG must decode to a valid tray icon; otherwise the tray fails at start-up.
    #[test]
    fn embedded_tray_icon_decodes() {
        create_tray_icon().expect("assets/tray-icon.png should decode to an RGBA icon");
    }
}
