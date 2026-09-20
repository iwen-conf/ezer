//! Integration tests that run the real turn loop against a scripted Chat Completions backend whose responses end with `finish_reason: "length"`.
//!
//! There is deliberately no test of the default agent with the env gate ON: env mutation is process-global and racy in the parallel test binary.
//! That case gains coverage when the RemoteSettings gate makes the budget injectable.
use super::support::*;
use super::*;
use std::sync::Arc;
use std::time::Duration;
use ezer_test_support::sse::chat_completion_script_exact;
use ezer_test_support::{MockInferenceServer, ScriptedResponse, SseEvent};
/// Distinctive fragment of the continue reminder.
const REMINDER_MARKER: &str = "exceeded the output token limit";
/// `SessionActor` turn futures overflow the default test thread stack.
fn block_on_session(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(f)
        .expect("spawn large-stack test thread")
        .join()
        .expect("test thread");
}
fn current_thread_local<F>(f: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    tokio::task::LocalSet::new().block_on(&rt, f);
}
/// SSE stream ending `finish_reason: "length"`.
/// Its reasoning delta pins that report aggregation spans a synthesized `Reasoning` sibling.
fn length_sse(text: &str) -> ScriptedResponse {
    ScriptedResponse::sse(vec![
        SseEvent::data(
            serde_json::json!({
                "id": "chatcmpl-len",
                "object": "chat.completion.chunk",
                "created": 1234567890,
                "model": "test",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "role": "assistant",
                        "reasoning_content": "thinking about the cut",
                        "content": text
                    },
                    "finish_reason": null
                }]
            })
            .to_string(),
        ),
        SseEvent::data(
            serde_json::json!({
                "id": "chatcmpl-len",
                "object": "chat.completion.chunk",
                "created": 1234567890,
                "model": "test",
                "choices": [{ "index": 0, "delta": {}, "finish_reason": "length" }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
            })
            .to_string(),
        ),
        SseEvent::data("[DONE]".to_string()),
    ])
}
fn stop_sse(text: &str) -> ScriptedResponse {
    ScriptedResponse::sse(chat_completion_script_exact(text, "test"))
}
/// Build an actor wired to the mock server on the Chat Completions backend.
async fn salvage_test_actor(server: &MockInferenceServer) -> Arc<SessionActor> {
    salvage_test_actor_with_budget(server, None).await
}
async fn salvage_test_actor_with_budget(
    server: &MockInferenceServer,
    configured_budget: Option<u32>,
) -> Arc<SessionActor> {
    salvage_test_actor_on_backend(
        server,
        0,
        256_000,
        ezer_sampling_types::ApiBackend::ChatCompletions,
        configured_budget,
    )
    .await
}
/// [`salvage_test_actor`] with a seeded token total and context window, for the tests where salvage interacts with compaction.
async fn salvage_test_actor_with_context(
    server: &MockInferenceServer,
    total_tokens: u64,
    context_window: u64,
) -> Arc<SessionActor> {
    salvage_test_actor_on_backend(
        server,
        total_tokens,
        context_window,
        ezer_sampling_types::ApiBackend::ChatCompletions,
        None,
    )
    .await
}
/// [`salvage_test_actor`] on an explicit backend.
/// The Messages test exists because only that stream layer delivers `Length` with completed tool calls.
/// Chat Completions rewrites the stop reason to `ToolCalls`.
async fn salvage_test_actor_on_backend(
    server: &MockInferenceServer,
    total_tokens: u64,
    context_window: u64,
    backend: ezer_sampling_types::ApiBackend,
    configured_budget: Option<u32>,
) -> Arc<SessionActor> {
    let sampling_cfg = ezer_sampler::SamplerConfig {
        api_key: Some("test-key".to_string()),
        base_url: server.url(),
        model: "test".to_string(),
        api_backend: backend.clone(),
        context_window,
        max_retries: Some(0),
        idle_timeout_secs: Some(30),
        ..Default::default()
    };
    let (sampler_event_tx, sampler_event_rx) =
        tokio::sync::mpsc::unbounded_channel::<ezer_sampler::SamplingEvent>();
    let sampler_handle = ezer_sampler::SamplerActor::spawn(
        sampling_cfg,
        ezer_sampler::RetryPolicy {
            max_retries: 0,
            ..Default::default()
        },
        sampler_event_tx,
    );
    let (gateway_tx, gateway_rx) =
        tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
    drain_gateway(gateway_rx);
    let (persistence_tx, persistence_rx) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
    drain_persistence(persistence_rx);
    let mut actor =
        create_test_actor(total_tokens, context_window, 85, gateway_tx, persistence_tx).await;
    actor.sampler_handle = sampler_handle;
    let mut cfg = actor
        .chat_state_handle
        .get_sampling_config()
        .await
        .expect("test actor has sampling config");
    cfg.base_url = server.url();
    cfg.api_backend = backend;
    cfg.model = "test".to_string();
    actor.chat_state_handle.update_sampling_config(cfg);
    let mut creds = actor.chat_state_handle.get_credentials().await;
    creds.api_key = Some("test-key".to_string());
    actor.chat_state_handle.update_credentials(creds);
    actor.length_salvage_remote_budget = configured_budget;
    actor
        .workspace_ops
        .bind_local_session(
            &actor.session_id_string(),
            actor.tool_context.cwd.as_path().to_path_buf(),
            actor.tool_context.hunk_tracker_handle.clone(),
            actor.agent.borrow().tool_bridge().toolset(),
            None,
        )
        .expect("bind_local_session");
    let actor = Arc::new(actor);
    {
        let drainer = actor.clone();
        let mut sampler_event_rx = sampler_event_rx;
        tokio::task::spawn_local(async move {
            while let Some(event) = sampler_event_rx.recv().await {
                drainer.handle_sampling_event(event).await;
            }
        });
    }
    actor
}
async fn run_prompt(actor: &Arc<SessionActor>, prompt_id: &str) -> PromptTurnResult {
    let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
        "write out the numbers".to_string(),
    ))];
    tokio::time::timeout(
        Duration::from_secs(60),
        actor.handle_prompt(
            prompt_id,
            prompt_blocks,
            PromptMode::Agent,
            None,
            None,
            None,
            None,
            true,
            false,
            None,
            None,
            None,
        ),
    )
    .await
    .expect("turn must finish within timeout")
}
/// The configured budget (user TOML or remote) reaches the resolver on a real actor.
#[test]
fn remote_budget_wires_into_the_resolver() {
    if ezer_config::env_bool("EZER_LENGTH_SALVAGE") == Some(true) {
        panic!("ambient EZER_LENGTH_SALVAGE=1 would mask the configured tier under test");
    }
    if ezer_config::env_bool("EZER_LENGTH_SALVAGE") == Some(false) {
        panic!("ambient EZER_LENGTH_SALVAGE=0 would mask the default-on path under test");
    }
    block_on_session(|| {
        current_thread_local(async {
            let (gateway_tx, gateway_rx) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            drain_gateway(gateway_rx);
            let (persistence_tx, persistence_rx) =
                tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            drain_persistence(persistence_rx);
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            assert_eq!(
                actor.length_salvage_budget(),
                Some(super::length_salvage::DEFAULT_LENGTH_CONTINUE_BUDGET),
                "on by default for BYOK/custom agents"
            );
            actor.length_salvage_remote_budget = Some(9);
            assert_eq!(
                actor.length_salvage_budget(),
                Some(9),
                "the configured tier feeds the resolver"
            );
            actor.length_salvage_remote_budget = Some(0);
            assert_eq!(
                actor.length_salvage_budget(),
                None,
                "the kill zero turns salvage off outright"
            );
        });
    });
}
/// `RemoteSettings` wire contract: legacy payloads without the key and
/// explicit null both mean absent; set values (including the kill zero)
/// round-trip.
#[test]
fn remote_settings_length_salvage_budget_serde_cells() {
    use crate::util::config::RemoteSettings;
    let legacy: RemoteSettings = serde_json::from_str("{}").expect("legacy payload");
    assert_eq!(legacy.length_salvage_budget, None);
    let null: RemoteSettings =
        serde_json::from_value(serde_json::json!({ "length_salvage_budget": null }))
            .expect("explicit null");
    assert_eq!(null.length_salvage_budget, None);
    let set: RemoteSettings =
        serde_json::from_value(serde_json::json!({ "length_salvage_budget": 0 }))
            .expect("kill value");
    assert_eq!(set.length_salvage_budget, Some(0));
}
/// Explicit kill (`length_salvage_budget = 0`): a Length response is the legacy hard failure.
#[test]
fn kill_switch_hard_fails_on_length() {
    if ezer_config::env_bool("EZER_LENGTH_SALVAGE") == Some(true) {
        panic!("ambient EZER_LENGTH_SALVAGE=1 would flip the kill switch under test");
    }
    block_on_session(|| {
        current_thread_local(async {
            let server = MockInferenceServer::start().await.expect("mock server");
            server.enqueue_response("/v1/chat/completions", length_sse("one, two, three,"));
            let actor = salvage_test_actor_with_budget(&server, Some(0)).await;
            let outcome = run_prompt(&actor, "length-kill").await;
            let err = outcome.expect_err("kill switch: Length must hard-fail the turn");
            assert!(
                crate::sampling::error::is_max_tokens_turn_error(&err),
                "the failure must be the max-tokens truncation error: {err:?}"
            );
            let conv = actor.chat_state_handle.get_conversation().await;
            let all_text: Vec<String> = conv.iter().map(|i| i.text_content()).collect();
            assert!(
                !all_text.iter().any(|t| t.contains(REMINDER_MARKER)),
                "no continue reminder with salvage off: {all_text:#?}"
            );
        });
    });
}
/// Terminal error whose metadata reports a tiny context window.
/// The overflow heuristic compares the token estimate to that window, so it fires for any nonempty history.
fn error_with_tiny_window(
    kind: ezer_sampler::SamplingErrorKind,
    status_code: u16,
) -> ezer_sampler::SamplingErrorInfo {
    ezer_sampler::SamplingErrorInfo {
        kind,
        status_code: Some(status_code),
        message: "terminal failure".to_string(),
        should_retry: None,
        error_code: None,
        is_retryable: false,
        retry_after_secs: None,
        model_metadata: Some(ezer_sampling_types::ResponseModelMetadata {
            context_window: Some(1),
            max_completion_tokens: None,
            models_etag: None,
        }),
        empty_response_context: None,
        doom_loop_triggers: None,
        doom_loop_aborted_at_chunk: None,
        credential: ezer_sampling_types::SentCredential::Unknown,
    }
}
/// A rate-limited terminal error mid-continuation keeps its terminal arm even when the estimate exceeds the reported window.
/// The quiet truncated-complete arm is only for overflows and empty caps, never for kinds naming a non-overflow cause.
#[test]
fn rate_limit_mid_continuation_stays_terminal() {
    block_on_session(|| {
        current_thread_local(async {
            let server = MockInferenceServer::start().await.expect("mock server");
            let actor = salvage_test_actor(&server).await;
            actor.chat_state_handle.push_user_message(
                ezer_sampling_types::ConversationItem::user(
                    "enough history that the token estimate clears the tiny window",
                ),
            );
            let error =
                error_with_tiny_window(ezer_sampler::SamplingErrorKind::RateLimited, 429);
            let Err(err) = actor
                .handle_sampling_failure(
                    error,
                    0,
                    transient_state(0, true),
                    true,
                    TurnParkState::Fresh,
                )
                .await
            else {
                panic!("a mid-salvage rate limit is still terminal");
            };
            assert_eq!(
                i32::from(err.code),
                crate::sampling::error::RATE_LIMITED_ERROR_CODE,
                "must take the rate-limited arm, not the quiet truncated-complete arm: {err:?}"
            );
        });
    });
}
/// A genuine context overflow mid-continuation completes the turn truncated even while auto-compaction is sticky-suppressed.
/// The overflow signal must not depend on the compaction gate.
#[test]
fn suppressed_overflow_mid_continuation_still_completes_truncated() {
    block_on_session(|| {
        current_thread_local(async {
            let server = MockInferenceServer::start().await.expect("mock server");
            let actor = salvage_test_actor(&server).await;
            actor.chat_state_handle.push_user_message(
                ezer_sampling_types::ConversationItem::user(
                    "enough history that the token estimate clears the tiny window",
                ),
            );
            actor.compaction.auto_compact_suppressed.store(
                crate::session::compaction_config::SUPPRESS_STICKY,
                std::sync::atomic::Ordering::Relaxed,
            );
            let error = error_with_tiny_window(ezer_sampler::SamplingErrorKind::Api, 500);
            let Err(err) = actor
                .handle_sampling_failure(
                    error,
                    0,
                    transient_state(0, true),
                    true,
                    TurnParkState::Fresh,
                )
                .await
            else {
                panic!("the quiet arm returns the typed error");
            };
            assert!(
                crate::sampling::error::is_max_tokens_turn_error(&err),
                "the turn loop needs the max-tokens marker to complete truncated: {err:?}"
            );
            let cause = err
                .data
                .as_ref()
                .and_then(|d| d.get(crate::sampling::error::SALVAGE_CAUSE_KEY))
                .and_then(|v| v.as_str());
            assert_eq!(
                cause,
                Some(crate::sampling::error::SALVAGE_CAUSE_OVERFLOW),
                "an over-window failure is the overflow population"
            );
        });
    });
}
/// Messages-backend SSE: text and a completed `tool_use` block, terminated `stop_reason: "max_tokens"`.
/// Only this backend delivers `Length` with tool calls to the turn loop.
/// Chat Completions rewrites the stop reason to `ToolCalls` at the stream layer.
fn messages_length_with_tool_call_sse(
    call_id: &str,
    name: &str,
    arguments: &str,
) -> ScriptedResponse {
    let events = vec![
        serde_json::json!({
            "type": "message_start",
            "message": {
                "id": "msg_len_tools", "type": "message", "role": "assistant",
                "content": [], "model": "test", "stop_reason": null,
                "usage": {
                    "input_tokens": 10, "output_tokens": 0,
                    "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0
                }
            }
        }),
        serde_json::json!({
            "type": "content_block_start", "index": 0,
            "content_block": {"type": "text", "text": ""}
        }),
        serde_json::json!({
            "type": "content_block_delta", "index": 0,
            "delta": {"type": "text_delta", "text": "recording the todo"}
        }),
        serde_json::json!({"type": "content_block_stop", "index": 0}),
        serde_json::json!({
            "type": "content_block_start", "index": 1,
            "content_block": {"type": "tool_use", "id": call_id, "name": name, "input": {}}
        }),
        serde_json::json!({
            "type": "content_block_delta", "index": 1,
            "delta": {"type": "input_json_delta", "partial_json": arguments}
        }),
        serde_json::json!({"type": "content_block_stop", "index": 1}),
        serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": "max_tokens"},
            "usage": {"output_tokens": 5, "input_tokens": 10}
        }),
        serde_json::json!({"type": "message_stop"}),
    ];
    ScriptedResponse::sse(
        events
            .into_iter()
            .map(|e| SseEvent::data(e.to_string()))
            .collect(),
    )
}
/// Messages SSE: plain text terminated `stop_reason: "end_turn"`.
fn messages_stop_sse(text: &str) -> ScriptedResponse {
    let events = vec![
        serde_json::json!({
            "type": "message_start",
            "message": {
                "id": "msg_stop", "type": "message", "role": "assistant",
                "content": [], "model": "test", "stop_reason": null,
                "usage": {
                    "input_tokens": 10, "output_tokens": 0,
                    "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0
                }
            }
        }),
        serde_json::json!({
            "type": "content_block_start", "index": 0,
            "content_block": {"type": "text", "text": ""}
        }),
        serde_json::json!({
            "type": "content_block_delta", "index": 0,
            "delta": {"type": "text_delta", "text": text}
        }),
        serde_json::json!({"type": "content_block_stop", "index": 0}),
        serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"output_tokens": 5, "input_tokens": 10}
        }),
        serde_json::json!({"type": "message_stop"}),
    ];
    ScriptedResponse::sse(
        events
            .into_iter()
            .map(|e| SseEvent::data(e.to_string()))
            .collect(),
    )
}

fn responses_length_sse(text: &str) -> ScriptedResponse {
    ScriptedResponse::sse(vec![
        SseEvent::data(
            serde_json::json!({
                "type": "response.created",
                "sequence_number": 0,
                "response": {
                    "id": "resp_len", "object": "response", "created_at": 1234567890,
                    "model": "test", "status": "in_progress", "output": []
                }
            })
            .to_string(),
        ),
        SseEvent::data(
            serde_json::json!({
                "type": "response.output_text.delta",
                "sequence_number": 1,
                "item_id": "item_test",
                "output_index": 0,
                "content_index": 0,
                "delta": text
            })
            .to_string(),
        ),
        SseEvent::data(
            serde_json::json!({
                "type": "response.incomplete",
                "sequence_number": 2,
                "response": {
                    "id": "resp_len", "object": "response", "created_at": 1234567890,
                    "model": "test", "status": "incomplete",
                    "incomplete_details": { "reason": "max_output_tokens" },
                    "output": [{
                        "type": "message", "id": "msg_test", "role": "assistant",
                        "status": "incomplete",
                        "content": [{ "type": "output_text", "text": text, "annotations": [] }]
                    }],
                    "usage": {
                        "input_tokens": 10, "output_tokens": 5, "total_tokens": 15,
                        "input_tokens_details": { "cached_tokens": 0 },
                        "output_tokens_details": { "reasoning_tokens": 0 }
                    }
                }
            })
            .to_string(),
        ),
        SseEvent::data("[DONE]".to_string()),
    ])
}

fn responses_stop_sse(text: &str) -> ScriptedResponse {
    ScriptedResponse::sse(ezer_test_support::sse::responses_api_script_exact(text, "test"))
}

type CapturedRetries = Arc<std::sync::Mutex<Vec<crate::extensions::notification::RetryState>>>;

fn drain_gateway_capturing_retries(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<xai_acp_lib::AcpClientMessage>,
) -> CapturedRetries {
    use crate::extensions::notification::{SessionNotification, SessionUpdate};
    let captured: CapturedRetries = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = captured.clone();
    tokio::task::spawn_local(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                xai_acp_lib::AcpClientMessage::SessionNotification(args) => {
                    let _ = args.response_tx.send(Ok(()));
                }
                xai_acp_lib::AcpClientMessage::ExtNotification(args)
                    if args.request.method.as_ref() == "ezer/session_notification" =>
                {
                    if let Ok(SessionNotification {
                        update: SessionUpdate::RetryState(rs),
                        ..
                    }) = serde_json::from_str::<SessionNotification>(args.request.params.get())
                    {
                        sink.lock().unwrap().push(rs);
                    }
                }
                _ => {}
            }
        }
    });
    captured
}

async fn salvage_test_actor_capturing(
    server: &MockInferenceServer,
    backend: ezer_sampling_types::ApiBackend,
    configured_budget: Option<u32>,
) -> (Arc<SessionActor>, CapturedRetries) {
    let (gateway_tx, gateway_rx) =
        tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
    let captured = drain_gateway_capturing_retries(gateway_rx);
    let (persistence_tx, persistence_rx) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
    drain_persistence(persistence_rx);
    let sampling_cfg = ezer_sampler::SamplerConfig {
        api_key: Some("test-key".to_string()),
        base_url: server.url(),
        model: "test".to_string(),
        api_backend: backend.clone(),
        context_window: 256_000,
        max_retries: Some(0),
        idle_timeout_secs: Some(30),
        ..Default::default()
    };
    let (sampler_event_tx, sampler_event_rx) =
        tokio::sync::mpsc::unbounded_channel::<ezer_sampler::SamplingEvent>();
    let sampler_handle = ezer_sampler::SamplerActor::spawn(
        sampling_cfg,
        ezer_sampler::RetryPolicy {
            max_retries: 0,
            ..Default::default()
        },
        sampler_event_tx,
    );
    let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
    actor.sampler_handle = sampler_handle;
    let mut cfg = actor
        .chat_state_handle
        .get_sampling_config()
        .await
        .expect("test actor has sampling config");
    cfg.base_url = server.url();
    cfg.api_backend = backend;
    cfg.model = "test".to_string();
    actor.chat_state_handle.update_sampling_config(cfg);
    let mut creds = actor.chat_state_handle.get_credentials().await;
    creds.api_key = Some("test-key".to_string());
    actor.chat_state_handle.update_credentials(creds);
    actor.length_salvage_remote_budget = configured_budget;
    actor
        .workspace_ops
        .bind_local_session(
            &actor.session_id_string(),
            actor.tool_context.cwd.as_path().to_path_buf(),
            actor.tool_context.hunk_tracker_handle.clone(),
            actor.agent.borrow().tool_bridge().toolset(),
            None,
        )
        .expect("bind_local_session");
    let actor = Arc::new(actor);
    {
        let drainer = actor.clone();
        let mut sampler_event_rx = sampler_event_rx;
        tokio::task::spawn_local(async move {
            while let Some(event) = sampler_event_rx.recv().await {
                drainer.handle_sampling_event(event).await;
            }
        });
    }
    (actor, captured)
}

#[test]
fn default_agent_auto_continues_truncated_stream() {
    if ezer_config::env_bool("EZER_LENGTH_SALVAGE") == Some(false) {
        panic!("ambient EZER_LENGTH_SALVAGE=0 would disable the default-on path");
    }
    block_on_session(|| {
        current_thread_local(async {
            let server = MockInferenceServer::start().await.expect("mock server");
            server.enqueue_response("/v1/chat/completions", length_sse("one, two, three,"));
            server.enqueue_response("/v1/chat/completions", stop_sse(" four, five."));
            let (actor, retries) = salvage_test_actor_capturing(
                &server,
                ezer_sampling_types::ApiBackend::ChatCompletions,
                None,
            )
            .await;
            let outcome = run_prompt(&actor, "length-auto-continue").await;
            let ok = outcome.expect("truncated stream must auto-continue and finish");
            assert_eq!(ok.stop_reason, acp::StopReason::EndTurn);
            let conv = actor.chat_state_handle.get_conversation().await;
            let all_text: Vec<String> = conv.iter().map(|i| i.text_content()).collect();
            assert!(
                all_text.iter().any(|t| t.contains(REMINDER_MARKER)),
                "continue reminder must be injected: {all_text:#?}"
            );
            let joined = all_text.join("\n");
            assert!(
                joined.contains("one, two, three,") && joined.contains("four, five."),
                "both the truncated prefix and the continuation must land: {all_text:#?}"
            );
            assert!(
                !joined.contains("Response truncated")
                    && !joined.contains("hit its output limit"),
                "successful continue must not write fatal truncation copy into the transcript: {all_text:#?}"
            );
            let retries = retries.lock().expect("retry lock");
            assert!(
                retries.iter().any(|rs| matches!(
                    rs,
                    crate::extensions::notification::RetryState::Retrying { reason, error_type: None, .. }
                        if reason.starts_with("continuing after output limit")
                )),
                "quiet continue status must be emitted: {retries:#?}"
            );
            assert!(
                !retries.iter().any(|rs| matches!(
                    rs,
                    crate::extensions::notification::RetryState::Failed { error_type, .. }
                        if error_type == "max_tokens_truncation"
                )),
                "successful continue must not emit RetryState::Failed max_tokens_truncation: {retries:#?}"
            );
        });
    });
}

#[test]
fn exhausted_continues_fail_the_turn() {
    if ezer_config::env_bool("EZER_LENGTH_SALVAGE") == Some(false) {
        panic!("ambient EZER_LENGTH_SALVAGE=0 would disable salvage before exhaustion");
    }
    block_on_session(|| {
        current_thread_local(async {
            let server = MockInferenceServer::start().await.expect("mock server");
            server.enqueue_response("/v1/chat/completions", length_sse("one,"));
            server.enqueue_response("/v1/chat/completions", length_sse(" two,"));
            let actor = salvage_test_actor_with_budget(&server, Some(1)).await;
            let outcome = run_prompt(&actor, "length-exhausted").await;
            let err = outcome.expect_err("budget of 1 continue then another Length must fail");
            assert!(
                crate::sampling::error::is_max_tokens_turn_error(&err),
                "exhausted continues must surface max_tokens_truncation: {err:?}"
            );
            let conv = actor.chat_state_handle.get_conversation().await;
            let all_text: Vec<String> = conv.iter().map(|i| i.text_content()).collect();
            assert!(
                all_text.iter().any(|t| t.contains(REMINDER_MARKER)),
                "the first continue still injects the reminder: {all_text:#?}"
            );
        });
    });
}

#[test]
fn responses_incomplete_max_output_tokens_auto_continues() {
    if ezer_config::env_bool("EZER_LENGTH_SALVAGE") == Some(false) {
        panic!("ambient EZER_LENGTH_SALVAGE=0 would disable the default-on path");
    }
    block_on_session(|| {
        current_thread_local(async {
            let server = MockInferenceServer::start().await.expect("mock server");
            server.enqueue_response("/v1/responses", responses_length_sse("cut at the cap,"));
            server.enqueue_response("/v1/responses", responses_stop_sse(" then finished."));
            let (actor, retries) = salvage_test_actor_capturing(
                &server,
                ezer_sampling_types::ApiBackend::Responses,
                None,
            )
            .await;
            let outcome = run_prompt(&actor, "responses-max-output-tokens").await;
            let ok = outcome.expect("Responses incomplete max_output_tokens must auto-continue");
            assert_eq!(ok.stop_reason, acp::StopReason::EndTurn);
            let conv = actor.chat_state_handle.get_conversation().await;
            let joined: String = conv
                .iter()
                .map(|i| i.text_content())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                joined.contains("cut at the cap,") && joined.contains("then finished."),
                "Responses continuation must keep both segments: {joined}"
            );
            assert!(
                !joined.contains("Response truncated"),
                "successful Responses continue must not emit fatal truncation copy: {joined}"
            );
            let retries = retries.lock().expect("retry lock");
            assert!(
                !retries.iter().any(|rs| matches!(
                    rs,
                    crate::extensions::notification::RetryState::Failed { error_type, .. }
                        if error_type == "max_tokens_truncation"
                )),
                "successful Responses continue must not fail the pager rail: {retries:#?}"
            );
        });
    });
}
