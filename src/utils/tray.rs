use std::sync::{Arc, Mutex};

use tray_item::{IconSource, TrayItem};

use crate::{loaders::StereoMode, utils::commands::ToggleSetting};

use super::commands::{AppCommands, AppState};

#[cfg_attr(feature = "profiling", profiling::function)]
fn add_tray_message_sender(
    tray_state: &Arc<Mutex<AppState>>,
    tray: &mut TrayItem,
    entry_name: &'static str,
    message: &'static AppCommands,
) -> anyhow::Result<()> {
    let cloned_state = tray_state.clone();
    Ok(tray.add_menu_item(entry_name, move || {
        if let Ok(mut locked_state) = cloned_state.lock() {
            locked_state.message = Some(message);
        }
    })?)
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn add_all_tray_message_senders(
    tray_state: &Arc<Mutex<AppState>>,
    tray: &mut TrayItem,
    entries: &[(&'static str, &'static AppCommands)],
) -> anyhow::Result<()> {
    for (entry_name, message) in entries {
        add_tray_message_sender(tray_state, tray, entry_name, message)?;
    }
    Ok(())
}

#[cfg_attr(feature = "profiling", profiling::function)]
pub(crate) fn build_tray(tray_state: &Arc<Mutex<AppState>>) -> anyhow::Result<TrayItem> {
    log::info!("Building system tray");
    let mut tray = TrayItem::new("VR Screen Cap", IconSource::Resource("tray-icon"))?;

    tray.add_label("Settings")?;
    add_all_tray_message_senders(
        tray_state,
        &mut tray,
        &[
            (
                "Swap Eyes",
                &AppCommands::ToggleSettings(ToggleSetting::SwapEyes),
            ),
            ("Flip X", &AppCommands::ToggleSettings(ToggleSetting::FlipX)),
            ("Flip Y", &AppCommands::ToggleSettings(ToggleSetting::FlipY)),
            (
                "Toggle Ambient Light",
                &AppCommands::ToggleSettings(ToggleSetting::AmbientLight),
            ),
        ],
    )?;

    tray.add_label("Desktop Settings")?;
    add_all_tray_message_senders(
        tray_state,
        &mut tray,
        &[
            ("Mono", &AppCommands::SetStereoMode(StereoMode::Mono)),
            ("Half-SBS", &AppCommands::SetStereoMode(StereoMode::Sbs)),
            ("Full-SBS", &AppCommands::SetStereoMode(StereoMode::FullSbs)),
            ("Half-TAB", &AppCommands::SetStereoMode(StereoMode::Tab)),
            ("Full-TAB", &AppCommands::SetStereoMode(StereoMode::FullTab)),
        ],
    )?;

    tray.add_label("Actions")?;
    add_all_tray_message_senders(
        tray_state,
        &mut tray,
        &[
            ("Reload Screen", &AppCommands::Reload),
            ("Recenter", &AppCommands::Recenter(true)),
            ("Recenter w/ Pitch", &AppCommands::Recenter(false)),
            ("Quit", &AppCommands::Quit),
        ],
    )?;

    Ok(tray)
}
