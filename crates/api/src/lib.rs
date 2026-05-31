use std::path::PathBuf;

use anyhow::Result;
use repomemo_domain::{AppSettings, Workspace};
use repomemo_storage::{StorageConfig, StorageEngine};

#[derive(Debug, Clone)]
pub struct RepoMemoCore {
    storage: StorageEngine,
}

impl RepoMemoCore {
    pub async fn boot(data_dir: PathBuf) -> Result<Self> {
        let storage = StorageEngine::open(StorageConfig { data_dir }).await?;
        Ok(Self { storage })
    }

    pub async fn create_workspace(&self, name: String) -> Result<Workspace> {
        self.storage.create_workspace(&name).await
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.storage.list_workspaces().await
    }

    pub fn app_settings(&self) -> AppSettings {
        AppSettings {
            data_dir: self.storage.data_dir().display().to_string(),
            ai_enabled: false,
            active_provider: None,
        }
    }
}
