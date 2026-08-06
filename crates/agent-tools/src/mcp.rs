use crate::{AgentToolService, ToolRequest};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value};
use std::io::{self, BufRead, Write};

const JSONRPC_VERSION: &str = "2.0";
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

pub fn run_stdio(service: AgentToolService) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line.context("reading MCP stdin")?;
        if line.trim().is_empty() {
            continue;
        }

        match handle_jsonrpc_message(&service, &line) {
            Ok(Some(response)) => {
                serde_json::to_writer(&mut stdout, &response).context("writing MCP response")?;
                stdout.write_all(b"\n").context("writing MCP newline")?;
                stdout.flush().context("flushing MCP response")?;
            }
            Ok(None) => {}
            Err(error) => eprintln!("MCP adapter error: {error:#}"),
        }
    }

    Ok(())
}

pub fn handle_jsonrpc_message(service: &AgentToolService, message: &str) -> Result<Option<Value>> {
    let request = match serde_json::from_str::<Value>(message) {
        Ok(request) => request,
        Err(error) => {
            return Ok(Some(error_response(
                Value::Null,
                -32700,
                format!("Parse error: {error}"),
            )));
        }
    };

    let id = request.get("id").cloned();
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Ok(Some(error_response(
            id.unwrap_or(Value::Null),
            -32600,
            "Invalid request: missing method",
        )));
    };

    if id.is_none() {
        return Ok(None);
    }

    let id = id.unwrap_or(Value::Null);
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(initialize_result()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => handle_tools_call(service, params),
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ => Err(JsonRpcError::new(
            -32601,
            format!("Unknown method: {method}"),
        )),
    };

    Ok(Some(match result {
        Ok(result) => success_response(id, result),
        Err(error) => error_response(id, error.code, error.message),
    }))
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        tool_definition(
            "library_search",
            "Search Rekordbox tracks by title, artist, album, genre, comments, or key.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    ("query", string_schema("Search query text.")),
                    (
                        "limit",
                        integer_schema("Maximum number of tracks to return."),
                    ),
                ],
                &["library_path", "query"],
            ),
        ),
        tool_definition(
            "library_get_track",
            "Get one Rekordbox track by ID.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    ("id", string_schema("Rekordbox content ID.")),
                ],
                &["library_path", "id"],
            ),
        ),
        tool_definition(
            "library_list_playlists",
            "List Rekordbox playlists.",
            object_schema(
                &[(
                    "library_path",
                    string_schema("Path to the Rekordbox master.db file."),
                )],
                &["library_path"],
            ),
        ),
        tool_definition(
            "library_get_playlist",
            "Get a playlist and its ordered tracks.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    ("id", string_schema("Rekordbox playlist ID.")),
                ],
                &["library_path", "id"],
            ),
        ),
        tool_definition(
            "library_list_cues",
            "List hot cues and memory cues for a track.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    ("track_id", string_schema("Rekordbox content ID.")),
                ],
                &["library_path", "track_id"],
            ),
        ),
        tool_definition(
            "health_orphan_scan",
            "Find library tracks whose file paths do not exist on disk.",
            object_schema(
                &[(
                    "library_path",
                    string_schema("Path to the Rekordbox master.db file."),
                )],
                &["library_path"],
            ),
        ),
        tool_definition(
            "health_duplicate_scan",
            "Find duplicate track groups in a Rekordbox library.",
            object_schema(
                &[(
                    "library_path",
                    string_schema("Path to the Rekordbox master.db file."),
                )],
                &["library_path"],
            ),
        ),
        tool_definition(
            "health_fuzzy_duplicate_scan",
            "Find likely-duplicate candidates by collapsing remix/feature/parenthetical markers from title and artist. Treat results as candidates needing manual review.",
            object_schema(
                &[(
                    "library_path",
                    string_schema("Path to the Rekordbox master.db file."),
                )],
                &["library_path"],
            ),
        ),
        tool_definition(
            "health_broken_link_scan",
            "Find metadata and link health issues in a Rekordbox library.",
            object_schema(
                &[(
                    "library_path",
                    string_schema("Path to the Rekordbox master.db file."),
                )],
                &["library_path"],
            ),
        ),
        tool_definition(
            "staging_list_changes",
            "List staged changes, optionally filtered by library path.",
            object_schema(
                &[(
                    "library_path",
                    string_schema("Optional Rekordbox library path filter."),
                )],
                &[],
            ),
        ),
        tool_definition(
            "library_read_file_tags",
            "Read the tags embedded in a track's audio file and compare to Rekordbox DB values.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    ("track_id", string_schema("Rekordbox content ID.")),
                ],
                &["library_path", "track_id"],
            ),
        ),
        tool_definition(
            "library_analyze_track",
            "Analyze BPM and key for a track from the audio file using stratum-dsp. Results are cached.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    ("track_id", string_schema("Rekordbox content ID.")),
                ],
                &["library_path", "track_id"],
            ),
        ),
        tool_definition(
            "library_scan_and_propose_missing",
            "Scan tracks with missing BPM or key, analyze them with stratum-dsp, and stage TrackMetadataEdit changes for each result.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    (
                        "fields",
                        json!({"type": "array", "items": {"type": "string"}, "description": "Fields to check: 'bpm', 'key', or both. Omit or pass empty array for both."}),
                    ),
                    (
                        "limit",
                        integer_schema("Maximum number of tracks to analyze (default 20)."),
                    ),
                ],
                &["library_path"],
            ),
        ),
        tool_definition(
            "undo_list",
            "List Sync runs recorded for a library, newest first, with how many changes of each \
             can be reversed and how many cannot.",
            object_schema(
                &[(
                    "library_path",
                    string_schema("Path to the Rekordbox master.db file."),
                )],
                &["library_path"],
            ),
        ),
        tool_definition(
            "undo_entries",
            "Show what one Sync run did, and the reason for each change that cannot be undone.",
            object_schema(
                &[("run_id", string_schema("Undo run ID, from undo_list."))],
                &["run_id"],
            ),
        ),
        tool_definition(
            "undo_run",
            "Stage the inverse of everything a Sync run applied. The inverses land as PROPOSED \
             changes and still go through review and Sync — this never writes to master.db. A run \
             can only be undone once.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    ("run_id", string_schema("Undo run ID, from undo_list.")),
                ],
                &["library_path", "run_id"],
            ),
        ),
        tool_definition(
            "smartlist_list",
            "List saved smartlists (rules-driven dynamic playlists) for a library.",
            object_schema(
                &[(
                    "library_path",
                    string_schema("Path to the Rekordbox master.db file."),
                )],
                &["library_path"],
            ),
        ),
        tool_definition(
            "smartlist_evaluate",
            "Evaluate a saved smartlist and return the tracks that currently match its rules.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    ("id", string_schema("Smartlist ID.")),
                ],
                &["library_path", "id"],
            ),
        ),
        tool_definition(
            "relocate_scan",
            "Find relocation candidates for broken/missing files by scanning root directories.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    (
                        "search_roots",
                        json!({"type": "array", "items": {"type": "string"}, "description": "List of absolute directory paths to scan for missing music files."}),
                    ),
                ],
                &["library_path", "search_roots"],
            ),
        ),
        tool_definition(
            "relocate_apply",
            "Stage a folder_path update for a broken file.",
            object_schema(
                &[
                    (
                        "library_path",
                        string_schema("Path to the Rekordbox master.db file."),
                    ),
                    ("track_id", string_schema("Rekordbox track ID.")),
                    ("new_path", string_schema("The new absolute path to the audio file.")),
                ],
                &["library_path", "track_id", "new_path"],
            ),
        ),
    ]
}

pub fn tool_request_from_name_and_arguments(name: &str, arguments: Value) -> Result<ToolRequest> {
    let arguments = arguments
        .as_object()
        .ok_or_else(|| anyhow!("arguments must be a JSON object"))?;

    match name {
        "library_search" | "library.search" => Ok(ToolRequest::LibrarySearch {
            library_path: required_string(arguments, "library_path")?,
            query: required_string(arguments, "query")?,
            limit: optional_usize(arguments, "limit")?,
        }),
        "library_bulk_add_intro_cues" | "library.bulk_add_intro_cues" => {
            Ok(ToolRequest::LibraryBulkAddIntroCues {
                library_path: required_string(arguments, "library_path")?,
                track_ids: arguments
                    .get("track_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        }
        "library_get_track" | "library.get_track" => Ok(ToolRequest::LibraryGetTrack {
            library_path: required_string(arguments, "library_path")?,
            id: required_string(arguments, "id")?,
        }),
        "library_list_playlists" | "library.list_playlists" => {
            Ok(ToolRequest::LibraryListPlaylists {
                library_path: required_string(arguments, "library_path")?,
            })
        }
        "library_get_playlist" | "library.get_playlist" => Ok(ToolRequest::LibraryGetPlaylist {
            library_path: required_string(arguments, "library_path")?,
            id: required_string(arguments, "id")?,
        }),
        "library_list_cues" | "library.list_cues" => Ok(ToolRequest::LibraryListCues {
            library_path: required_string(arguments, "library_path")?,
            track_id: required_string(arguments, "track_id")?,
        }),
        "health_orphan_scan" | "health.orphan_scan" => Ok(ToolRequest::HealthOrphanScan {
            library_path: required_string(arguments, "library_path")?,
        }),
        "health_duplicate_scan" | "health.duplicate_scan" => Ok(ToolRequest::HealthDuplicateScan {
            library_path: required_string(arguments, "library_path")?,
        }),
        "health_fuzzy_duplicate_scan" | "health.fuzzy_duplicate_scan" => {
            Ok(ToolRequest::HealthFuzzyDuplicateScan {
                library_path: required_string(arguments, "library_path")?,
            })
        }
        "health_broken_link_scan" | "health.broken_link_scan" => {
            Ok(ToolRequest::HealthBrokenLinkScan {
                library_path: required_string(arguments, "library_path")?,
            })
        }
        "staging_list_changes" | "staging.list_changes" => Ok(ToolRequest::StagingListChanges {
            library_path: optional_string(arguments, "library_path")?,
        }),
        "export_accepted_changes" | "export.accepted_changes" => {
            Ok(ToolRequest::ExportAcceptedChanges {
                library_path: required_string(arguments, "library_path")?,
                output_path: required_string(arguments, "output_path")?,
            })
        }
        "library_read_file_tags" | "library.read_file_tags" => {
            Ok(ToolRequest::LibraryReadFileTags {
                library_path: required_string(arguments, "library_path")?,
                track_id: required_string(arguments, "track_id")?,
            })
        }
        "library_analyze_track" | "library.analyze_track" => Ok(ToolRequest::LibraryAnalyzeTrack {
            library_path: required_string(arguments, "library_path")?,
            track_id: required_string(arguments, "track_id")?,
        }),
        "library_scan_and_propose_missing" | "library.scan_and_propose_missing" => {
            Ok(ToolRequest::LibraryScanAndProposeMissing {
                library_path: required_string(arguments, "library_path")?,
                fields: arguments
                    .get("fields")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                limit: optional_usize(arguments, "limit")?,
            })
        }
        "undo_list" | "undo.list" => Ok(ToolRequest::UndoList {
            library_path: required_string(arguments, "library_path")?,
        }),
        "undo_entries" | "undo.entries" => Ok(ToolRequest::UndoEntries {
            run_id: required_string(arguments, "run_id")?,
        }),
        "undo_run" | "undo.run" => Ok(ToolRequest::UndoRun {
            library_path: required_string(arguments, "library_path")?,
            run_id: required_string(arguments, "run_id")?,
        }),
        "smartlist_list" | "smartlists.list" => Ok(ToolRequest::SmartlistList {
            library_path: required_string(arguments, "library_path")?,
        }),
        "smartlist_evaluate" | "smartlists.evaluate" => Ok(ToolRequest::SmartlistEvaluate {
            library_path: required_string(arguments, "library_path")?,
            id: required_string(arguments, "id")?,
        }),
        "relocate_scan" | "relocate.scan" => Ok(ToolRequest::RelocateScan {
            library_path: required_string(arguments, "library_path")?,
            search_roots: arguments
                .get("search_roots")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        }),
        "relocate_apply" | "relocate.apply" => Ok(ToolRequest::RelocateApply {
            library_path: required_string(arguments, "library_path")?,
            track_id: required_string(arguments, "track_id")?,
            new_path: required_string(arguments, "new_path")?,
        }),
        _ => bail!("Unknown tool: {name}"),
    }
}

fn handle_tools_call(
    service: &AgentToolService,
    params: Value,
) -> std::result::Result<Value, JsonRpcError> {
    let params = params.as_object().ok_or_else(|| {
        JsonRpcError::new(-32602, "Invalid params: expected object for tools/call")
    })?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| JsonRpcError::new(-32602, "Invalid params: missing tool name"))?;
    if !is_advertised_tool_alias(name) {
        return Err(JsonRpcError::new(-32602, format!("Unknown tool: {name}")));
    }
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let request = tool_request_from_name_and_arguments(name, arguments)
        .map_err(|error| JsonRpcError::new(-32602, error.to_string()))?;
    let (text, is_error) = match service.execute(request) {
        Ok(value) => (
            serde_json::to_string_pretty(&value)
                .map_err(|error| JsonRpcError::new(-32603, error.to_string()))?,
            false,
        ),
        Err(error) => (error.to_string(), true),
    };

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "isError": is_error
    }))
}

fn is_advertised_tool_alias(name: &str) -> bool {
    matches!(
        name,
        "library_search"
            | "library.search"
            | "library_get_track"
            | "library.get_track"
            | "library_list_playlists"
            | "library.list_playlists"
            | "library_get_playlist"
            | "library.get_playlist"
            | "library_list_cues"
            | "library.list_cues"
            | "health_orphan_scan"
            | "health.orphan_scan"
            | "health_duplicate_scan"
            | "health.duplicate_scan"
            | "health_fuzzy_duplicate_scan"
            | "health.fuzzy_duplicate_scan"
            | "health_broken_link_scan"
            | "health.broken_link_scan"
            | "staging_list_changes"
            | "staging.list_changes"
            | "library_read_file_tags"
            | "library.read_file_tags"
            | "library_analyze_track"
            | "library.analyze_track"
            | "library_scan_and_propose_missing"
            | "library.scan_and_propose_missing"
            | "relocate_scan"
            | "relocate.scan"
            | "relocate_apply"
            | "relocate.apply"
    )
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "serverInfo": {
            "name": "rekordagent",
            "version": env!("CARGO_PKG_VERSION")
        },
        "capabilities": {
            "tools": {}
        }
    })
}

fn tool_definition(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn object_schema(properties: &[(&str, Value)], required: &[&str]) -> Value {
    let properties = properties
        .iter()
        .map(|(name, schema)| ((*name).to_owned(), schema.clone()))
        .collect::<Map<_, _>>();

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn string_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description
    })
}

fn integer_schema(description: &str) -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "description": description
    })
}

fn required_string(arguments: &Map<String, Value>, key: &str) -> Result<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("missing or invalid string argument `{key}`"))
}

fn optional_string(arguments: &Map<String, Value>, key: &str) -> Result<Option<String>> {
    arguments
        .get(key)
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("invalid string argument `{key}`"))
        })
        .transpose()
}

fn optional_usize(arguments: &Map<String, Value>, key: &str) -> Result<Option<usize>> {
    arguments
        .get(key)
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| anyhow!("invalid integer argument `{key}`"))
        })
        .transpose()
}

fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result
    })
}

fn error_response(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}

#[derive(Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
}

impl JsonRpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::mcp::handle_jsonrpc_message;
    use crate::AgentToolService;
    use rusqlite::Connection;
    use serde_json::Value;
    use std::path::Path;
    use tempfile::{NamedTempFile, TempPath};

    const RB_KEY: &str = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";
    const SCHEMA: &str = include_str!("../../rekordbox-db/src/sql/schema.sql");
    const SEED: &str = include_str!("../../rekordbox-db/src/sql/seed.sql");

    fn make_fixture_db() -> TempPath {
        let tmp = NamedTempFile::new().expect("tempfile");
        let path = tmp.into_temp_path();
        {
            let conn = writable_cipher_conn(&path);
            conn.execute_batch(SCHEMA).expect("schema");
            conn.execute_batch(SEED).expect("seed");
        }
        path
    }

    fn writable_cipher_conn(path: &Path) -> Connection {
        let conn = Connection::open(path).expect("open writable");
        conn.execute_batch(&format!(
            "PRAGMA key = '{RB_KEY}'; PRAGMA busy_timeout = 5000;"
        ))
        .expect("pragmas");
        conn
    }

    #[test]
    fn mcp_tools_list_includes_library_search() {
        let response = handle_jsonrpc_message(
            &AgentToolService::default(),
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        )
        .expect("response")
        .expect("not notification");

        let tools = response["result"]["tools"].as_array().expect("tools array");
        assert!(tools.iter().any(|tool| tool["name"] == "library_search"));
        assert!(!tools
            .iter()
            .any(|tool| tool["name"] == "export_accepted_changes"));
    }

    #[test]
    fn mcp_tools_call_library_search_accepts_dot_name_and_returns_content_json() {
        let library_path = make_fixture_db();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "library.search",
                "arguments": {
                    "library_path": library_path.display().to_string(),
                    "query": "Beta",
                    "limit": 1
                }
            }
        });

        let response = handle_jsonrpc_message(&AgentToolService::default(), &request.to_string())
            .expect("response")
            .expect("not notification");

        assert_eq!(response["result"]["isError"], false);
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text content");
        let value: Value = serde_json::from_str(text).expect("pretty JSON content");
        assert_eq!(value[0]["title"], "Test Track Beta");
    }

    #[test]
    fn mcp_tools_call_service_error_returns_mcp_tool_error_result() {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "staging_list_changes",
                "arguments": {}
            }
        });

        let response = handle_jsonrpc_message(&AgentToolService::default(), &request.to_string())
            .expect("response")
            .expect("not notification");

        assert!(response.get("error").is_none());
        assert_eq!(response["result"]["isError"], true);
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .expect("text")
            .contains("cache_path is required"));
    }

    #[test]
    fn mcp_unknown_tool_returns_jsonrpc_error() {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "library.nope",
                "arguments": {}
            }
        });

        let response = handle_jsonrpc_message(&AgentToolService::default(), &request.to_string())
            .expect("response")
            .expect("not notification");

        assert_eq!(response["error"]["code"], -32602);
        assert!(response["error"]["message"]
            .as_str()
            .expect("message")
            .contains("Unknown tool"));
    }

    #[test]
    fn undo_tools_are_advertised_and_parse() {
        let defs = crate::mcp::tool_definitions();
        let names: Vec<&str> = defs.iter().filter_map(|t| t["name"].as_str()).collect();
        for expected in ["undo_list", "undo_entries", "undo_run"] {
            assert!(names.contains(&expected), "{expected} not advertised");
        }

        assert_eq!(
            crate::mcp::tool_request_from_name_and_arguments(
                "undo_run",
                serde_json::json!({ "library_path": "/tmp/master.db", "run_id": "r1" }),
            )
            .expect("parsed"),
            crate::ToolRequest::UndoRun {
                library_path: "/tmp/master.db".into(),
                run_id: "r1".into(),
            }
        );
    }

    #[test]
    fn undo_run_requires_both_arguments() {
        // Staging inverses against the wrong library would put a pile of edits
        // on a database that never had them.
        assert!(crate::mcp::tool_request_from_name_and_arguments(
            "undo_run",
            serde_json::json!({ "run_id": "r1" }),
        )
        .is_err());
        assert!(crate::mcp::tool_request_from_name_and_arguments(
            "undo_run",
            serde_json::json!({ "library_path": "/tmp/master.db" }),
        )
        .is_err());
    }

    #[test]
    fn mcp_ping_returns_empty_result() {
        let response = handle_jsonrpc_message(
            &AgentToolService::default(),
            r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#,
        )
        .expect("response")
        .expect("not notification");

        assert_eq!(response["result"], serde_json::json!({}));
    }

    #[test]
    fn mcp_tools_call_rejects_unadvertised_export_aliases() {
        for name in ["export_accepted_changes", "export.accepted_changes"] {
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": {
                        "library_path": "/tmp/master.db",
                        "output_path": "/tmp/export.xml"
                    }
                }
            });

            let response =
                handle_jsonrpc_message(&AgentToolService::default(), &request.to_string())
                    .expect("response")
                    .expect("not notification");

            assert_eq!(response["error"]["code"], -32602);
            assert!(response["error"]["message"]
                .as_str()
                .expect("message")
                .contains("Unknown tool"));
        }
    }
}
