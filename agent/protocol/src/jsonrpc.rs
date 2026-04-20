//! JSON-RPC 2.0 envelope types
//!
//! Transport-agnostic message wrappers. All fields comply with the
//! [JSON-RPC 2.0 specification](https://www.jsonrpc.org/specification).

use serde::{Deserialize, Serialize};

/// Top-level JSON-RPC message. Can represent any of the four message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
    Error(JsonRpcErrorResponse),
}

/// A request that expects a response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<serde_json::Map<String, serde_json::Value>>,
}

/// A one-way notification that does not expect a response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<serde_json::Map<String, serde_json::Value>>,
}

/// A successful response to a request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<serde_json::Map<String, serde_json::Value>>,
}

/// An error response to a request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    pub error: JsonRpcError,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Error object within an error response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Request identifier. Always a string (UUID-style), never an integer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
}

impl Default for RequestId {
    fn default() -> Self {
        RequestId::String(String::new())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trip() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            method: "session.initialize".to_string(),
            params: Some(serde_json::json!({"client_type": "tui"})),
            ..Default::default()
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, RequestId::String("req-1".to_string()));
        assert_eq!(parsed.method, "session.initialize");
    }

    #[test]
    fn notification_round_trip() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "session.heartbeat".to_string(),
            params: Some(serde_json::json!({})),
            ..Default::default()
        };
        let json = serde_json::to_string(&notif).unwrap();
        let parsed: JsonRpcNotification = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.method, "session.heartbeat");
        assert!(parsed.params.is_some());
    }

    #[test]
    fn response_round_trip() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            result: Some(serde_json::json!({"session_id": "sess-abc"})),
            ..Default::default()
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, RequestId::String("req-1".to_string()));
        assert!(parsed.result.is_some());
    }

    #[test]
    fn error_response_round_trip() {
        let err = JsonRpcErrorResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("req-1".to_string()),
            error: JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: Some(serde_json::json!({"method": "foo.bar"})),
                ..Default::default()
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&err).unwrap();
        let parsed: JsonRpcErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.error.code, -32601);
        assert_eq!(parsed.error.message, "Method not found");
    }

    #[test]
    fn parse_invalid_json() {
        let bad = "{not json}";
        let result: Result<JsonRpcMessage, _> = serde_json::from_str(bad);
        assert!(result.is_err());
    }

    #[test]
    fn request_id_is_string_not_int() {
        // Verify that an integer id does NOT parse into RequestId::String
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"test"}"#;
        let result: Result<JsonRpcRequest, _> = serde_json::from_str(json);
        // Since we use untagged enum, integer id won't match String variant
        assert!(result.is_err());
    }

    #[test]
    fn ext_roundtrip() {
        let mut ext = serde_json::Map::new();
        ext.insert("com.example.custom".to_string(), serde_json::json!({"foo": 42}));

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("ext-1".to_string()),
            method: "test".to_string(),
            params: None,
            ext: Some(ext.clone()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ext, Some(ext.clone()));

        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("ext-1".to_string()),
            result: None,
            ext: Some(ext.clone()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ext, Some(ext));
    }

    #[test]
    fn ext_omitted_when_none() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::String("1".to_string()),
            method: "m".to_string(),
            params: None,
            ext: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("ext").is_none());
    }
}
