use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

use super::ConfigLoader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub motion_api_key: String,
    pub sync_sources: Vec<SyncSource>,
    pub global_sync_rules: SyncRules,
    pub database_path: Option<String>,
    pub polling_interval_seconds: u64,
    pub schedule_overrides: Option<Vec<ScheduleOverride>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSource {
    pub name: String,
    pub linear_api_key: String,
    pub projects: Option<Vec<String>>,
    pub webhook_base_url: Option<String>,
    pub sync_rules: Option<SyncRules>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRules {
    pub default_task_duration_mins: u32,
    pub completed_linear_tag: String,
    pub time_estimate_strategy: TimeEstimateStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEstimateStrategy {
    pub fibonacci: Option<HashMap<String, u32>>,
    pub tshirt: Option<HashMap<String, u32>>,
    pub linear: Option<HashMap<String, u32>>,
    pub points: Option<HashMap<String, u32>>,
    pub default_duration_mins: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleOverride {
    pub name: String,
    pub interval_seconds: u64,
    pub start_time: String, // HH:MM format
    pub end_time: String,   // HH:MM format
    pub days: Vec<String>,  // mon, tue, wed, thu, fri, sat, sun
}

impl SyncSource {
    pub fn effective_sync_rules(&self, global_rules: &SyncRules) -> SyncRules {
        match &self.sync_rules {
            Some(source_rules) => source_rules.clone(),
            None => global_rules.clone(),
        }
    }
}

impl TimeEstimateStrategy {
    pub fn convert_estimate(&self, estimate: f64, estimate_type: &str) -> Option<u32> {
        let estimate_key = estimate.to_string();

        match estimate_type.to_lowercase().as_str() {
            "fibonacci" => self.fibonacci.as_ref()?.get(&estimate_key).copied(),
            "tshirt" | "t-shirt" => self.tshirt.as_ref()?.get(&estimate_key).copied(),
            "linear" => self.linear.as_ref()?.get(&estimate_key).copied(),
            "points" => self.points.as_ref()?.get(&estimate_key).copied(),
            _ => None,
        }
    }

    pub fn convert_estimate_by_value(&self, estimate: f64) -> Option<u32> {
        let estimate_key = estimate.to_string();

        // Try each strategy in order
        if let Some(mappings) = &self.fibonacci {
            if let Some(duration) = mappings.get(&estimate_key) {
                return Some(*duration);
            }
        }

        if let Some(mappings) = &self.tshirt {
            if let Some(duration) = mappings.get(&estimate_key) {
                return Some(*duration);
            }
        }

        if let Some(mappings) = &self.linear {
            if let Some(duration) = mappings.get(&estimate_key) {
                return Some(*duration);
            }
        }

        if let Some(mappings) = &self.points {
            if let Some(duration) = mappings.get(&estimate_key) {
                return Some(*duration);
            }
        }

        // Fall back to default duration
        self.default_duration_mins
    }
}

impl AppConfig {
    pub fn validate(&self) -> crate::Result<()> {
        use crate::Error;

        // Validate Motion API key
        if self.motion_api_key.trim().is_empty()
            || self.motion_api_key == "your_motion_api_key_here"
        {
            return Err(Error::Validation("Motion API key is required".to_string()));
        }

        // Validate sync sources
        if self.sync_sources.is_empty() {
            return Err(Error::Validation(
                "At least one sync source is required".to_string(),
            ));
        }

        for (idx, source) in self.sync_sources.iter().enumerate() {
            if source.linear_api_key.trim().is_empty()
                || source.linear_api_key == "your_linear_api_key_here"
            {
                return Err(Error::Validation(format!(
                    "Linear API key is required for sync source {} ({})",
                    idx, source.name
                )));
            }

            if let Some(projects) = &source.projects {
                if projects.is_empty() {
                    return Err(Error::Validation(format!(
                        "At least one project is required for source projects filter {} ({})",
                        idx, source.name
                    )));
                }
            }

            if source.name.trim().is_empty() {
                return Err(Error::Validation(format!(
                    "Name is required for sync source {}",
                    idx
                )));
            }
        }

        // Validate database path
        if let Some(true) = self.database_path.as_ref().map(|p| p.trim().is_empty()) {
            return Err(Error::Validation("Database path is required".to_string()));
        }

        // Validate schedule overrides
        if let Some(overrides) = &self.schedule_overrides {
            for (idx, override_config) in overrides.iter().enumerate() {
                self.validate_schedule_override(idx, override_config)?;
            }
        }

        Ok(())
    }

    fn validate_schedule_override(
        &self,
        idx: usize,
        override_config: &ScheduleOverride,
    ) -> crate::Result<()> {
        use crate::Error;

        // Validate time format (HH:MM)
        if !self.is_valid_time_format(&override_config.start_time) {
            return Err(Error::Validation(format!(
                "Invalid start_time format for schedule override {}: expected HH:MM",
                idx
            )));
        }

        if !self.is_valid_time_format(&override_config.end_time) {
            return Err(Error::Validation(format!(
                "Invalid end_time format for schedule override {}: expected HH:MM",
                idx
            )));
        }

        // Validate days
        let valid_days = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
        for day in &override_config.days {
            if !valid_days.contains(&day.as_str()) {
                return Err(Error::Validation(format!(
                    "Invalid day '{}' in schedule override {}. Valid days: {}",
                    day,
                    idx,
                    valid_days.join(", ")
                )));
            }
        }

        Ok(())
    }

    pub fn database_path(&self) -> PathBuf {
        return self
            .database_path
            .as_deref()
            .unwrap_or(
                ConfigLoader::get_default_database_path()
                    .unwrap()
                    .as_os_str()
                    .to_str()
                    .unwrap(),
            )
            .into();
    }

    fn is_valid_time_format(&self, time_str: &str) -> bool {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 2 {
            return false;
        }

        if let (Ok(hour), Ok(minute)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            hour < 24 && minute < 60
        } else {
            false
        }
    }
}
