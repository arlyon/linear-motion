use crate::Result;
use fjall::{Keyspace, PartitionHandle, PersistMode};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncStatus {
    InProgress,
    Completed,
    Failed,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusEntry {
    pub id: String,
    pub sync_source: String,
    pub linear_issue_id: String,
    pub motion_task_id: Option<String>,
    pub status: SyncStatus,
    pub last_sync_attempt: chrono::DateTime<chrono::Utc>,
    pub error_message: Option<String>,
    pub retry_count: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSourceStatus {
    pub source_name: String,
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,
    pub total_issues_processed: u64,
    pub successful_syncs: u64,
    pub failed_syncs: u64,
    pub errors: Vec<String>,
}

pub struct StatusStore {
    keyspace: Keyspace,
    statuses: PartitionHandle,
    source_stats: PartitionHandle,
}

impl StatusStore {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let keyspace = fjall::Config::new(db_path).open()?;
        let statuses =
            keyspace.open_partition("sync_statuses", fjall::PartitionCreateOptions::default())?;
        let source_stats =
            keyspace.open_partition("source_stats", fjall::PartitionCreateOptions::default())?;

        debug!("status tracking store initialized");

        Ok(Self {
            keyspace,
            statuses,
            source_stats,
        })
    }

    pub async fn create_status_entry(
        &self,
        sync_source: String,
        linear_issue_id: String,
    ) -> Result<SyncStatusEntry> {
        let entry = SyncStatusEntry::new(sync_source, linear_issue_id);
        self.store_status_entry(&entry).await?;
        Ok(entry)
    }

    pub async fn store_status_entry(&self, entry: &SyncStatusEntry) -> Result<()> {
        let key = &entry.id;
        let value = serde_json::to_vec(entry)?;

        self.statuses.insert(key, &value)?;
        debug!(
            "Stored status entry: {} ({})",
            entry.id, entry.linear_issue_id
        );

        Ok(())
    }

    pub async fn get_status_entry(&self, id: &str) -> Result<Option<SyncStatusEntry>> {
        match self.statuses.get(id)? {
            Some(value) => {
                let entry: SyncStatusEntry = serde_json::from_slice(&value)?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    pub async fn update_status(
        &self,
        id: &str,
        status: SyncStatus,
        error_message: Option<String>,
    ) -> Result<()> {
        if let Some(mut entry) = self.get_status_entry(id).await? {
            entry.status = status;
            entry.error_message = error_message.clone();
            entry.last_sync_attempt = chrono::Utc::now();
            entry.updated_at = chrono::Utc::now();

            if error_message.is_some() {
                entry.retry_count += 1;
            }

            self.store_status_entry(&entry).await?;
            debug!("Updated status for {}: {:?}", id, entry.status);
        } else {
            warn!("Attempted to update non-existent status entry: {}", id);
        }

        Ok(())
    }

    pub async fn mark_completed(&self, id: &str, motion_task_id: String) -> Result<()> {
        if let Some(mut entry) = self.get_status_entry(id).await? {
            entry.status = SyncStatus::Completed;
            entry.motion_task_id = Some(motion_task_id);
            entry.last_sync_attempt = chrono::Utc::now();
            entry.updated_at = chrono::Utc::now();
            entry.error_message = None;

            self.store_status_entry(&entry).await?;
            debug!("Marked {} as completed", id);
        }

        Ok(())
    }

    pub async fn mark_failed(&self, id: &str, error: String) -> Result<()> {
        self.update_status(id, SyncStatus::Failed, Some(error))
            .await
    }

    pub async fn list_statuses_by_source(&self, sync_source: &str) -> Result<Vec<SyncStatusEntry>> {
        let mut entries = Vec::new();

        for item in self.statuses.iter() {
            let (_, value) = item?;
            let entry: SyncStatusEntry = serde_json::from_slice(&value)?;

            if entry.sync_source == sync_source {
                entries.push(entry);
            }
        }

        debug!(
            "Found {} status entries for source: {}",
            entries.len(),
            sync_source
        );
        Ok(entries)
    }

    pub async fn list_failed_entries(&self) -> Result<Vec<SyncStatusEntry>> {
        let mut failed_entries = Vec::new();

        for item in self.statuses.iter() {
            let (_, value) = item?;
            let entry: SyncStatusEntry = serde_json::from_slice(&value)?;

            if matches!(entry.status, SyncStatus::Failed) {
                failed_entries.push(entry);
            }
        }

        debug!("Found {} failed status entries", failed_entries.len());
        Ok(failed_entries)
    }

    pub async fn get_source_status(&self, source_name: &str) -> Result<Option<SyncSourceStatus>> {
        match self.source_stats.get(source_name)? {
            Some(value) => {
                let status: SyncSourceStatus = serde_json::from_slice(&value)?;
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    pub async fn update_source_stats(
        &self,
        source_name: &str,
        success: bool,
        error: Option<String>,
    ) -> Result<()> {
        let mut status = self
            .get_source_status(source_name)
            .await?
            .unwrap_or_else(|| SyncSourceStatus::new(source_name.to_string()));

        status.last_sync = Some(chrono::Utc::now());
        status.total_issues_processed += 1;

        if success {
            status.successful_syncs += 1;
        } else {
            status.failed_syncs += 1;
            if let Some(err) = error {
                status.errors.push(err);
                // Keep only the last 10 errors
                if status.errors.len() > 10 {
                    status.errors.remove(0);
                }
            }
        }

        let value = serde_json::to_vec(&status)?;
        self.source_stats.insert(source_name, &value)?;

        debug!(
            "Updated stats for source {}: {} successful, {} failed",
            source_name, status.successful_syncs, status.failed_syncs
        );

        Ok(())
    }

    pub async fn list_all_source_stats(&self) -> Result<Vec<SyncSourceStatus>> {
        let mut stats = Vec::new();

        for item in self.source_stats.iter() {
            let (_, value) = item?;
            let status: SyncSourceStatus = serde_json::from_slice(&value)?;
            stats.push(status);
        }

        Ok(stats)
    }

    pub async fn cleanup_old_entries(&self, older_than_days: u64) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(older_than_days as i64);
        let mut deleted_count = 0;

        let mut keys_to_delete = Vec::new();

        for item in self.statuses.iter() {
            let (key, value) = item?;
            let entry: SyncStatusEntry = serde_json::from_slice(&value)?;

            if entry.created_at < cutoff && matches!(entry.status, SyncStatus::Completed) {
                keys_to_delete.push(key.to_vec());
            }
        }

        for key in keys_to_delete {
            self.statuses.remove(&key)?;
            deleted_count += 1;
        }

        if deleted_count > 0 {
            info!("Cleaned up {} old status entries", deleted_count);
        }

        Ok(deleted_count)
    }

    pub async fn flush(&self) -> Result<()> {
        self.keyspace.persist(PersistMode::SyncAll)?;
        Ok(())
    }
}

impl SyncStatusEntry {
    pub fn new(sync_source: String, linear_issue_id: String) -> Self {
        let now = chrono::Utc::now();

        Self {
            id: Uuid::new_v4().to_string(),
            sync_source,
            linear_issue_id,
            motion_task_id: None,
            status: SyncStatus::InProgress,
            last_sync_attempt: now,
            error_message: None,
            retry_count: 0,
            created_at: now,
            updated_at: now,
        }
    }
}

impl SyncSourceStatus {
    pub fn new(source_name: String) -> Self {
        Self {
            source_name,
            last_sync: None,
            total_issues_processed: 0,
            successful_syncs: 0,
            failed_syncs: 0,
            errors: Vec::new(),
        }
    }
}
