use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchChangeKind {
    Add,
    Delete,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplyStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecCommandStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpInvocation {
    pub server: String,
    pub tool: String,
    pub arguments: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSearchAction {
    Search {
        query: Option<String>,
        queries: Option<Vec<String>>,
    },
    OpenPage {
        url: Option<String>,
    },
    FindInPage {
        url: Option<String>,
        pattern: Option<String>,
    },
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchChange {
    pub path: String,
    pub move_path: Option<String>,
    pub kind: PatchChangeKind,
    pub diff: String,
    pub added: usize,
    pub removed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_change_kind_serialization() {
        let kind = PatchChangeKind::Add;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"add\"");
    }

    #[test]
    fn patch_change_kind_roundtrip() {
        for kind in [PatchChangeKind::Add, PatchChangeKind::Delete, PatchChangeKind::Update] {
            let json = serde_json::to_string(&kind).unwrap();
            let parsed: PatchChangeKind = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn exec_command_status_serialization() {
        let status = ExecCommandStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");
    }

    #[test]
    fn exec_command_status_roundtrip() {
        for status in [ExecCommandStatus::InProgress, ExecCommandStatus::Completed, ExecCommandStatus::Failed, ExecCommandStatus::Declined] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: ExecCommandStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn mcp_invocation_serialization() {
        let invocation = McpInvocation {
            server: "test-server".to_string(),
            tool: "test-tool".to_string(),
            arguments: Some(serde_json::json!({"key": "value"})),
        };
        let json = serde_json::to_string(&invocation).unwrap();
        assert!(json.contains("\"server\":\"test-server\""));
    }

    #[test]
    fn mcp_invocation_roundtrip() {
        let invocation = McpInvocation {
            server: "test-server".to_string(),
            tool: "test-tool".to_string(),
            arguments: Some(serde_json::json!({"key": "value"})),
        };
        let json = serde_json::to_string(&invocation).unwrap();
        let parsed: McpInvocation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.server, invocation.server);
        assert_eq!(parsed.tool, invocation.tool);
        assert_eq!(parsed.arguments, invocation.arguments);
    }

    #[test]
    fn mcp_invocation_empty_arguments() {
        let invocation = McpInvocation {
            server: "test-server".to_string(),
            tool: "test-tool".to_string(),
            arguments: None,
        };
        let json = serde_json::to_string(&invocation).unwrap();
        let parsed: McpInvocation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.arguments, None);
    }

    #[test]
    fn web_search_action_serialization() {
        let action = WebSearchAction::Search {
            query: Some("test query".to_string()),
            queries: None,
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"type\":\"search\""));
    }

    #[test]
    fn web_search_action_roundtrip() {
        let actions = [
            WebSearchAction::Search {
                query: Some("test".to_string()),
                queries: None,
            },
            WebSearchAction::Search {
                query: None,
                queries: Some(vec!["q1".to_string(), "q2".to_string()]),
            },
            WebSearchAction::OpenPage {
                url: Some("https://example.com".to_string()),
            },
            WebSearchAction::FindInPage {
                url: Some("https://example.com".to_string()),
                pattern: Some("pattern".to_string()),
            },
            WebSearchAction::Other,
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let parsed: WebSearchAction = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, action);
        }
    }

    #[test]
    fn patch_change_roundtrip() {
        let change = PatchChange {
            path: "/src/main.rs".to_string(),
            move_path: Some("/src/lib.rs".to_string()),
            kind: PatchChangeKind::Update,
            diff: "--- old\n+++ new".to_string(),
            added: 10,
            removed: 5,
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: PatchChange = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, change.path);
        assert_eq!(parsed.move_path, change.move_path);
        assert_eq!(parsed.kind, change.kind);
        assert_eq!(parsed.added, change.added);
        assert_eq!(parsed.removed, change.removed);
    }

    #[test]
    fn patch_change_no_move_path() {
        let change = PatchChange {
            path: "/src/main.rs".to_string(),
            move_path: None,
            kind: PatchChangeKind::Add,
            diff: "new content".to_string(),
            added: 20,
            removed: 0,
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: PatchChange = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.move_path, None);
    }
}