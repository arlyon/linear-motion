pub mod mapping;
pub mod status;

use crate::Result;
use std::path::Path;

pub struct SyncDatabase {
    pub mappings: mapping::MappingStore,
    pub status: status::StatusStore,
}

impl SyncDatabase {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_path = db_path.as_ref();
        
        // Create the database directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let mappings = mapping::MappingStore::new(db_path).await?;
        let status = status::StatusStore::new(db_path).await?;
        
        Ok(Self {
            mappings,
            status,
        })
    }

    pub async fn initialize_with_default_path() -> Result<Self> {
        use crate::config::ConfigLoader;
        
        ConfigLoader::ensure_data_dir()?;
        let db_path = ConfigLoader::get_default_database_path()?;
        
        Self::new(db_path).await
    }

    pub async fn flush(&self) -> Result<()> {
        self.mappings.flush().await?;
        self.status.flush().await?;
        Ok(())
    }
}

pub use mapping::{TaskMapping, MappingStatus};
pub use status::{SyncStatus, SyncStatusEntry, SyncSourceStatus};