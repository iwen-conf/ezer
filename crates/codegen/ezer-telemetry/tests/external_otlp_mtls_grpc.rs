mod otlp_collector;

use std::time::Duration;

use otlp_collector as col;
use ezer_test_support::{OtelRecorder, OtelSignal};

fn write_temp(contents: &str) -> (tempfile::NamedTempFile, String) {
    let file = tempfile::NamedTempFile::new().expect("temp file");
    std::fs::write(file.path(), contents).expect("write pem");
    let path = file.path().to_str().expect("utf-8 path").to_string();
    (file, path)
}

#[test]
fn external_stream_grpc_mtls_end_to_end() {
    col::init_test_tracing();

    let tls = col::generate_tls_material();
    let (_ca_file, ca_path) = write_temp(&tls.ca_cert_pem);
    let (_cert_file, cert_path) = write_temp(&tls.client_cert_pem);
    let (_key_file, key_path) = write_temp(&tls.client_key_pem);

    let recorder = OtelRecorder::new();
    let endpoint = col::start_grpc_mtls_collector(
        recorder.clone(),
        tls.server_cert_pem.clone(),
        tls.server_key_pem.clone(),
        tls.ca_cert_pem.clone(),
    );
    assert!(endpoint.starts_with("https://"), "{endpoint}");

    let mut cfg = ezer_telemetry::external::ExternalOtelConfig::resolve_with(
        |name| match name {
            "EZER_EXTERNAL_OTEL" => Some("1".into()),
            "OTEL_LOGS_EXPORTER" | "OTEL_METRICS_EXPORTER" => Some("otlp".into()),
            "OTEL_EXPORTER_OTLP_ENDPOINT" => Some(endpoint.clone()),
            "OTEL_EXPORTER_OTLP_PROTOCOL" => Some("grpc".into()),
            "OTEL_EXPORTER_OTLP_CERTIFICATE" => Some(ca_path.clone()),
            "OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE" => Some(cert_path.clone()),
            "OTEL_EXPORTER_OTLP_CLIENT_KEY" => Some(key_path.clone()),
            "OTEL_METRIC_EXPORT_INTERVAL" => Some("200".into()),
            "OTEL_BLRP_SCHEDULE_DELAY" => Some("100".into()),
            _ => None,
        },
        None,
    )
    .expect("double opt-in must resolve");
    assert_eq!(
        cfg.logs_client_certificate.as_deref(),
        Some(cert_path.as_str())
    );
    assert_eq!(cfg.logs_client_key.as_deref(), Some(key_path.as_str()));
    cfg.client = ezer_telemetry::external::config::ExternalClientInfo {
        service_version: "0.0.0-test".into(),
        client_version: "0.0.0-test".into(),
        app_entrypoint: "cli".into(),
    };

    ezer_telemetry::external::init(Some(cfg));
    assert!(
        ezer_telemetry::external::is_active(),
        "mTLS gRPC exporters must build and activate the stream"
    );

    ezer_telemetry::log_event(ezer_telemetry::events::SessionNew {
        session_id: "sess-grpc-mtls-1".into(),
        client_identifier: None,
        client_version: None,
        is_git_repo: true,
        permission_mode: ezer_telemetry::enums::PermissionMode::Ask,
    });
    ezer_telemetry::log_event(ezer_telemetry::events::SessionHarness {
        session_id: "sess-grpc-mtls-1".into(),
        client_identifier: Some("ezer".into()),
        model_id: "grok-4".into(),
        agent_name: "ezer-build-plan".into(),
        permission_mode: ezer_telemetry::enums::PermissionMode::Ask,
        mcp_server_names: vec![],
        plugin_names: vec![],
        skill_names: vec![],
        lsp_server_names: vec![],
        hook_names: vec![],
        agents_md_dir_names: vec![],
        memory_enabled: false,
        memory_retrieval_mode: ezer_telemetry::events::MemoryRetrievalMode::Disabled,
        is_git_repo: true,
        auto_update: None,
    });

    ezer_telemetry::external::flush();
    col::block_on(recorder.wait_for_signals(Duration::from_secs(10), &[OtelSignal::Logs]))
        .expect("log records must arrive over mTLS");
    let names = recorder.event_names();
    assert!(
        names.iter().any(|n| n == "ezer.session_start"),
        "expected ezer.session_start in {names:?}"
    );

    col::block_on(recorder.wait_for_signals(Duration::from_secs(10), &[OtelSignal::Metrics]))
        .expect("metric exports must arrive over mTLS");

    ezer_telemetry::external::shutdown();
}
