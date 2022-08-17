
use std::{sync::mpsc::{Receiver, channel}, time::Duration, error::Error};

use notify::{Watcher, DebouncedEvent, watcher, RecursiveMode, RecommendedWatcher};
use serde::{Serialize, Deserialize};
use clap::Parser;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScreenParamsUniform {
    x_curvature: f32,
    y_curvature: f32,
    eye_offset: f32,
    y_offset: f32,
    x_offset: f32,
}

#[derive(Parser)]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct AppConfig {
    // Maximum depth at the center in meters, default: 0.4, usage: --x-curvature=0.4
    #[clap(short, long, value_parser, default_value_t = 0.4)]
    pub x_curvature: f32,
    // Maximum depth at the center in meters, default: 0.08, usage: --y-curvature=0.08
    #[clap(long, value_parser, default_value_t = 0.08)]
    pub y_curvature: f32,
    // default: true, usage: --swap-eyes=true (geo11 has them swapped, might be GPU dependent)
    #[clap(long, value_parser, default_value_t = true)]
    pub swap_eyes: bool,
    // default: false, usage: --flip-x=false
    #[clap(long, value_parser, default_value_t = false)]
    pub flip_x: bool,
    // default: false, usage: --flip-y=false
    #[clap(long, value_parser, default_value_t = false)]
    pub flip_y: bool,
    // Distance from user in meters, default: 20.0, usage: --distance=20.0
    #[clap(short, long, value_parser, default_value_t = 20.0)]
    pub distance: f32,
    // Screen scaling factor (screen width in meters), default: 40.0, usage: --scale=40.0
    #[clap(short, long, value_parser, default_value_t = 40.0)]
    pub scale: f32,
    // Configuration file to watch for live changes, usage: --config-file=config.json
    #[clap(short, long, value_parser)]
    pub config_file: Option<String>,
}

impl AppConfig {
    pub fn uniform(&self) -> ScreenParamsUniform {
        ScreenParamsUniform {
            x_curvature: self.x_curvature,
            y_curvature: self.y_curvature,
            eye_offset: match self.swap_eyes { 
                true => 1.0,
                _ => 0.0
            },
            y_offset: match self.flip_y { 
                true => 1.0,
                _ => 0.0
            },
            x_offset: match self.flip_x { 
                true => 1.0,
                _ => 0.0
            },
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            x_curvature: 0.4,
            y_curvature: 0.08,
            swap_eyes: true,
            flip_x: false,
            flip_y: false,
            distance: 20.0,
            scale: 40.0,
            config_file: None,
        }
    }
}

//Notifications

pub struct ConfigContext {
    pub config_notifier: Option<Receiver<DebouncedEvent>>,
    pub config_watcher: Option<RecommendedWatcher>,
    pub config_file: Option<String>,
    pub last_config: Option<AppConfig>,
}

impl ConfigContext {
    pub fn try_setup() -> Result<Option<ConfigContext>, Box<dyn Error>> {
        let config = AppConfig::parse();
        if let Some(config_file_path) = config.config_file {
            log::info!("Using config file: {}", config_file_path);
            let params = serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(config_file_path.clone())?))?;
            let (tx, rx) = channel();
            let mut watcher = watcher(tx, Duration::from_secs(1))?;
            watcher.watch(config_file_path.clone(), RecursiveMode::NonRecursive)?;
            return Ok(Some(ConfigContext {
                config_notifier: Some(rx),
                config_watcher: Some(watcher),
                config_file: Some(config_file_path),
                last_config: Some(params),
            }));
        }
        Ok(None)
    }

    pub fn update_config(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(config_file_path) = self.config_file.clone() {
            let params = serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(config_file_path)?))?;
            self.last_config = Some(params);
        }
        Ok(())
    }
}