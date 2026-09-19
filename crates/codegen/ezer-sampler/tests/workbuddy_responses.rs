//! WorkBuddy2API-Hub Responses mock: request shape, auth headers, and
//! `function_call_arguments.delta` without `item_id` (call_id only).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::Request;
use axum::response::sse::{Event, Sse};
use axum::routing::{get, post};
use futures_util::stream;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

use ezer_sampler::{
    ApiBackend, AuthScheme, RequestId, RetryPolicy, SamplerActor, SamplerConfig,
};
use ezer_sampling_types::{
    ConversationItem, ConversationRequest, SyntheticReason, ToolSpec, UserItem,
};

struct MockServer {
    addr: std::net::SocketAddr,
    shutdown_tx: oneshot::Sender<()>,
}

impl MockServer {
    async fn spawn(app: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        Self { addr, shutdown_tx }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

/// WorkBuddy-shaped Responses SSE: args delta has `call_id` and no `item_id`.
fn workbuddy_tool_call_sse() -> Vec<Event> {
    let events = [
        json!({
            "type": "response.created",
            "sequence_number": 0,
            "response": {
                "id": "resp_wb",
                "object": "response",
                "created_at": 0,
                "model": "deepseek-v4.1-flash",
                "status": "in_progress",
                "output": []
            }
        }),
        json!({
            "type": "response.output_item.added",
            "sequence_number": 1,
            "output_index": 0,
            "item": {
                "type": "function_call",
                "id": "fc_wb",
                "call_id": "call_wb_1",
                "name": "read_file",
                "arguments": "",
                "status": "in_progress"
            }
        }),
        json!({
            "type": "response.function_call_arguments.delta",
            "sequence_number": 2,
            "call_id": "call_wb_1",
            "output_index": 0,
            "delta": "{\"path\":\"README.md\"}"
        }),
        json!({
            "type": "response.function_call_arguments.done",
            "sequence_number": 3,
            "call_id": "call_wb_1",
            "output_index": 0,
            "name": "read_file",
            "arguments": "{\"path\":\"README.md\"}"
        }),
        json!({
            "type": "response.output_item.done",
            "sequence_number": 4,
            "output_index": 0,
            "item": {
                "type": "function_call",
                "id": "fc_wb",
                "call_id": "call_wb_1",
                "name": "read_file",
                "arguments": "{\"path\":\"README.md\"}",
                "status": "completed"
            }
        }),
        json!({
            "type": "response.completed",
            "sequence_number": 5,
            "response": {
                "id": "resp_wb",
                "object": "response",
                "created_at": 0,
                "model": "deepseek-v4.1-flash",
                "status": "completed",
                "output": [{
                    "type": "function_call",
                    "id": "fc_wb",
                    "call_id": "call_wb_1",
                    "name": "read_file",
                    "arguments": "{\"path\":\"README.md\"}",
                    "status": "completed"
                }],
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 5,
                    "total_tokens": 15,
                    "input_tokens_details": { "cached_tokens": 0 },
                    "output_tokens_details": { "reasoning_tokens": 0 }
                }
            }
        }),
    ];
    events
        .into_iter()
        .map(|v| Event::default().data(v.to_string()))
        .collect()
}

#[derive(Default, Clone)]
struct Captured {
    method_path: String,
    authorization: Option<String>,
    x_api_key: Option<String>,
    api_key: Option<String>,
    body: Option<serde_json::Value>,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workbuddy_responses_turn_sends_headers_and_parses_call_id_tool_delta() {
    let captured = Arc::new(Mutex::new(Captured::default()));
    let captured_h = Arc::clone(&captured);
    let app = Router::new()
        .route(
            "/v1/models",
            get(|| async {
                axum::Json(json!({
                    "object": "list",
                    "data": [
                        { "id": "deepseek-v4.1-flash", "object": "model", "owned_by": "workbuddy" },
                        { "id": "hy4-preview-f", "object": "model", "owned_by": "workbuddy" },
                        { "id": "hy3", "object": "model", "owned_by": "workbuddy" }
                    ]
                }))
            }),
        )
        .route(
            "/v1/responses",
            post(move |req: Request| {
                let captured = Arc::clone(&captured_h);
                async move {
                    let headers = req.headers().clone();
                    let bytes = axum::body::to_bytes(req.into_body(), 1 << 20)
                        .await
                        .unwrap();
                    let mut cap = captured.lock().unwrap();
                    cap.method_path = "/v1/responses".into();
                    cap.authorization = headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    cap.x_api_key = headers
                        .get("x-api-key")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    cap.api_key = headers
                        .get("api-key")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    cap.body = serde_json::from_slice(&bytes).ok();
                    drop(cap);
                    let events = workbuddy_tool_call_sse();
                    Sse::new(stream::iter(
                        events.into_iter().map(Ok::<_, std::convert::Infallible>),
                    ))
                }
            }),
        );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let cfg = SamplerConfig {
        api_key: Some("SrdCiNW_testkey".into()),
        base_url: server.base_url(),
        model: "deepseek-v4.1-flash".into(),
        api_backend: ApiBackend::Responses,
        auth_scheme: AuthScheme::Bearer,
        max_completion_tokens: Some(1024),
        context_window: 128_000,
        max_retries: Some(0),
        idle_timeout_secs: Some(30),
        ..Default::default()
    };
    let handle = SamplerActor::spawn(cfg, RetryPolicy::default(), event_tx);
    let request = ConversationRequest {
        items: vec![ConversationItem::User(UserItem {
            content: vec![ezer_sampling_types::ContentPart::Text {
                text: std::sync::Arc::<str>::from("read the readme"),
            }],
            synthetic_reason: SyntheticReason::Human,
            ..Default::default()
        })],
        tools: vec![ToolSpec {
            name: "read_file".into(),
            description: Some("Read a file".into()),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }],
        ..Default::default()
    };
    let result = handle
        .submit_and_collect(RequestId::from("wb-1"), request)
        .await;
    server.shutdown();
    let (response, _metrics) = result.expect("WorkBuddy Responses turn must complete");
    let assistant = response.assistant().expect("assistant item");
    assert_eq!(assistant.tool_calls.len(), 1);
    assert_eq!(assistant.tool_calls[0].name, "read_file");
    assert!(assistant.tool_calls[0].arguments.contains("README.md"));

    let cap = captured.lock().unwrap();
    assert_eq!(cap.method_path, "/v1/responses");
    assert_eq!(
        cap.authorization.as_deref(),
        Some("Bearer SrdCiNW_testkey")
    );
    assert_eq!(cap.x_api_key.as_deref(), Some("SrdCiNW_testkey"));
    assert_eq!(cap.api_key.as_deref(), Some("SrdCiNW_testkey"));
    let body = cap.body.as_ref().expect("request body");
    assert_eq!(body.get("model").and_then(|v| v.as_str()), Some("deepseek-v4.1-flash"));
    assert!(body.get("input").is_some(), "Responses input items required");
    assert!(
        body.get("tools").and_then(|v| v.as_array()).is_some(),
        "function tools must be sent: {body}"
    );
}
