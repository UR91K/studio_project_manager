use log::info;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    Icon, TrayIconBuilder, TrayIconEvent,
};

fn generate_embedded_icon_data() -> Vec<u8> {
    let rgba_data = vec![
        58, 57, 58, 255, 57, 57, 57, 255, 58, 57, 58, 255, 58, 57, 58, 255, 58, 57, 58, 255, 57,
        57, 57, 255, 57, 57, 57, 255, 58, 57, 58, 255, 57, 57, 57, 255, 57, 56, 57, 255, 58, 57,
        58, 255, 58, 57, 58, 255, 58, 57, 58, 255, 58, 57, 58, 255, 57, 57, 57, 255, 58, 57, 58,
        255, // Row 0
        57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57,
        57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 59, 59,
        59, 255, 59, 59, 59, 255, 59, 59, 59, 255, 58, 58, 58, 255, 57, 57, 57, 255, 57, 57, 57,
        255, // Row 1
        58, 57, 58, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 58, 58, 58, 255, 59,
        59, 59, 255, 60, 60, 60, 255, 48, 48, 48, 255, 154, 154, 154, 255, 177, 176, 177, 255, 41,
        41, 41, 255, 45, 45, 45, 255, 46, 46, 46, 255, 54, 54, 54, 255, 59, 59, 59, 255, 57, 57,
        57, 255, // Row 2
        58, 57, 58, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 52, 52, 52, 255, 48,
        48, 48, 255, 54, 54, 54, 255, 47, 47, 47, 255, 186, 186, 186, 255, 230, 230, 230, 255, 129,
        129, 129, 255, 155, 155, 155, 255, 132, 132, 132, 255, 70, 70, 70, 255, 49, 49, 49, 255,
        59, 59, 59, 255, // Row 3
        57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 58, 58, 58, 255, 82, 82, 82, 255, 109,
        109, 109, 255, 83, 83, 83, 255, 42, 42, 42, 255, 172, 172, 172, 255, 254, 254, 254, 255,
        160, 160, 160, 255, 154, 154, 154, 255, 204, 204, 204, 255, 215, 215, 215, 255, 95, 96, 95,
        255, 50, 50, 50, 255, // Row 4
        58, 57, 58, 255, 60, 60, 60, 255, 55, 55, 55, 255, 72, 72, 72, 255, 197, 197, 197, 255,
        216, 216, 216, 255, 238, 238, 238, 255, 109, 109, 109, 255, 202, 202, 202, 255, 206, 206,
        206, 255, 86, 86, 86, 255, 223, 223, 223, 255, 197, 197, 197, 255, 213, 213, 213, 255, 198,
        198, 198, 255, 64, 64, 64, 255, // Row 5
        57, 57, 57, 255, 48, 48, 48, 255, 45, 45, 45, 255, 72, 72, 72, 255, 163, 163, 163, 255,
        157, 157, 157, 255, 238, 238, 238, 255, 188, 188, 188, 255, 245, 245, 245, 255, 200, 200,
        200, 255, 91, 91, 91, 255, 232, 232, 232, 255, 195, 195, 195, 255, 100, 100, 100, 255, 173,
        173, 173, 255, 116, 116, 116, 255, // Row 6
        55, 54, 55, 255, 145, 145, 145, 255, 119, 119, 119, 255, 145, 145, 145, 255, 255, 255, 255,
        255, 153, 153, 153, 255, 247, 247, 247, 255, 198, 198, 198, 255, 252, 252, 252, 255, 205,
        205, 205, 255, 93, 93, 93, 255, 142, 142, 142, 255, 216, 216, 216, 255, 218, 218, 218, 255,
        167, 167, 167, 255, 127, 126, 127, 255, // Row 7
        56, 55, 56, 255, 218, 218, 218, 255, 189, 189, 189, 255, 96, 96, 96, 255, 220, 220, 220,
        255, 183, 183, 183, 255, 209, 209, 209, 255, 148, 148, 148, 255, 223, 223, 223, 255, 178,
        178, 178, 255, 102, 102, 102, 255, 214, 214, 214, 255, 221, 221, 221, 255, 184, 184, 184,
        255, 200, 200, 200, 255, 91, 90, 91, 255, // Row 8
        56, 56, 56, 255, 69, 69, 69, 255, 67, 67, 67, 255, 50, 50, 50, 255, 63, 63, 63, 255, 68,
        68, 68, 255, 52, 52, 52, 255, 59, 59, 59, 255, 213, 213, 213, 255, 199, 199, 199, 255, 75,
        75, 75, 255, 68, 68, 68, 255, 100, 100, 100, 255, 183, 183, 183, 255, 160, 160, 160, 255,
        50, 50, 50, 255, // Row 9
        58, 58, 58, 255, 54, 54, 54, 255, 54, 54, 54, 255, 58, 58, 58, 255, 58, 58, 58, 255, 46,
        46, 46, 255, 69, 69, 69, 255, 198, 198, 198, 255, 149, 149, 149, 255, 131, 131, 131, 255,
        200, 200, 200, 255, 189, 189, 189, 255, 194, 194, 194, 255, 148, 148, 148, 255, 56, 56, 56,
        255, 56, 56, 56, 255, // Row 10
        57, 57, 57, 255, 58, 58, 58, 255, 58, 58, 58, 255, 58, 58, 58, 255, 45, 45, 45, 255, 87,
        87, 87, 255, 210, 210, 210, 255, 136, 136, 136, 255, 45, 45, 45, 255, 47, 47, 47, 255, 63,
        63, 63, 255, 82, 82, 82, 255, 67, 67, 67, 255, 47, 47, 47, 255, 56, 56, 56, 255, 58, 58,
        58, 255, // Row 11
        57, 57, 57, 255, 57, 57, 57, 255, 58, 58, 58, 255, 52, 52, 52, 255, 108, 108, 108, 255,
        212, 212, 212, 255, 113, 113, 113, 255, 43, 43, 43, 255, 59, 59, 59, 255, 59, 59, 59, 255,
        55, 55, 55, 255, 51, 51, 51, 255, 54, 54, 54, 255, 59, 59, 59, 255, 57, 57, 57, 255, 57,
        57, 57, 255, // Row 12
        58, 58, 58, 255, 58, 58, 58, 255, 53, 53, 53, 255, 75, 75, 75, 255, 194, 194, 194, 255, 97,
        97, 97, 255, 43, 43, 43, 255, 60, 60, 60, 255, 57, 57, 57, 255, 57, 57, 57, 255, 58, 58,
        58, 255, 58, 58, 58, 255, 58, 58, 58, 255, 57, 57, 57, 255, 57, 57, 57, 255, 58, 58, 58,
        255, // Row 13
        57, 57, 57, 255, 57, 57, 57, 255, 56, 56, 56, 255, 61, 61, 61, 255, 63, 63, 63, 255, 50,
        50, 50, 255, 59, 59, 59, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57,
        57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57, 255, 57, 57, 57,
        255, // Row 14
        58, 58, 58, 255, 57, 57, 57, 255, 58, 58, 58, 255, 57, 57, 57, 255, 56, 56, 56, 255, 59,
        59, 59, 255, 57, 57, 57, 255, 57, 57, 57, 255, 58, 58, 58, 255, 57, 57, 57, 255, 58, 58,
        58, 255, 58, 58, 58, 255, 58, 58, 58, 255, 58, 58, 58, 255, 57, 57, 57, 255, 58, 58, 58,
        255, // Row 15
    ];
    rgba_data
}

fn create_tray_icon() -> Result<Icon, Box<dyn std::error::Error + Send + Sync>> {
    let rgba_data = generate_embedded_icon_data();

    Ok(Icon::from_rgba(rgba_data, 16, 16)?)
}
pub struct TrayApp {
    _tray_icon: tray_icon::TrayIcon,
    quit_id: MenuId,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: mpsc::Receiver<()>,
}

impl TrayApp {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let menu = Menu::new();

        let quit = MenuItem::new("Quit", true, None);

        // Store the menu item ID
        let quit_id = quit.id().clone();

        menu.append(&quit)?;

        // Create icon - multiple approaches available
        let icon = create_tray_icon()?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Studio Project Manager")
            .with_icon(icon)
            .build()?;

        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        Ok(TrayApp {
            _tray_icon: tray_icon,
            quit_id,
            shutdown_tx,
            shutdown_rx,
        })
    }

    pub fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("System tray initialized, server running in background");

        let quit_id = self.quit_id.clone();
        let shutdown_tx = self.shutdown_tx.clone();

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

        let shutdown_tx_clone = shutdown_tx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id() == &quit_id {
                info!("Quit requested from tray menu");
                let _ = shutdown_tx_clone.send(());
            }
        }));

        // Main event loop
        loop {
            // Check for shutdown signal
            if let Ok(_) = self.shutdown_rx.try_recv() {
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

        info!("Shutting down Studio Project Manager");
        Ok(())
    }
}
