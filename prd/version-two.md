Product Requirements Document: Canonical Sync Engine for Linear & Motion

    Author: Alexander Lyon (@arlyon)

    Date: 2025-09-08

    Status: Draft

    Version: 1.1 (Updated with Queue-based Architecture)

1. Introduction / Overview

This document outlines the requirements for building a new, robust synchronization engine to connect Linear and Motion for personal productivity. The existing sync process is a primitive, procedural system that is difficult to test, unreliable for continuous operation, and hard to maintain.

This project will replace the current system with a stateful, reliable service built on a Canonical Data Model. This new architecture will serve as a resilient foundation for bidirectional synchronization, treating Linear as the primary source of truth while allowing for specific, crucial status updates from Motion. The goal is to create a "set it and forget it" tool for a single developer user to automate their personal workflow.
2. Goals / Objectives

    Primary Objective: To automatically populate the user's Motion calendar with tasks assigned to them in Linear, enabling them to accurately visualize and measure their workload.

    Technical Goal: To build a highly reliable, testable, and maintainable synchronization daemon that can run continuously without data drift or manual intervention, using a modern, asynchronous, queue-based architecture.

3. Target Audience / User Personas

    Persona: A software developer (specifically, the author) who uses Linear for issue and project tracking and Motion for calendar blocking and time management.

    Needs:

        A seamless, automated way to get Linear issues into their calendar.

        Confidence that the sync is reliable and won't require debugging.

        The ability for the system to correctly handle the entire lifecycle of a task.

4. User Stories / Use Cases

    As a developer, I want all issues assigned to me in Linear to be automatically created as tasks in Motion so that I don't have to do manual data entry and my calendar is always up-to-date.

    As a developer, I want any updates to my Linear issues (title, description, due date, estimate) to be reflected in the corresponding Motion tasks so that my calendar accurately represents the current state of my work.

    As a developer, I want a task in Motion to be removed from my calendar when its corresponding Linear issue is moved to a terminal state (e.g., "Done", "Canceled") so that my calendar only shows active work.

    As a developer, I want to be able to "archive" a task in Motion to signal its completion so that the corresponding Linear issue is updated with a "Motioned" label for tracking purposes.

    As a developer, I want the system to handle initial setup by syncing all of my currently assigned Linear issues so that I can get started immediately with a complete picture of my workload.

5. Functional Requirements
5.1. Core Architecture

    The system MUST use a Canonical Data Model as an intermediary, platform-agnostic representation of a synchronized entity.

    The core synchronization engine MUST be agnostic of the source APIs (Linear, Motion). Its sole responsibility is to process a queue of Diff objects.

    All data translation logic MUST be encapsulated within source-specific "mappers" (Lenses).

    The canonical state MUST be persisted locally in an embedded key-value store (Fjall).

    All operations MUST be asynchronous, using a message queue to decouple diff generation from application.

5.2. Synchronization Logic

    Source of Truth: Linear is the definitive source of truth for all synchronized data. In any conflict, the state from Linear will win.

    Diff Generation (Producer):

        A change detected from a source (Linear webhook or Motion poll) will be mapped to a CanonicalTask.

        This new CanonicalTask will be diffed against the current state stored in Fjall.

        If changes are detected, a TaskDiff object is generated and pushed onto an in-memory queue.

    Diff Application (Consumer):

        The SyncEngine will consume TaskDiffs from the queue in batches.

        It will merge all diffs related to the same entity ID within a batch.

        For each merged diff, it will call the apply_diff method on the other system's mapper to execute the update.

5.3. Field Mappings

Linear Field


Direction


Motion Field


Notes

issue.title


→


task.name


Linear is the source of truth.

issue.description


→


task.description


Linear is the source of truth.

issue.estimate


→


task.duration


Point-to-minute conversion rule must be configurable.

issue.dueDate


→


task.dueDate


Linear is the source of truth.

issue.assignee


→


task.assigneeId


Maps the Linear user to the Motion user.

issue.labels


←


task.status: archived


Motion archived status adds a "Motioned" label in Linear.
5.4. Lifecycle Management

    Initial Sync: Upon first run for a user, the system MUST fetch and sync all open issues currently assigned to that user in Linear.

    Terminal States: When a Linear issue is moved to a terminal state (Done, Canceled, Duplicate), the system MUST generate a diff that results in the deletion of the corresponding task in Motion and removal of the entity from the local state database (Fjall).

    Deletion: If a Linear issue is deleted, the system MUST behave as if it entered a terminal state.

    Reopening: If an issue in a terminal state is re-opened in Linear, the system will treat it as a new issue and create a new corresponding task in Motion.

6. Non-Functional Requirements

    Reliability: The sync daemon MUST be able to run continuously for over one week without crashes, data drift, or inconsistencies. The queueing mechanism should prevent loss of updates during transient API failures.

    Testability: All business logic, especially in the mappers/lenses and diffing logic, MUST be unit-testable in isolation from network I/O.

    Idempotency: The entire sync process MUST be idempotent. Re-processing the same source event multiple times MUST not result in duplicated or erroneous data changes.

    Performance:

        Linear → Motion: Changes from Linear webhooks MUST be processed and pushed to the queue near-instantly. The consumer should apply the change to Motion within 3 seconds of it being dequeued.

        Motion → Linear: The system will poll Motion for status changes at a maximum frequency of once every 10 seconds.

    Security: API keys and other credentials MUST be managed securely via environment variables or a system secrets manager, not hardcoded in the source.

7. Design & Data Flow Visualization (Queue Architecture)
7.1. Diff Generation Flow (Producer)

sequenceDiagram
    participant Source API (Linear/Motion)
    participant Producer (Webhook/Poller)
    participant Source Mapper
    participant Fjall DB
    participant Diff Engine
    participant Message Queue (Diffs)

    Source API->>+Producer: Event Occurs (e.g., Webhook)
    Producer->>+Source Mapper: Map to CanonicalTask (After State)
    Source Mapper-->>-Producer: Returns CanonicalTask
    Producer->>+Fjall DB: Read Current State (Before State)
    Fjall DB-->>-Producer: Returns CanonicalTask
    Producer->>+Diff Engine: diff(before, after)
    Diff Engine-->>-Producer: Returns TaskDiff
    alt TaskDiff is Some
        Producer->>+Message Queue (Diffs): Enqueue TaskDiff
    end

7.2. Diff Application Flow (Consumer)

sequenceDiagram
    participant Message Queue (Diffs)
    participant Sync Engine (Consumer)
    participant Target Mapper
    participant Target API

    loop Batch Processing
        Sync Engine (Consumer)->>+Message Queue (Diffs): Dequeue Batch of Diffs
        Message Queue (Diffs)-->>-Sync Engine (Consumer): Diffs
        Sync Engine (Consumer)->>Sync Engine (Consumer): Group by Entity & Merge Diffs

        Note over Sync Engine (Consumer): For each merged diff...

        Sync Engine (Consumer)->>+Target Mapper: apply_diff(merged_diff)
        Target Mapper->>Target Mapper: Transform Diff to API Payload
        Target Mapper->>+Target API: POST/PATCH/DELETE
        Target API-->>-Target Mapper: Success/Failure
        Target Mapper-->>-Sync Engine (Consumer): Result
    end

8. Success Metrics

    Primary Metric: The tool is used daily by the author and reliably automates the Linear-to-Motion workflow, fulfilling the primary objective without requiring manual intervention or correction.

    Validation Criteria:

        A comprehensive suite of unit tests for the mappers, diffing logic, and canonical model passes.

        The sync daemon runs for over one week in daemon mode without data drift, crashes, or inconsistencies.

        Adding a new field mapping can be achieved with changes confined to the relevant mapper and canonical model, without altering the core diffing engine.

9. Open Questions / Future Considerations

    Accompanying Web App: A web application for configuration, status monitoring, and manual conflict resolution is currently out of scope.

    Error Handling: A dead-letter queue for persistently failing diffs should be considered for future robustness.

    Configurable Mappings: The point-to-minute conversion ratio is assumed to be fixed. Future iterations could make this user-configurable.

10. Code Draft

```rust
//! Core traits and data structures for the asynchronous, diff-based synchronization engine.
//! This version uses a "diff-the-projection" pattern for propagation.

// We assume a `diff` crate is used, which provides derive macros for `Diff` and `Apply`.
// e.g., `use diff::{Diff, Apply};`
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- Conceptual Imports from `diff` crate ---
// These stand in for the actual traits from the crate you described.
pub trait Diff: Sized {
    type Repr; // The diff representation, e.g., `CanonicalTaskDiff`.
    fn diff(&self, other: &Self) -> Self::Repr;
}
pub trait Apply<D> {
    fn apply(&mut self, diff: &D);
}
// --- End Conceptual Imports ---


// --- 1. The Canonical Data Model ---
// This is the central, platform-agnostic representation of a task.
// It can be diffed and have diffs applied to it.

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
// In a real implementation, we would derive the `Diff` and `Apply` traits.
// #[derive(Diff, Apply)]
// #[diff(attr(#[derive(Debug, Clone, Serialize, Deserialize)]))]
pub struct CanonicalTask {
    pub id: String, // A unique ID across both platforms
    pub title: String,
    pub description: Option<String>,
    pub status: CanonicalStatus,
    pub estimate_points: Option<f32>,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum CanonicalStatus {
    #[default]
    Active,
    ArchivedInMotion,
    Done, // Represents a terminal state from Linear
}

// --- 2. The Canonical Diff ---
// This struct would be GENERATED by `#[derive(Diff)]` on `CanonicalTask`.
// It represents a diff of the canonical model and is the type stored in the queue.
// I've named it `CanonicalTaskDiff` for clarity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalTaskDiff {
    // Each field is an Option, representing a potential change.
    pub title: Option<String>,
    pub description: Option<Option<String>>, // Option<Option<T>> allows setting a field to None
    pub status: Option<CanonicalStatus>,
    pub estimate_points: Option<Option<f32>>,
    pub due_date: Option<Option<chrono::DateTime<chrono::Utc>>>,
    // This identifies which system produced the diff, to avoid echo-updates.
    pub source_system: String,
}

impl CanonicalTaskDiff {
    /// Merges another diff into this one. The `other` diff's values take precedence.
    pub fn merge(&mut self, other: Self) {
        if let Some(title) = other.title { self.title = Some(title); }
        if let Some(description) = other.description { self.description = Some(description); }
        if let Some(status) = other.status { self.status = Some(status); }
        if let Some(estimate) = other.estimate_points { self.estimate_points = Some(estimate); }
        if let Some(due_date) = other.due_date { self.due_date = Some(due_date); }
        // The last source to write wins.
        self.source_system = other.source_system;
    }
}

// This `Apply` implementation would also be derived by the macro.
// This is a manual implementation to demonstrate the logic.
impl Apply<CanonicalTaskDiff> for CanonicalTask {
    fn apply(&mut self, diff: &CanonicalTaskDiff) {
        if let Some(title) = &diff.title { self.title = title.clone(); }
        if let Some(description) = &diff.description { self.description = description.clone(); }
        if let Some(status) = &diff.status { self.status = status.clone(); }
        if let Some(estimate) = diff.estimate_points { self.estimate_points = estimate; }
        if let Some(due_date) = diff.due_date { self.due_date = due_date; }
    }
}


// --- 3. The Adapter and Lens Traits ---
// An Adapter connects an external system (like Linear or Motion) to the sync engine.

#[async_trait]
pub trait Adapter {
    // The `Lens` is the projection of the canonical state into the specific
    // structure of the target system. It must be diffable.
    type Lens: Diff + Send + Sync;
    type Error: std::error::Error + Send + Sync;

    /// Projects the central canonical model into this adapter's specific lens model.
    async fn project(&self, canonical: &CanonicalTask) -> Result<Self::Lens, Self::Error>;

    /// Applies a diff of the LENS model to the external system.
    async fn apply(&self, diff: &<Self::Lens as Diff>::Repr) -> Result<(), Self::Error>;
}

// --- 4. Placeholders for Concrete Lens Implementations ---
// These show how a specific adapter would define its lens.

// #[derive(Debug, Clone, Diff)]
// #[diff(attr(#[derive(Debug, Clone)]))]
pub struct LinearLens { /* fields that match Linear's API structure */ }
// This would generate a `LinearLensDiff` struct.

// #[derive(Debug, Clone, Diff)]
// #[diff(attr(#[derive(Debug, Clone)]))]
pub struct MotionLens { /* fields that match Motion's API structure */ }
// This would generate a `MotionLensDiff` struct.


// --- 5. The Sync Engine ---
// The consumer that processes diffs from the queue and drives the adapters.

// A conceptual handle to a database like Fjall.
#[async_trait]
pub trait Database {
    type Error: std::error::Error;
    async fn get(&self, id: &str) -> Result<Option<CanonicalTask>, Self::Error>;
    async fn set(&self, id: &str, task: &CanonicalTask) -> Result<(), Self::Error>;
}

pub struct SyncEngine<A1: Adapter, A2: Adapter, DB: Database> {
    diff_queue_rx: tokio::sync::mpsc::Receiver<(String, CanonicalTaskDiff)>,
    mapper_1: A1,
    mapper_2: A2,
    db: DB,
}

impl<A1, A2, DB> SyncEngine<A1, A2, DB>
where
    A1: Adapter + Send + Sync,
    A2: Adapter + Send + Sync,
    DB: Database + Send + Sync,
{
    pub async fn run(mut self) {
        loop {
            // ... Batching logic to pull from `diff_queue_rx` ...
            let mut diff_batch = Vec::new();
            if let Some(first_diff) = self.diff_queue_rx.recv().await {
                diff_batch.push(first_diff);
                while let Ok(diff) = self.diff_queue_rx.try_recv() {
                    diff_batch.push(diff);
                }
            } else { break; } // Channel closed

            let mut merged_diffs: HashMap<String, CanonicalTaskDiff> = HashMap::new();
            for (entity_id, diff) in diff_batch {
                merged_diffs.entry(entity_id)
                    .and_modify(|d| d.merge(diff.clone()))
                    .or_insert(diff);
            }

            for (entity_id, merged_diff) in merged_diffs {
                // The core "diff-the-projection" logic starts here.
                if let Err(e) = self.propagate_change(&entity_id, &merged_diff).await {
                    eprintln!("Failed to propagate change for {}: {}", entity_id, e);
                    // Error handling: retry, dead-letter-queue, etc.
                }
            }
        }
    }

    async fn propagate_change(
        &self,
        entity_id: &str,
        diff: &CanonicalTaskDiff,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. Get the 'before' state from the database. A new task is a diff from default.
        let c_before = self.db.get(entity_id).await?.unwrap_or_default();

        // 2. Apply the canonical diff to get the 'after' state.
        let mut c_after = c_before.clone();
        c_after.apply(diff);

        // 3. Determine the target adapter to propagate the change to.
        let (target_adapter, source_name): (&dyn Adapter<Lens = _, Error = _>, _) = if diff.source_system == "Linear" {
            (&self.mapper_2, "Motion")
        } else {
            (&self.mapper_1, "Linear")
        };

        // 4. Project both 'before' and 'after' states into the target's lens.
        let lens_before = target_adapter.project(&c_before).await?;
        let lens_after = target_adapter.project(&c_after).await?;

        // 5. Diff the projected lens states to get a target-specific diff.
        let lens_diff = lens_before.diff(&lens_after);

        // 6. Apply the lens-specific diff to the target system's API.
        // Note: The diff crate might produce a diff that indicates no change,
        // so we'd need a way to check if the diff is empty before applying.
        // `if !lens_diff.is_empty()`
        target_adapter.apply(&lens_diff).await?;

        // 7. If successful, persist the new canonical 'after' state to our DB.
        self.db.set(entity_id, &c_after).await?;

        println!("Successfully propagated change from {} to {}.", diff.source_system, source_name);
        Ok(())
    }
}
```
