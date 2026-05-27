use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::telemetry::SseTelemetry;
use codex_client::StreamResponse;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use tracing::debug;

pub(super) fn spawn_chat_completions_stream(
    stream_response: StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) -> ResponseStream {
    let upstream_request_id = stream_response
        .headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);
    tokio::spawn(async move {
        process_chat_sse(stream_response.bytes, tx_event, idle_timeout, telemetry).await;
    });

    ResponseStream {
        rx_event,
        upstream_request_id,
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    id: Option<String>,
    model: Option<String>,
    choices: Vec<ChatChoiceChunk>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceChunk {
    delta: ChatChoiceDelta,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceDelta {
    #[serde(rename = "role")]
    _role: Option<String>,
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ChatToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ChatToolCallDelta {
    index: usize,
    id: Option<String>,
    function: Option<ChatToolCallFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct ChatToolCallFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    completion_tokens_details: Option<ChatCompletionTokenDetails>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionTokenDetails {
    reasoning_tokens: Option<i64>,
}

impl From<ChatUsage> for TokenUsage {
    fn from(usage: ChatUsage) -> Self {
        let input_tokens = usage.prompt_tokens.unwrap_or(0);
        let output_tokens = usage.completion_tokens.unwrap_or(0);
        let reasoning_output_tokens = usage
            .completion_tokens_details
            .and_then(|details| details.reasoning_tokens)
            .unwrap_or(0);
        TokenUsage {
            input_tokens,
            cached_input_tokens: 0,
            output_tokens,
            reasoning_output_tokens,
            total_tokens: usage.total_tokens.unwrap_or(input_tokens + output_tokens),
        }
    }
}

#[derive(Debug, Default)]
struct PendingChatToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Debug, Default)]
struct ChatStreamState {
    response_id: Option<String>,
    server_model: Option<String>,
    created_sent: bool,
    message_started: bool,
    message_text: String,
    reasoning_text: String,
    tool_calls: BTreeMap<usize, PendingChatToolCall>,
    usage: Option<TokenUsage>,
    finished: bool,
}

impl ChatStreamState {
    async fn handle_chunk(
        &mut self,
        chunk: ChatCompletionChunk,
        tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
    ) {
        if self.response_id.is_none() {
            self.response_id = chunk.id;
        }
        if self.server_model.is_none()
            && let Some(model) = chunk.model
        {
            self.server_model = Some(model.clone());
            let _ = tx_event.send(Ok(ResponseEvent::ServerModel(model))).await;
        }
        if !self.created_sent {
            self.created_sent = true;
            let _ = tx_event.send(Ok(ResponseEvent::Created)).await;
        }
        if let Some(usage) = chunk.usage {
            self.usage = Some(usage.into());
        }

        for choice in chunk.choices {
            let ChatChoiceChunk { delta } = choice;
            if let Some(reasoning) = delta.reasoning_content {
                self.reasoning_text.push_str(&reasoning);
            }
            if let Some(content) = delta.content
                && !content.is_empty()
            {
                self.ensure_message_started(tx_event).await;
                self.message_text.push_str(&content);
                let _ = tx_event
                    .send(Ok(ResponseEvent::OutputTextDelta(content)))
                    .await;
            }
            if let Some(tool_calls) = delta.tool_calls {
                self.extend_tool_calls(tool_calls);
            }
        }
    }

    async fn ensure_message_started(
        &mut self,
        tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
    ) {
        if self.message_started {
            return;
        }
        self.message_started = true;
        let item = ResponseItem::Message {
            id: Some(self.item_id("msg")),
            role: "assistant".to_string(),
            content: Vec::new(),
            phase: Some(MessagePhase::FinalAnswer),
        };
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputItemAdded(item)))
            .await;
    }

    fn extend_tool_calls(&mut self, tool_calls: Vec<ChatToolCallDelta>) {
        for tool_call in tool_calls {
            let pending = self.tool_calls.entry(tool_call.index).or_default();
            if let Some(id) = tool_call.id {
                pending.id = Some(id);
            }
            if let Some(function) = tool_call.function {
                if let Some(name) = function.name {
                    pending.name = Some(name);
                }
                if let Some(arguments) = function.arguments {
                    pending.arguments.push_str(&arguments);
                }
            }
        }
    }

    async fn finish(&mut self, tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>) {
        if self.finished {
            return;
        }
        self.finished = true;
        if self.message_started || !self.message_text.is_empty() {
            let item = ResponseItem::Message {
                id: Some(self.item_id("msg")),
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: self.message_text.clone(),
                }],
                phase: Some(MessagePhase::FinalAnswer),
            };
            let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
        }
        if !self.reasoning_text.is_empty() {
            let item = ResponseItem::Reasoning {
                id: self.item_id("rs"),
                summary: vec![ReasoningItemReasoningSummary::SummaryText {
                    text: self.reasoning_text.clone(),
                }],
                content: Some(vec![ReasoningItemContent::ReasoningText {
                    text: self.reasoning_text.clone(),
                }]),
                encrypted_content: None,
            };
            let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
        }
        let mut emitted_tool_call = false;
        for pending in self.tool_calls.values() {
            if let (Some(call_id), Some(name)) = (&pending.id, &pending.name) {
                emitted_tool_call = true;
                let item = ResponseItem::FunctionCall {
                    id: Some(self.item_id("fc")),
                    name: name.clone(),
                    namespace: None,
                    arguments: pending.arguments.clone(),
                    call_id: call_id.clone(),
                };
                let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
            }
        }
        let response_id = self
            .response_id
            .clone()
            .unwrap_or_else(|| "chatcmpl_unknown".to_string());
        let _ = tx_event
            .send(Ok(ResponseEvent::Completed {
                response_id,
                token_usage: self.usage.take(),
                end_turn: Some(!emitted_tool_call),
            }))
            .await;
    }

    fn item_id(&self, prefix: &str) -> String {
        let response_id = self.response_id.as_deref().unwrap_or("chatcmpl_unknown");
        format!("{prefix}_{response_id}")
    }
}

async fn process_chat_sse(
    stream: codex_client::ByteStream,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
) {
    let mut stream = stream.eventsource();
    let mut state = ChatStreamState::default();

    loop {
        let start = Instant::now();
        let response = timeout(idle_timeout, stream.next()).await;
        if let Some(t) = telemetry.as_ref() {
            t.on_sse_poll(&response, start.elapsed());
        }
        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(err))) => {
                debug!("chat completions SSE error: {err:#}");
                let _ = tx_event.send(Err(ApiError::Stream(err.to_string()))).await;
                return;
            }
            Ok(None) => {
                state.finish(&tx_event).await;
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream(
                        "chat completions SSE stream idle timeout".to_string(),
                    )))
                    .await;
                return;
            }
        };

        let data = sse.data.trim();
        if data == "[DONE]" {
            state.finish(&tx_event).await;
            return;
        }
        if data.is_empty() {
            continue;
        }
        match serde_json::from_str::<ChatCompletionChunk>(data) {
            Ok(chunk) => {
                state.handle_chunk(chunk, &tx_event).await;
            }
            Err(err) => {
                let message = format!("failed to parse chat completions SSE chunk: {err}");
                debug!("{message}; data={data}");
                let _ = tx_event.send(Err(ApiError::Stream(message))).await;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[tokio::test]
    async fn parses_streaming_text_and_usage() {
        let chunks = vec![
            Ok(Bytes::from(
                "data: {\"id\":\"chatcmpl-1\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"he\"},\"finish_reason\":null}]}\n\n",
            )),
            Ok(Bytes::from(
                "data: {\"id\":\"chatcmpl-1\",\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{\"content\":\"llo\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\n",
            )),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];
        let stream = Box::pin(futures::stream::iter(chunks));
        let (tx, mut rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(16);

        process_chat_sse(stream, tx, Duration::from_secs(10), None).await;
        let mut events = Vec::new();
        while let Some(event) = rx_event.recv().await {
            if let Ok(event) = event {
                events.push(event);
            }
        }

        assert!(matches!(events[0], ResponseEvent::ServerModel(_)));
        assert!(matches!(events[1], ResponseEvent::Created));
        assert!(
            events.iter().any(
                |event| matches!(event, ResponseEvent::OutputTextDelta(delta) if delta == "he")
            )
        );
        assert!(events.iter().any(|event| matches!(
            event,
            ResponseEvent::OutputItemDone(ResponseItem::Message { content, .. })
                if content == &vec![ContentItem::OutputText { text: "hello".to_string() }]
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ResponseEvent::Completed { response_id, token_usage: Some(usage), end_turn: Some(true) }
                if response_id == "chatcmpl-1" && usage.total_tokens == 5
        )));
    }
}
