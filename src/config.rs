use directories::ProjectDirs;
use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not get project directory")]
    Dirs,

    #[error("I/O Error")]
    Io(#[from] std::io::Error),

    #[error("Failed to serialize config to file")]
    Serialize(#[from] ron::Error),
}

#[derive(Debug)]
pub struct Config {
    dirs: ProjectDirs,
    data: ConfigData,
}

#[derive(Debug, Deserialize, Serialize)]
struct ConfigData {
    window_width: u32,
    window_height: u32,
}

impl Config {
    pub fn new() -> Result<Self, Error> {
        let dirs = ProjectDirs::from("org", "KodeWerx", "EdgeScan").ok_or(Error::Dirs)?;

        let mut path = dirs.config_dir().to_path_buf();
        path.push("config.ron");

        let mut data = if let Ok(contents) = std::fs::read_to_string(path) {
            ron::from_str(&contents).unwrap_or_default()
        } else {
            ConfigData::default()
        };

        // Do not trust user input: Normalize the configuration data before use.
        data.normalize();

        Ok(Self { dirs, data })
    }

    /// Save configuration.
    ///
    /// The config file is created if it does not exist, along with all intermediate directories in
    /// the path.
    pub fn save(&self) -> Result<(), Error> {
        let mut path = self.dirs.config_dir().to_path_buf();
        std::fs::create_dir_all(&path)?;
        path.push("config.ron");

        let contents = ron::to_string(&self.data)?;
        std::fs::write(path, contents)?;

        Ok(())
    }

    pub fn get_window_size(&self) -> (u32, u32) {
        (self.data.window_width, self.data.window_height)
    }

    pub(crate) fn set_window_size(&mut self, width: u32, height: u32, scale_factor: f64) {
        self.data.window_width = (width as f64 / scale_factor) as u32;
        self.data.window_height = (height as f64 / scale_factor) as u32;
    }
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            window_width: 1200,
            window_height: 800,
        }
    }
}

impl ConfigData {
    fn normalize(&mut self) {
        // TODO: Max might be more than the `wgpu` adapter supports.
        self.window_width = self.window_width.clamp(400, 10000);
        self.window_height = self.window_height.clamp(400, 10000);
    }
}
