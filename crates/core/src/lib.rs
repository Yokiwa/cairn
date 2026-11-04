//! Cairn core library: all launcher logic, independent of any UI.

pub mod auth;
pub mod download;
pub mod endpoints;
pub mod error;
pub mod fsutil;
pub mod http;
pub mod paths;
pub mod settings;
pub mod tasks;

pub use endpoints::Endpoints;
pub use error::{AppError, Result};
pub use paths::Paths;

use download::Downloader;
use http::HttpClient;
use settings::Settings;
use tasks::TaskManager;

/// Everything the launcher needs, wired together once.
pub struct Core {
    pub paths: Paths,
    pub http: HttpClient,
    pub downloader: Downloader,
    pub endpoints: Endpoints,
    pub tasks: TaskManager,
}

impl Core {
    pub fn with_endpoints(paths: Paths, endpoints: Endpoints) -> Result<Core> {
        let settings = Settings::load(&paths)?;
        let http = HttpClient::new()?;
        let downloader = Downloader::new(http.clone(), settings.download_threads as usize);
        Ok(Core {
            paths,
            http,
            downloader,
            endpoints,
            tasks: TaskManager::new(),
        })
    }
}
