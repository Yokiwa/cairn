//! Cairn core library: all launcher logic, independent of any UI.

use std::sync::Arc;

pub mod auth;
pub mod download;
pub mod endpoints;
pub mod error;
pub mod fsutil;
pub mod http;
pub mod instances;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod paths;
pub mod settings;
pub mod tasks;
pub mod versions;

pub use endpoints::Endpoints;
pub use error::{AppError, Result};
pub use paths::Paths;

use auth::store::{KeyringStore, MemoryStore, SecretStore};
use auth::AuthService;
use download::Downloader;
use http::HttpClient;
use instances::InstanceStore;
use settings::Settings;
use tasks::TaskManager;

/// Everything the launcher needs, wired together once.
pub struct Core {
    pub paths: Paths,
    pub http: HttpClient,
    pub downloader: Downloader,
    pub endpoints: Endpoints,
    pub tasks: TaskManager,
    pub instances: InstanceStore,
    pub auth: AuthService,
}

impl Core {
    /// The real thing: production endpoints and the OS keychain.
    pub fn new(paths: Paths) -> Result<Core> {
        Self::with_secret_store(paths, Endpoints::default(), Arc::new(KeyringStore))
    }

    /// Custom endpoints with an in-memory secret store (tests).
    pub fn with_endpoints(paths: Paths, endpoints: Endpoints) -> Result<Core> {
        Self::with_secret_store(paths, endpoints, Arc::new(MemoryStore::default()))
    }

    pub fn with_secret_store(
        paths: Paths,
        endpoints: Endpoints,
        store: Arc<dyn SecretStore>,
    ) -> Result<Core> {
        let settings = Settings::load(&paths)?;
        let http = HttpClient::new()?;
        let downloader = Downloader::new(http.clone(), settings.download_threads as usize);
        let instances = InstanceStore::new(paths.clone());
        let auth = AuthService::new(paths.clone(), http.clone(), endpoints.msa.clone(), store);
        Ok(Core {
            paths,
            http,
            downloader,
            endpoints,
            tasks: TaskManager::new(),
            instances,
            auth,
        })
    }
}
