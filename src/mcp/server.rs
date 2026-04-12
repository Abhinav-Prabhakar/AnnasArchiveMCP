use std::collections::HashMap;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{
    self, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::mcp::tools::ToolRouter;

const JSON_RPC_VERSION: &str = "2.0";
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Clone)]
pub struct McpServer {
    server_info: ServerInfo,
    instructions: Option<String>,
    tool_router: Arc<ToolRouter>,
}

impl McpServer {
    pub fn new(
        server_name: impl Into<String>,
        server_version: impl Into<String>,
        instructions: Option<String>,
        tool_router: Arc<ToolRouter>,
    ) -> Self {
        Self {
            server_info: ServerInfo {
                name: server_name.into(),
                version: server_version.into(),
            },
            instructions,
            tool_router,
        }
    }

    pub async fn run_stdio(&self) -> AppResult<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        self.run_with_io(stdin, stdout).await
    }

    pub async fn run_with_io<R, W>(&self, reader: R, writer: W) -> AppResult<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut lines = BufReader::new(reader).lines();
        let mut writer = BufWriter::new(writer);
        let mut session = SessionState::default();

        info!(
            server_name = %self.server_info.name,
            server_version = %self.server_info.version,
            "starting MCP stdio server"
        );

        while let Some(line) = lines.next_line().await.map_err(AppError::from)? {
            let payload = line.trim();
            if payload.is_empty() {
                continue;
            }

            debug!(payload = %payload, "received stdio message");

            let request: JsonRpcRequest = match serde_json::from_str(payload) {
                Ok(request) => request,
                Err(error) => {
                    error!(%error, "failed to parse json-rpc request");
                    let response = JsonRpcResponse::error(
                        Some(Value::Null),
                        JsonRpcError::parse_error(format!("Invalid JSON payload: {error}")),
                    );
                    write_response(&mut writer, &response).await?;
                    continue;
                }
            };

            let response = self.handle_request(request, &mut session).await;
            if let Some(response) = response {
                write_response(&mut writer, &response).await?;
            }

            if session.exit_requested {
                break;
            }
        }

        writer.flush().await.map_err(AppError::from)?;
        info!("mcp stdio server stopped");
        Ok(())
    }

    async fn handle_request(
        &self,
        request: JsonRpcRequest,
        session: &mut SessionState,
    ) -> Option<JsonRpcResponse> {
        let request_id = request.id.clone();

        let result = self.handle_request_inner(request, session).await;

        match result {
            Ok(response) => response,
            Err(error) => {
                let rpc_error = error_to_json_rpc_error(&error);
                let is_notification = request_id.is_none();

                if is_notification {
                    warn!(error = %error, "request failed but caller did not provide an id");
                    None
                } else {
                    Some(JsonRpcResponse::error(request_id, rpc_error))
                }
            }
        }
    }

    async fn handle_request_inner(
        &self,
        request: JsonRpcRequest,
        session: &mut SessionState,
    ) -> AppResult<Option<JsonRpcResponse>> {
        ensure_json_rpc_version(request.jsonrpc.as_deref())?;
        let is_notification = request.id.is_none();

        match request.method.as_str() {
            "initialize" => {
                let params: InitializeParams = parse_params(request.params)?;
                session.client_info = params.client_info.clone();
                session.protocol_version = Some(params.protocol_version);
                session.initialize_seen = true;

                let result = InitializeResult {
                    protocol_version: MCP_PROTOCOL_VERSION.to_string(),
                    capabilities: ServerCapabilities {
                        tools: Some(ToolsCapability {
                            list_changed: false,
                        }),
                    },
                    server_info: self.server_info.clone(),
                    instructions: self.instructions.clone(),
                };

                if is_notification {
                    Ok(None)
                } else {
                    Ok(Some(JsonRpcResponse::success(
                        request.id,
                        serde_json::to_value(result).map_err(AppError::from)?,
                    )))
                }
            }
            "notifications/initialized" => {
                if !session.initialize_seen {
                    return Err(AppError::protocol(
                        "Client sent notifications/initialized before initialize".to_string(),
                    ));
                }

                session.initialized_notification_seen = true;
                info!(
                    client_name = session
                        .client_info
                        .as_ref()
                        .map(|v| v.name.as_str())
                        .unwrap_or("unknown"),
                    "client initialization completed"
                );
                Ok(None)
            }
            "ping" => {
                if is_notification {
                    Ok(None)
                } else {
                    Ok(Some(JsonRpcResponse::success(request.id, json!({}))))
                }
            }
            "tools/list" => {
                ensure_initialized(session)?;

                let result = ToolsListResult {
                    tools: self.tool_router.descriptors(),
                };

                if is_notification {
                    Ok(None)
                } else {
                    Ok(Some(JsonRpcResponse::success(
                        request.id,
                        serde_json::to_value(result).map_err(AppError::from)?,
                    )))
                }
            }
            "tools/call" => {
                ensure_initialized(session)?;

                let params: ToolCallParams = parse_params(request.params)?;
                let call_id = Uuid::new_v4().to_string();

                info!(tool = %params.name, call_id = %call_id, "executing tool");

                let result: ToolCallResult = self
                    .tool_router
                    .call(&params.name, params.arguments.unwrap_or_else(|| json!({})))
                    .await?;

                if is_notification {
                    Ok(None)
                } else {
                    Ok(Some(JsonRpcResponse::success(
                        request.id,
                        serde_json::to_value(result).map_err(AppError::from)?,
                    )))
                }
            }
            "shutdown" => {
                session.shutdown_requested = true;
                if is_notification {
                    Ok(None)
                } else {
                    Ok(Some(JsonRpcResponse::success(request.id, json!({}))))
                }
            }
            "exit" => {
                if !session.shutdown_requested {
                    warn!("received exit before shutdown");
                }
                session.exit_requested = true;
                Ok(None)
            }
            method => {
                if is_notification {
                    warn!(method = %method, "ignoring unknown notification");
                    Ok(None)
                } else {
                    Err(AppError::method_not_found(format!(
                        "Unknown method: {method}"
                    )))
                }
            }
        }
    }
}

async fn write_response<W>(writer: &mut BufWriter<W>, response: &JsonRpcResponse) -> AppResult<()>
where
    W: AsyncWrite + Unpin,
{
    let bytes = serde_json::to_vec(response).map_err(AppError::from)?;
    writer.write_all(&bytes).await.map_err(AppError::from)?;
    writer.write_all(b"\n").await.map_err(AppError::from)?;
    writer.flush().await.map_err(AppError::from)?;
    Ok(())
}

fn ensure_json_rpc_version(version: Option<&str>) -> AppResult<()> {
    match version {
        Some(JSON_RPC_VERSION) => Ok(()),
        Some(other) => Err(AppError::protocol(format!(
            "Unsupported JSON-RPC version: {other}"
        ))),
        None => Err(AppError::protocol(
            "Missing required jsonrpc version field".to_string(),
        )),
    }
}

fn ensure_initialized(session: &SessionState) -> AppResult<()> {
    if !session.initialize_seen {
        return Err(AppError::protocol(
            "Client must call initialize before using MCP methods".to_string(),
        ));
    }

    Ok(())
}

fn parse_params<T>(params: Option<Value>) -> AppResult<T>
where
    T: DeserializeOwned,
{
    let value = params.unwrap_or_else(|| json!({}));
    serde_json::from_value(value).map_err(|error| AppError::invalid_params(error.to_string()))
}

fn error_to_json_rpc_error(error: &AppError) -> JsonRpcError {
    let message = error.to_string();

    if message.to_ascii_lowercase().contains("method not found")
        || message.to_ascii_lowercase().contains("unknown method")
    {
        return JsonRpcError::method_not_found(message);
    }

    if message.to_ascii_lowercase().contains("invalid params") {
        return JsonRpcError::invalid_params(message);
    }

    if message.to_ascii_lowercase().contains("parse") {
        return JsonRpcError::parse_error(message);
    }

    if message.to_ascii_lowercase().contains("protocol")
        || message.to_ascii_lowercase().contains("initialize")
        || message.to_ascii_lowercase().contains("json-rpc")
    {
        return JsonRpcError::invalid_request(message);
    }

    JsonRpcError::tool_error(
        "Tool execution failed",
        Some(json!({
            "message": message,
        })),
    )
}

#[derive(Debug, Default, Clone)]
struct SessionState {
    protocol_version: Option<String>,
    client_info: Option<ClientInfo>,
    initialize_seen: bool,
    initialized_notification_seen: bool,
    shutdown_requested: bool,
    exit_requested: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: Option<String>,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self {
            code: -32700,
            message: message.into(),
            data: None,
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: message.into(),
            data: None,
        }
    }

    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: message.into(),
            data: None,
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    pub fn tool_error(message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            code: -32000,
            message: message.into(),
            data,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    #[serde(default)]
    pub capabilities: HashMap<String, Value>,
    #[serde(default)]
    pub client_info: Option<ClientInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsListResult {
    pub tools: Vec<ToolDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Value>,
}

pub fn string_schema(description: impl Into<String>, required: bool) -> (&'static str, Value) {
    (
        if required { "required" } else { "optional" },
        json!({
            "type": "string",
            "description": description.into(),
        }),
    )
}

pub fn integer_schema(description: impl Into<String>, required: bool) -> (&'static str, Value) {
    (
        if required { "required" } else { "optional" },
        json!({
            "type": "integer",
            "description": description.into(),
        }),
    )
}

pub fn boolean_schema(description: impl Into<String>, required: bool) -> (&'static str, Value) {
    (
        if required { "required" } else { "optional" },
        json!({
            "type": "boolean",
            "description": description.into(),
        }),
    )
}

pub fn enum_string_schema(
    description: impl Into<String>,
    values: &[&str],
    required: bool,
) -> (&'static str, Value) {
    (
        if required { "required" } else { "optional" },
        json!({
            "type": "string",
            "description": description.into(),
            "enum": values,
        }),
    )
}

pub fn array_schema(
    description: impl Into<String>,
    item_schema: Value,
    required: bool,
) -> (&'static str, Value) {
    (
        if required { "required" } else { "optional" },
        json!({
            "type": "array",
            "description": description.into(),
            "items": item_schema,
        }),
    )
}

pub fn json_schema_for_object(fields: Vec<(&str, (&'static str, Value))>) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for (name, (requiredness, schema)) in fields {
        if requiredness == "required" {
            required.push(name.to_string());
        }
        properties.insert(name.to_string(), schema);
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_rpc_success_response_serializes() {
        let response = JsonRpcResponse::success(Some(json!(1)), json!({ "ok": true }));
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], 1);
        assert_eq!(value["result"]["ok"], true);
        assert!(value.get("error").is_none());
    }

    #[test]
    fn json_rpc_error_response_serializes() {
        let response = JsonRpcResponse::error(
            Some(json!("abc")),
            JsonRpcError::invalid_request("bad request"),
        );
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], "abc");
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["error"]["message"], "bad request");
    }

    #[test]
    fn initialize_params_deserialize() {
        let value = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        });

        let params: InitializeParams = serde_json::from_value(value).unwrap();
        assert_eq!(params.protocol_version, "2024-11-05");
        assert_eq!(params.client_info.unwrap().name, "test-client");
    }
}
