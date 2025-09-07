use crate::Result;
use fjall::{Keyspace, PartitionHandle, PersistMode};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMapping {
    pub linear_issue_id: String,
    pub motion_task_id: String,
    pub sync_source: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct MappingStore {
    keyspace: Keyspace,
    mappings: PartitionHandle,
}

impl MappingStore {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let keyspace = fjall::Config::new(db_path).open()?;
        let mappings = keyspace.open_partition("task_mappings", fjall::PartitionCreateOptions::default())?;
        
        info!("Task mapping store initialized");
        
        Ok(Self {
            keyspace,
            mappings,
        })
    }

    pub async fn store_mapping(&self, mapping: TaskMapping) -> Result<()> {
        let key = format!("{}:{}", mapping.sync_source, mapping.linear_issue_id);
        let value = serde_json::to_vec(&mapping)?;
        
        self.mappings.insert(&key, &value)?;
        debug!("Stored mapping: {} -> {}", mapping.linear_issue_id, mapping.motion_task_id);
        
        Ok(())
    }

    pub async fn get_mapping_by_linear_id(&self, sync_source: &str, linear_issue_id: &str) -> Result<Option<TaskMapping>> {
        let key = format!("{}:{}", sync_source, linear_issue_id);
        
        match self.mappings.get(&key)? {
            Some(value) => {
                let mapping: TaskMapping = serde_json::from_slice(&value)?;
                debug!("Found mapping: {} -> {}", linear_issue_id, mapping.motion_task_id);
                Ok(Some(mapping))
            }
            None => {
                debug!("No mapping found for Linear issue: {}", linear_issue_id);
                Ok(None)
            }
        }
    }

    pub async fn get_mapping_by_motion_id(&self, motion_task_id: &str) -> Result<Option<TaskMapping>> {
        // Since we need to search by Motion task ID, we'll iterate through all mappings
        // For better performance in a production system, we might want to maintain a reverse index
        for item in self.mappings.iter() {
            let (_, value) = item?;
            let mapping: TaskMapping = serde_json::from_slice(&value)?;
            
            if mapping.motion_task_id == motion_task_id {
                debug!("Found mapping by Motion ID: {} -> {}", motion_task_id, mapping.linear_issue_id);
                return Ok(Some(mapping));
            }
        }
        
        debug!("No mapping found for Motion task: {}", motion_task_id);
        Ok(None)
    }

    pub async fn remove_mapping(&self, sync_source: &str, linear_issue_id: &str) -> Result<Option<TaskMapping>> {
        let key = format!("{}:{}", sync_source, linear_issue_id);
        
        let existing = match self.mappings.get(&key)? {
            Some(value) => Some(serde_json::from_slice(&value)?),
            None => None,
        };

        if existing.is_some() {
            self.mappings.remove(&key)?;
            debug!("Removed mapping for Linear issue: {}", linear_issue_id);
        }

        Ok(existing)
    }

    pub async fn list_mappings_by_source(&self, sync_source: &str) -> Result<Vec<TaskMapping>> {
        let prefix = format!("{}:", sync_source);
        let mut mappings = Vec::new();

        for item in self.mappings.iter() {
            let (key, value) = item?;
            let key_str = String::from_utf8_lossy(&key);
            
            if key_str.starts_with(&prefix) {
                let mapping: TaskMapping = serde_json::from_slice(&value)?;
                mappings.push(mapping);
            }
        }

        debug!("Found {} mappings for sync source: {}", mappings.len(), sync_source);
        Ok(mappings)
    }

    pub async fn list_all_mappings(&self) -> Result<Vec<TaskMapping>> {
        let mut mappings = Vec::new();

        for item in self.mappings.iter() {
            let (_, value) = item?;
            let mapping: TaskMapping = serde_json::from_slice(&value)?;
            mappings.push(mapping);
        }

        debug!("Found {} total mappings", mappings.len());
        Ok(mappings)
    }

    pub async fn update_mapping(&self, mapping: TaskMapping) -> Result<()> {
        let updated_mapping = TaskMapping {
            updated_at: chrono::Utc::now(),
            ..mapping
        };
        
        self.store_mapping(updated_mapping).await
    }

    pub async fn flush(&self) -> Result<()> {
        self.keyspace.persist(PersistMode::SyncAll)?;
        Ok(())
    }
}

impl TaskMapping {
    pub fn new(linear_issue_id: String, motion_task_id: String, sync_source: String) -> Self {
        let now = chrono::Utc::now();
        
        Self {
            linear_issue_id,
            motion_task_id,
            sync_source,
            created_at: now,
            updated_at: now,
        }
    }
}