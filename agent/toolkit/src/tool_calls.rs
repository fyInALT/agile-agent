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
    fn exec_command_status_serialization() {
        let status = ExecCommandStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");
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
    fn web_search_action_serialization() {
        let action = WebSearchAction::Search {
            query: Some("test query".to_string()),
            queries: None,
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"type\":\"search\""));
    }
}