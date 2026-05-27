use crate::auth::SharedAuthProvider;
use crate::common::ResponseStream;
use crate::common::ResponsesApiRequest;
use crate::endpoint::ResponsesOptions;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::requests::Compression;
use crate::requests::headers::build_session_headers;
use crate::requests::headers::insert_header;
use crate::requests::headers::subagent_header;
use crate::telemetry::SseTelemetry;
use codex_client::HttpTransport;
use codex_client::RequestCompression;
use codex_client::RequestTelemetry;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use tracing::instrument;

mod stream;

use stream::spawn_chat_completions_stream;

pub struct ChatCompletionsClient<T: HttpTransport> {
    session: EndpointSession<T>,
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

impl<T: HttpTransport> ChatCompletionsClient<T> {
    pub fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            session: EndpointSession::new(transport, provider, auth),
            sse_telemetry: None,
        }
    }

    pub fn with_telemetry(
        self,
        request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        Self {
            session: self.session.with_request_telemetry(request),
            sse_telemetry: sse,
        }
    }

    #[instrument(
        name = "chat_completions.stream_request",
        level = "info",
        skip_all,
        fields(
            transport = "chat_completions_http",
            http.method = "POST",
            api.path = "chat/completions"
        )
    )]
    pub async fn stream_request(
        &self,
        request: ResponsesApiRequest,
        options: ResponsesOptions,
    ) -> Result<ResponseStream, ApiError> {
        let ResponsesOptions {
            session_id,
            thread_id,
            session_source,
            extra_headers,
            compression,
            turn_state: _,
        } = options;

        let body = ChatCompletionsRequest::from_responses_request(request);
        let body = serde_json::to_value(&body).map_err(|err| {
            ApiError::Stream(format!("failed to encode chat completions request: {err}"))
        })?;

        let mut headers = extra_headers;
        if let Some(ref thread_id) = thread_id {
            insert_header(&mut headers, "x-client-request-id", thread_id);
        }
        headers.extend(build_session_headers(session_id, thread_id));
        if let Some(subagent) = subagent_header(&session_source) {
            insert_header(&mut headers, "x-openai-subagent", &subagent);
        }

        self.stream(body, headers, compression).await
    }

    fn path() -> &'static str {
        "chat/completions"
    }

    #[instrument(
        name = "chat_completions.stream",
        level = "info",
        skip_all,
        fields(
            transport = "chat_completions_http",
            http.method = "POST",
            api.path = "chat/completions"
        )
    )]
    pub async fn stream(
        &self,
        body: Value,
        extra_headers: HeaderMap,
        compression: Compression,
    ) -> Result<ResponseStream, ApiError> {
        let request_compression = match compression {
            Compression::None => RequestCompression::None,
            Compression::Zstd => RequestCompression::Zstd,
        };

        let stream_response = self
            .session
            .stream_with(
                Method::POST,
                Self::path(),
                extra_headers,
                Some(body),
                |req| {
                    req.headers.insert(
                        http::header::ACCEPT,
                        HeaderValue::from_static("text/event-stream"),
                    );
                    req.compression = request_compression;
                },
            )
            .await?;

        Ok(spawn_chat_completions_stream(
            stream_response,
            self.session.provider().stream_idle_timeout,
            self.sse_telemetry.clone(),
        ))
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionsRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ChatThinking>,
}

impl ChatCompletionsRequest {
    fn from_responses_request(request: ResponsesApiRequest) -> Self {
        let mut messages = Vec::new();
        if !request.instructions.trim().is_empty() {
            messages.push(ChatMessage::text("system", request.instructions));
        }
        for item in request.input {
            messages.extend(chat_messages_from_response_item(item));
        }

        let reasoning_effort = request.reasoning.as_ref().and_then(|reasoning| {
            reasoning
                .effort
                .filter(|effort| !matches!(effort, ReasoningEffort::None))
        });

        Self {
            model: request.model,
            messages,
            stream: true,
            tools: chat_tools_from_responses_tools(&request.tools),
            tool_choice: chat_tool_choice(&request.tool_choice),
            parallel_tool_calls: request.parallel_tool_calls.then_some(true),
            reasoning_effort,
            thinking: reasoning_effort.map(|_| ChatThinking { r#type: "enabled" }),
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatThinking {
    r#type: &'static str,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<ChatContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

impl ChatMessage {
    fn text(role: &str, content: String) -> Self {
        Self {
            role: role.to_string(),
            content: Some(ChatContent::Text(content)),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn assistant_tool_call(call_id: String, name: String, arguments: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![ChatToolCall {
                id: call_id,
                r#type: "function".to_string(),
                function: ChatToolCallFunction { name, arguments },
            }]),
            tool_call_id: None,
        }
    }

    fn tool_output(call_id: String, output: String) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(ChatContent::Text(output)),
            tool_calls: None,
            tool_call_id: Some(call_id),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ChatContent {
    Text(String),
    Parts(Vec<ChatContentPart>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatContentPart {
    Text { text: String },
    ImageUrl { image_url: ChatImageUrl },
}

#[derive(Debug, Serialize)]
struct ChatImageUrl {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct ChatToolCall {
    id: String,
    r#type: String,
    function: ChatToolCallFunction,
}

#[derive(Debug, Serialize)]
struct ChatToolCallFunction {
    name: String,
    arguments: String,
}

fn chat_messages_from_response_item(item: ResponseItem) -> Vec<ChatMessage> {
    match item {
        ResponseItem::Message { role, content, .. } => vec![ChatMessage {
            role: chat_role(&role).to_string(),
            content: Some(chat_content_from_content_items(content)),
            tool_calls: None,
            tool_call_id: None,
        }],
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            ..
        }
        | ResponseItem::CustomToolCall {
            name,
            input: arguments,
            call_id,
            ..
        } => vec![ChatMessage::assistant_tool_call(call_id, name, arguments)],
        ResponseItem::FunctionCallOutput { call_id, output }
        | ResponseItem::CustomToolCallOutput {
            call_id, output, ..
        } => vec![ChatMessage::tool_output(
            call_id,
            function_output_text(&output),
        )],
        ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::Other => Vec::new(),
    }
}

fn chat_role(role: &str) -> &str {
    match role {
        "developer" => "system",
        "system" | "user" | "assistant" | "tool" => role,
        _ => "user",
    }
}

fn chat_content_from_content_items(content: Vec<ContentItem>) -> ChatContent {
    let has_image = content
        .iter()
        .any(|item| matches!(item, ContentItem::InputImage { .. }));
    if !has_image {
        let text = content
            .into_iter()
            .filter_map(|item| match item {
                ContentItem::InputText { text } | ContentItem::OutputText { text } => Some(text),
                ContentItem::InputImage { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        return ChatContent::Text(text);
    }

    ChatContent::Parts(
        content
            .into_iter()
            .map(|item| match item {
                ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                    ChatContentPart::Text { text }
                }
                ContentItem::InputImage { image_url, detail } => ChatContentPart::ImageUrl {
                    image_url: ChatImageUrl {
                        url: image_url,
                        detail: detail.map(chat_image_detail),
                    },
                },
            })
            .collect(),
    )
}

fn chat_image_detail(detail: ImageDetail) -> &'static str {
    match detail {
        ImageDetail::Auto => "auto",
        ImageDetail::Low => "low",
        ImageDetail::High => "high",
        ImageDetail::Original => "high",
    }
}

fn function_output_text(output: &FunctionCallOutputPayload) -> String {
    output.body.to_text().unwrap_or_default()
}

fn chat_tools_from_responses_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .filter_map(chat_tool_from_responses_tool)
        .collect()
}

fn chat_tool_from_responses_tool(tool: &Value) -> Option<Value> {
    match tool.get("type").and_then(Value::as_str) {
        Some("function") => chat_function_tool(tool),
        _ => None,
    }
}

fn chat_function_tool(tool: &Value) -> Option<Value> {
    let name = tool.get("name")?.clone();
    let mut function = serde_json::Map::new();
    function.insert("name".to_string(), name);
    if let Some(description) = tool.get("description") {
        function.insert("description".to_string(), description.clone());
    }
    if let Some(parameters) = tool.get("parameters") {
        function.insert("parameters".to_string(), parameters.clone());
    }
    if let Some(strict) = tool.get("strict") {
        function.insert("strict".to_string(), strict.clone());
    }
    Some(json!({
        "type": "function",
        "function": Value::Object(function),
    }))
}

fn chat_tool_choice(tool_choice: &str) -> Option<Value> {
    match tool_choice {
        "auto" | "none" | "required" => Some(Value::String(tool_choice.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(input: Vec<ResponseItem>, tools: Vec<Value>) -> ResponsesApiRequest {
        ResponsesApiRequest {
            model: "deepseek-v4-pro".to_string(),
            instructions: "You are helpful.".to_string(),
            input,
            tools,
            tool_choice: "auto".to_string(),
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: Vec::new(),
            service_tier: None,
            prompt_cache_key: None,
            text: None,
            client_metadata: None,
        }
    }

    #[test]
    fn maps_responses_request_to_chat_completion_body() {
        let req = request(
            vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "hello".to_string(),
                }],
                phase: None,
            }],
            vec![json!({
                "type": "function",
                "name": "shell",
                "description": "run a command",
                "parameters": {"type": "object"},
                "strict": false
            })],
        );

        let body = ChatCompletionsRequest::from_responses_request(req);
        let value = serde_json::to_value(body).unwrap();

        assert_eq!(value["model"], "deepseek-v4-pro");
        assert_eq!(value["messages"][0]["role"], "system");
        assert_eq!(value["messages"][1]["role"], "user");
        assert_eq!(value["messages"][1]["content"], "hello");
        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["function"]["name"], "shell");
    }
}
