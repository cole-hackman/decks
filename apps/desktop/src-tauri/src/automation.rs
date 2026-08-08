//! Automatic Actions (Epic 4) — background behaviours the user opts into once.
//!
//! Per `docs/lexicon/00-overview.md §Automatic Actions`. Five settings, of
//! which `decks` can currently honour two; the rest are reported as
//! **unavailable with the reason**, rather than offered as toggles that quietly
//! do nothing. A switch that does not switch anything is worse than a switch
//! that says why it is off.
//!
//! The three that remain unavailable are blocked on genuinely absent work, and
//! their reasons say which: structural drop detection, the Beatshift Fixer, and
//! the enrichment providers. `AUTO_WRITE_TAGS` was on that list citing field
//! mappings, which have existed since Epic 4 — a stale blocker outliving the
//! thing that caused it, which is its own kind of lie about what the app can
//! do.
//!
//! The recurring rule from the spec, which carries here: automation applies to
//! tracks the user brought in, never to tracks imported from a DJ app.

use serde::Serialize;

use crate::{read_config, write_config};

/// Stable keys, also used as the config field names.
pub const AUTO_ANALYZE: &str = "auto_analyze_new_tracks";
pub const AUTO_GENERATE_CUES: &str = "auto_generate_cues_on_play";
pub const AUTO_REENCODE: &str = "auto_reencode_new_files";
pub const AUTO_WRITE_TAGS: &str = "auto_write_tags";
pub const AUTO_FIND_TAGS: &str = "auto_find_custom_tags";

/// One row in the settings group.
#[derive(Debug, Serialize)]
pub struct AutomaticAction {
    pub key: String,
    pub label: String,
    pub description: String,
    pub enabled: bool,
    /// `None` when the action works. `Some(reason)` when `decks` cannot honour
    /// it yet — the UI disables the toggle and shows the reason verbatim.
    pub unavailable: Option<String>,
}

fn definitions() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
)> {
    vec![
        (
            AUTO_ANALYZE,
            "Auto-analyse new tracks",
            "Detect BPM and key when a file arrives in a watch folder. Never applied to tracks that came from Rekordbox.",
            None,
        ),
        (
            AUTO_GENERATE_CUES,
            "Auto-generate cues on play",
            "Apply the cue template to a played track that has no cues.",
            Some("Needs automatic drop detection. Today every anchor comes from a cue you placed by hand, so there is nothing to generate from on a track with no cues."),
        ),
        (
            AUTO_REENCODE,
            "Auto re-encode new MP3/M4A",
            "Run the Beatshift Fixer on arrival, before any cues exist.",
            Some("Needs the Beatshift Fixer, which is not built yet."),
        ),
        (
            AUTO_WRITE_TAGS,
            "Auto-write file tags",
            "Write detected BPM and key back into an arriving file's tags. Only ever applied to tracks you brought in, never to tracks that came from Rekordbox — and only when the analysis is confident.",
            None,
        ),
        (
            AUTO_FIND_TAGS,
            "Auto-find custom tags for new tracks",
            "Look up genre custom tags for new arrivals. Adds tags only, never touches the genre field.",
            Some("Needs the enrichment providers, which are not wired up yet."),
        ),
    ]
}

/// Read one flag, defaulting to off.
///
/// An unavailable action reads as off no matter what is stored, so a setting
/// enabled before its feature regressed cannot silently take effect.
pub fn is_enabled(app: &tauri::AppHandle, key: &str) -> bool {
    if definitions()
        .iter()
        .any(|(k, _, _, unavailable)| *k == key && unavailable.is_some())
    {
        return false;
    }
    read_config(app)
        .ok()
        .and_then(|c| c["automation"][key].as_bool())
        .unwrap_or(false)
}

#[tauri::command]
pub fn list_automatic_actions(app: tauri::AppHandle) -> Result<Vec<AutomaticAction>, String> {
    let config = read_config(&app)?;
    Ok(definitions()
        .into_iter()
        .map(|(key, label, description, unavailable)| AutomaticAction {
            key: key.to_string(),
            label: label.to_string(),
            description: description.to_string(),
            enabled: unavailable.is_none() && config["automation"][key].as_bool().unwrap_or(false),
            unavailable: unavailable.map(String::from),
        })
        .collect())
}

#[tauri::command]
pub fn set_automatic_action(
    app: tauri::AppHandle,
    key: String,
    enabled: bool,
) -> Result<(), String> {
    let Some((_, _, _, unavailable)) = definitions().into_iter().find(|(k, _, _, _)| *k == key)
    else {
        return Err(format!("unknown automatic action: {key}"));
    };
    if let Some(reason) = unavailable {
        return Err(reason.to_string());
    }

    let mut config = read_config(&app)?;
    if !config["automation"].is_object() {
        config["automation"] = serde_json::json!({});
    }
    config["automation"][key] = serde_json::json!(enabled);
    write_config(&app, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_in_the_spec_is_present() {
        let keys: Vec<_> = definitions().iter().map(|(k, _, _, _)| *k).collect();
        assert_eq!(
            keys,
            vec![
                AUTO_ANALYZE,
                AUTO_GENERATE_CUES,
                AUTO_REENCODE,
                AUTO_WRITE_TAGS,
                AUTO_FIND_TAGS
            ]
        );
    }

    #[test]
    fn only_the_actions_we_can_honour_are_available() {
        // Deliberate: the other three are surfaced with a reason rather than
        // offered as toggles that quietly do nothing.
        let available: Vec<_> = definitions()
            .into_iter()
            .filter(|(_, _, _, u)| u.is_none())
            .map(|(k, _, _, _)| k)
            .collect();
        assert_eq!(available, vec![AUTO_ANALYZE, AUTO_WRITE_TAGS]);
    }

    /// A blocker that outlives its cause is its own kind of lie about the app.
    #[test]
    fn no_action_is_blocked_on_field_mappings_any_more() {
        // Field mappings shipped in Epic 4, and the Rekordbox profile in a
        // later one. `AUTO_WRITE_TAGS` was still citing them as missing.
        for (key, _, _, unavailable) in definitions() {
            if let Some(reason) = unavailable {
                assert!(
                    !reason.contains("field mappings"),
                    "{key} still claims field mappings are missing"
                );
            }
        }
    }

    #[test]
    fn every_unavailable_action_explains_what_it_needs() {
        for (key, _, _, unavailable) in definitions() {
            if let Some(reason) = unavailable {
                assert!(
                    reason.contains("Needs"),
                    "{key} should say what it needs, got: {reason}"
                );
            }
        }
    }
}
