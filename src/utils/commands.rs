use std::sync::{Arc, Mutex};

use tray_item::TrayItem;

use super::tray;

#[derive(Clone)]
pub(crate) enum AppCommands {
    Quit,
    Reload,
    Recenter(bool),
    ToggleSettings(ToggleSetting),
}

#[derive(Clone)]
pub(crate) enum ToggleSetting {
    FlipX,
    FlipY,
    SwapEyes,
    AmbientLight,
}

pub(crate) struct AppContext {
    pub state: Arc<Mutex<AppState>>,
    #[cfg(not(target_os = "android"))]
    _tray: TrayItem,
}

pub(crate) struct AppState {
    pub message: Option<&'static AppCommands>,
}

pub(crate) struct RecenterRequest {
    pub delay: i64,
    pub horizon_locked: bool,
}

impl AppContext {
    pub fn new() -> anyhow::Result<Self> {
        let state = Arc::new(Mutex::new(AppState { message: None }));

        #[cfg(not(target_os = "android"))]
        let tray = tray::build_tray(&state)?;

        Ok(Self {
            state,
            #[cfg(not(target_os = "android"))]
            _tray: tray,
        })
    }
}
