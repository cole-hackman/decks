//! Staged ChangeManager — accumulate, diff, and apply library changes.
//!
//! Vendored and adapted from reklawdbox `src/changes.rs` (MIT, Ryan Voitiskis).

pub mod applier;
pub mod key_format;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeStatus {
    Proposed,
    Accepted,
    Rejected,
    Exported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    TrackMetadataEdit,
    TrackDelete,
    /// The file moved on disk; `master.db` needs to be told where it went.
    /// Distinct from `TrackMetadataEdit` because it touches several columns at
    /// once and because the filesystem move has already happened by the time
    /// this is staged — a rejected relocate leaves the database pointing at a
    /// file that is no longer there.
    TrackRelocate,
    CueMetadataEdit,
    TrackAddCue,
    TrackDeleteCue,
    PlaylistCreate,
    PlaylistRename,
    PlaylistDelete,
    PlaylistAddTrack,
    PlaylistRemoveTrack,
    PlaylistReorderTrack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagedChange {
    pub id: String,
    pub library_path: Option<String>,
    pub kind: ChangeKind,
    pub target_id: Option<String>,
    pub field: Option<String>,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub reason: Option<String>,
    pub confidence: Option<f64>,
    pub status: ChangeStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewChange {
    pub library_path: Option<String>,
    pub kind: ChangeKind,
    pub target_id: Option<String>,
    pub field: Option<String>,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub reason: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChangeError {
    #[error("change {0} was not found")]
    NotFound(String),
    #[error("cannot transition change from {from:?} to {to:?}")]
    InvalidTransition {
        from: ChangeStatus,
        to: ChangeStatus,
    },
}

#[derive(Debug, Default)]
pub struct ChangeManager {
    changes: Vec<StagedChange>,
}

impl ChangeManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stage(&mut self, input: NewChange) -> StagedChange {
        let now = unix_timestamp();
        let change = StagedChange {
            id: new_change_id(),
            library_path: input.library_path,
            kind: input.kind,
            target_id: input.target_id,
            field: input.field,
            old_value: input.old_value,
            new_value: input.new_value,
            reason: input.reason,
            confidence: input.confidence,
            status: ChangeStatus::Proposed,
            created_at: now,
            updated_at: now,
        };
        self.changes.push(change.clone());
        change
    }

    pub fn list(&self) -> &[StagedChange] {
        &self.changes
    }

    pub fn accept(&mut self, id: &str) -> Result<StagedChange, ChangeError> {
        self.transition(id, ChangeStatus::Accepted)
    }

    pub fn reject(&mut self, id: &str) -> Result<StagedChange, ChangeError> {
        self.transition(id, ChangeStatus::Rejected)
    }

    pub fn mark_exported(&mut self, id: &str) -> Result<StagedChange, ChangeError> {
        self.transition(id, ChangeStatus::Exported)
    }

    pub fn accept_all_safe(&mut self) -> Vec<StagedChange> {
        self.changes
            .iter_mut()
            .filter(|change| {
                change.status == ChangeStatus::Proposed && is_safe_batch_kind(&change.kind)
            })
            .map(|change| {
                change.status = ChangeStatus::Accepted;
                change.updated_at = unix_timestamp();
                change.clone()
            })
            .collect()
    }

    pub fn reject_all(&mut self) -> Vec<StagedChange> {
        self.changes
            .iter_mut()
            .filter(|change| change.status == ChangeStatus::Proposed)
            .map(|change| {
                change.status = ChangeStatus::Rejected;
                change.updated_at = unix_timestamp();
                change.clone()
            })
            .collect()
    }

    fn transition(&mut self, id: &str, to: ChangeStatus) -> Result<StagedChange, ChangeError> {
        let change = self
            .changes
            .iter_mut()
            .find(|change| change.id == id)
            .ok_or_else(|| ChangeError::NotFound(id.to_owned()))?;
        ensure_transition(&change.status, &to)?;
        change.status = to;
        change.updated_at = unix_timestamp();
        Ok(change.clone())
    }
}

pub fn ensure_transition(from: &ChangeStatus, to: &ChangeStatus) -> Result<(), ChangeError> {
    let valid = matches!(
        (from, to),
        (ChangeStatus::Proposed, ChangeStatus::Accepted)
            | (ChangeStatus::Proposed, ChangeStatus::Rejected)
            | (ChangeStatus::Accepted, ChangeStatus::Exported)
    );
    if valid {
        Ok(())
    } else {
        Err(ChangeError::InvalidTransition {
            from: from.clone(),
            to: to.clone(),
        })
    }
}

pub fn is_safe_batch_kind(kind: &ChangeKind) -> bool {
    matches!(
        kind,
        ChangeKind::TrackMetadataEdit | ChangeKind::CueMetadataEdit
    )
}

pub fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

fn new_change_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    // A process-local atomic disambiguates IDs even when two calls land in the
    // same nanosecond — which happens in tight staging loops on fast hardware.
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("change_{nanos}_{n}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track_edit() -> NewChange {
        NewChange {
            library_path: Some("/library/master.db".to_owned()),
            kind: ChangeKind::TrackMetadataEdit,
            target_id: Some("track-1".to_owned()),
            field: Some("genre".to_owned()),
            old_value: Some(Value::String("House".to_owned())),
            new_value: Some(Value::String("Deep House".to_owned())),
            reason: Some("Normalize genre".to_owned()),
            confidence: Some(0.9),
        }
    }

    #[test]
    fn stages_changes_as_proposed() {
        let mut manager = ChangeManager::new();
        let change = manager.stage(track_edit());
        assert_eq!(change.status, ChangeStatus::Proposed);
        assert_eq!(change.field.as_deref(), Some("genre"));
        assert_eq!(manager.list().len(), 1);
    }

    #[test]
    fn accepts_or_rejects_only_proposed_changes() {
        let mut manager = ChangeManager::new();
        let change = manager.stage(track_edit());
        let accepted = manager.accept(&change.id).unwrap();
        assert_eq!(accepted.status, ChangeStatus::Accepted);
        let err = manager.reject(&change.id).unwrap_err();
        assert_eq!(
            err,
            ChangeError::InvalidTransition {
                from: ChangeStatus::Accepted,
                to: ChangeStatus::Rejected
            }
        );
    }

    #[test]
    fn exported_requires_accepted_status() {
        let mut manager = ChangeManager::new();
        let change = manager.stage(track_edit());
        let err = manager.mark_exported(&change.id).unwrap_err();
        assert_eq!(
            err,
            ChangeError::InvalidTransition {
                from: ChangeStatus::Proposed,
                to: ChangeStatus::Exported
            }
        );
        manager.accept(&change.id).unwrap();
        assert_eq!(
            manager.mark_exported(&change.id).unwrap().status,
            ChangeStatus::Exported
        );
    }

    #[test]
    fn batch_accepts_only_safe_metadata_changes() {
        let mut manager = ChangeManager::new();
        manager.stage(track_edit());
        manager.stage(NewChange {
            kind: ChangeKind::PlaylistDelete,
            ..track_edit()
        });

        let accepted = manager.accept_all_safe();
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].kind, ChangeKind::TrackMetadataEdit);
        assert_eq!(manager.list()[1].status, ChangeStatus::Proposed);
    }

    #[test]
    fn accept_returns_not_found_for_unknown_id() {
        let mut manager = ChangeManager::new();
        let err = manager.accept("does-not-exist").unwrap_err();
        assert_eq!(err, ChangeError::NotFound("does-not-exist".to_owned()));
    }

    #[test]
    fn track_add_cue_is_not_a_safe_batch_kind() {
        // TrackAddCue mutates cue state on a track — it should require explicit accept,
        // not be swept up by accept_all_safe. Guard the safety set so future additions
        // don't accidentally include it.
        assert!(!is_safe_batch_kind(&ChangeKind::TrackAddCue));
        assert!(!is_safe_batch_kind(&ChangeKind::PlaylistCreate));
        assert!(!is_safe_batch_kind(&ChangeKind::PlaylistDelete));
        assert!(!is_safe_batch_kind(&ChangeKind::PlaylistAddTrack));
        assert!(!is_safe_batch_kind(&ChangeKind::PlaylistRemoveTrack));
        assert!(!is_safe_batch_kind(&ChangeKind::PlaylistReorderTrack));
        assert!(!is_safe_batch_kind(&ChangeKind::PlaylistRename));
        assert!(is_safe_batch_kind(&ChangeKind::TrackMetadataEdit));
        assert!(is_safe_batch_kind(&ChangeKind::CueMetadataEdit));
    }

    #[test]
    fn staged_changes_have_unique_ids() {
        let mut manager = ChangeManager::new();
        let mut ids = std::collections::HashSet::new();
        for _ in 0..50 {
            let c = manager.stage(track_edit());
            assert!(ids.insert(c.id.clone()), "duplicate id: {}", c.id);
        }
    }

    #[test]
    fn rejected_changes_cannot_be_re_accepted_or_exported() {
        let mut manager = ChangeManager::new();
        let c = manager.stage(track_edit());
        manager.reject(&c.id).unwrap();
        assert!(matches!(
            manager.accept(&c.id),
            Err(ChangeError::InvalidTransition { .. })
        ));
        assert!(matches!(
            manager.mark_exported(&c.id),
            Err(ChangeError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn rejects_all_proposed_changes() {
        let mut manager = ChangeManager::new();
        manager.stage(track_edit());
        manager.stage(NewChange {
            kind: ChangeKind::PlaylistRename,
            ..track_edit()
        });

        let rejected = manager.reject_all();
        assert_eq!(rejected.len(), 2);
        assert!(manager
            .list()
            .iter()
            .all(|change| change.status == ChangeStatus::Rejected));
    }
}
