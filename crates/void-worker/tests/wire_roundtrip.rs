//! Wire-contract round trip: spawn mock worker, send a PersistentCommand on
//! the real UDS control channel, decode the CommandReceipt the worker sends
//! back — the same path the C++ engine will speak.

use std::time::Duration;

use flatbuffers::FlatBufferBuilder;
use void_protocol::proto;
use void_worker::{Supervisor, SupervisorConfig};

fn mock_path() -> std::path::PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("void-mock-worker")
}

fn config() -> SupervisorConfig {
    SupervisorConfig {
        executable: mock_path(),
        args: vec![],
        env: vec![],
        launch_base: std::env::temp_dir().join("void-test"),
        handshake_timeout: Duration::from_secs(10),
        restart: void_worker::RestartPolicy {
            max_attempts: 1,
            backoff_base_ms: 50,
        },
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn command_receipt_roundtrip() {
    let mut handle = Supervisor::new(config()).spawn().await.unwrap();
    let control = handle.take_control().unwrap();
    let (mut writer, mut reader) = control.into_split();

    // PersistentCommand: CreateProjectOp
    let mut b = FlatBufferBuilder::new();
    let name = b.create_string("wire-test");
    let dir = b.create_string("/tmp/wire-test.void");
    let op = proto::CreateProjectOp::create(
        &mut b,
        &proto::CreateProjectOpArgs {
            name: Some(name),
            container_dir: Some(dir),
            sample_rate: 48000,
            initial_bpm: 120.0,
        },
    );
    let cid = b.create_string("cmd-1");
    let tid = b.create_string("tx-1");
    let pid = b.create_string("proj-1");
    let cmd = proto::PersistentCommand::create(
        &mut b,
        &proto::PersistentCommandArgs {
            command_id: Some(cid),
            transaction_id: Some(tid),
            project_id: Some(pid),
            engine_epoch: 1,
            expected_revision: 0,
            op_type: proto::PersistentOp::CreateProjectOp,
            op: Some(op.as_union_value()),
        },
    );
    let req = proto::RequestFrame::create(
        &mut b,
        &proto::RequestFrameArgs {
            request_type: proto::ControlRequest::PersistentCommand,
            request: Some(cmd.as_union_value()),
        },
    );
    let env = proto::ControlEnvelope::create(
        &mut b,
        &proto::ControlEnvelopeArgs {
            frame_type: proto::ControlFrame::RequestFrame,
            frame: Some(req.as_union_value()),
        },
    );
    b.finish_minimal(env);
    writer.send(b.finished_data()).await.unwrap();

    // Read EventFrame/CommandReceipt back.
    let resp = tokio::time::timeout(Duration::from_secs(5), reader.recv())
        .await
        .unwrap()
        .unwrap();
    let env = flatbuffers::root::<proto::ControlEnvelope>(&resp).unwrap();
    let ev = env.frame_as_event_frame().expect("expected EventFrame");
    let receipt = ev
        .event_as_command_receipt()
        .expect("expected CommandReceipt");
    assert_eq!(receipt.command_id(), Some("cmd-1"));
    assert_eq!(receipt.status(), proto::AckStatus::APPLIED);
    assert_eq!(receipt.revision(), 1);
    assert_eq!(receipt.engine_epoch(), 1);

    handle.shutdown().await;
}

/// rev-2: SubmitJobOp echoes an APPLIED receipt on control and the mock's
/// deterministic queued→running→succeeded JobEvent sequence on telemetry —
/// the same path the real runner's progress reporting will ride.
#[tokio::test(flavor = "multi_thread")]
async fn submit_job_op_roundtrip_with_job_events() {
    let mut handle = Supervisor::new(config()).spawn().await.unwrap();
    let control = handle.take_control().unwrap();
    let mut telemetry = handle.take_telemetry().unwrap();
    let (mut writer, mut reader) = control.into_split();

    let mut b = FlatBufferBuilder::new();
    let jid = b.create_string("job-1");
    let pid_s = b.create_string("proj-1");
    let ctx = b.create_string("context");
    let kind = b.create_string("av_export");
    let rid = b.create_string("void-export");
    let tok = b.create_string("export:job-1");
    let spec = proto::JobSpec::create(
        &mut b,
        &proto::JobSpecArgs {
            job_id: Some(jid),
            project_id: Some(pid_s),
            source_revision: 0,
            context_sha256: Some(ctx),
            kind: Some(kind),
            runtime_id: Some(rid),
            output_scope_token: Some(tok),
            ..Default::default()
        },
    );
    let op = proto::SubmitJobOp::create(&mut b, &proto::SubmitJobOpArgs { spec: Some(spec) });
    let cid = b.create_string("cmd-job-1");
    let tid = b.create_string("tx-job-1");
    let pid = b.create_string("proj-1");
    let cmd = proto::PersistentCommand::create(
        &mut b,
        &proto::PersistentCommandArgs {
            command_id: Some(cid),
            transaction_id: Some(tid),
            project_id: Some(pid),
            engine_epoch: 1,
            expected_revision: 0,
            op_type: proto::PersistentOp::SubmitJobOp,
            op: Some(op.as_union_value()),
        },
    );
    let req = proto::RequestFrame::create(
        &mut b,
        &proto::RequestFrameArgs {
            request_type: proto::ControlRequest::PersistentCommand,
            request: Some(cmd.as_union_value()),
        },
    );
    let env = proto::ControlEnvelope::create(
        &mut b,
        &proto::ControlEnvelopeArgs {
            frame_type: proto::ControlFrame::RequestFrame,
            frame: Some(req.as_union_value()),
        },
    );
    b.finish_minimal(env);
    writer.send(b.finished_data()).await.unwrap();

    // Receipt on control.
    let resp = tokio::time::timeout(Duration::from_secs(5), reader.recv())
        .await
        .unwrap()
        .unwrap();
    let env = flatbuffers::root::<proto::ControlEnvelope>(&resp).unwrap();
    let receipt = env
        .frame_as_event_frame()
        .and_then(|ev| ev.event_as_command_receipt())
        .expect("expected CommandReceipt");
    assert_eq!(receipt.command_id(), Some("cmd-job-1"));
    assert_eq!(receipt.status(), proto::AckStatus::APPLIED);
    assert_eq!(receipt.revision(), 1);

    // Deterministic job lifecycle on telemetry: queued → running → succeeded.
    for (status, percent) in [("queued", 0.0f32), ("running", 50.0), ("succeeded", 100.0)] {
        let frame = tokio::time::timeout(Duration::from_secs(5), telemetry.recv())
            .await
            .unwrap()
            .unwrap();
        let tf = flatbuffers::root::<proto::TelemetryFrame>(&frame).unwrap();
        let je = tf
            .event_as_job_event()
            .expect("expected JobEvent telemetry frame");
        assert_eq!(je.job_id(), Some("job-1"));
        assert_eq!(je.status(), Some(status));
        assert_eq!(je.percent(), percent);
    }

    handle.shutdown().await;
}
