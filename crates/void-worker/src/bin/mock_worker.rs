//! Reference mock worker: connects to the supervisor's sockets and sends
//! an authenticated WorkerHello, then echoes commands until killed.
//! Used by supervisor tests (T05/T06 evidence) and as the minimal example
//! of the worker-side protocol for the C++ engine.

use flatbuffers::FlatBufferBuilder;
use std::env;
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use void_protocol::frame::encode_frame;
use void_protocol::proto::{self, WorkerHelloArgs};

#[tokio::main]
async fn main() {
    let control_path = env::var(void_worker::ENV_CONTROL_SOCK).expect("VOID_CONTROL_SOCK");
    let telemetry_path = env::var(void_worker::ENV_TELEMETRY_SOCK).expect("VOID_TELEMETRY_SOCK");
    let token = env::var(void_worker::ENV_WORKER_TOKEN).expect("VOID_WORKER_TOKEN");

    let mut control = UnixStream::connect(&control_path).await.unwrap();
    let mut telemetry = UnixStream::connect(&telemetry_path).await.unwrap();

    // WorkerHello inside a ControlEnvelope
    let mut b = FlatBufferBuilder::new();
    let token_s = b.create_string(&token);
    let instance = b.create_string("mock-worker-1");
    let cap_strs: Vec<_> = ["mock", "test-fixture"]
        .iter()
        .map(|s| b.create_string(s))
        .collect();
    let caps = b.create_vector(&cap_strs);
    let hello = proto::WorkerHello::create(
        &mut b,
        &WorkerHelloArgs {
            worker_kind: proto::WorkerKind::ENGINE,
            worker_instance_id: Some(instance),
            protocol_major: void_protocol::PROTOCOL_MAJOR,
            protocol_minor: void_protocol::PROTOCOL_MINOR,
            launch_token: Some(token_s),
            engine_epoch: 1,
            capabilities: Some(caps),
        },
    );
    let envelope = proto::ControlEnvelope::create(
        &mut b,
        &proto::ControlEnvelopeArgs {
            frame_type: proto::ControlFrame::WorkerHello,
            frame: Some(hello.as_union_value()),
        },
    );
    b.finish_minimal(envelope);
    let framed = encode_frame(b.finished_data()).unwrap();
    use tokio::io::AsyncWriteExt;
    control.write_all(&framed).await.unwrap();
    control.flush().await.unwrap();

    // Serve: for every RequestFrame/PersistentCommand, reply
    // EventFrame/CommandReceipt APPLIED revision=expected+1 (echo contract).
    let mut reader = void_protocol::frame::FrameReader::default();
    let mut buf = [0u8; 16 * 1024];
    let mut revision: u64 = 0;
    loop {
        while let Some(payload) = reader.next_frame().unwrap() {
            let Ok(env) = flatbuffers::root::<proto::ControlEnvelope>(&payload) else {
                continue;
            };
            let Some(req) = env.frame_as_request_frame() else {
                continue;
            };
            let resp: Vec<u8> = if let Some(cmd) = req.request_as_persistent_command() {
                revision += 1;
                let mut b = FlatBufferBuilder::new();
                let cid = b.create_string(cmd.command_id().unwrap_or(""));
                let tid = b.create_string(cmd.transaction_id().unwrap_or(""));
                let msg = b.create_string("applied by mock worker");
                let hash = b.create_string("mock");
                let rc = proto::CommandReceipt::create(
                    &mut b,
                    &proto::CommandReceiptArgs {
                        command_id: Some(cid),
                        transaction_id: Some(tid),
                        status: proto::AckStatus::APPLIED,
                        error: proto::ErrorCode::NONE,
                        revision,
                        engine_epoch: 1,
                        message: Some(msg),
                        payload_hash: Some(hash),
                    },
                );
                let ef = proto::EventFrame::create(
                    &mut b,
                    &proto::EventFrameArgs {
                        event_type: proto::ControlEvent::CommandReceipt,
                        event: Some(rc.as_union_value()),
                    },
                );
                let env = proto::ControlEnvelope::create(
                    &mut b,
                    &proto::ControlEnvelopeArgs {
                        frame_type: proto::ControlFrame::EventFrame,
                        frame: Some(ef.as_union_value()),
                    },
                );
                b.finish_minimal(env);
                b.finished_data().to_vec()
            } else if let Some(t) = req.request_as_transport_request() {
                let mut b = FlatBufferBuilder::new();
                let rid = b.create_string(t.request_id().unwrap_or(""));
                let ack = proto::TransportAck::create(
                    &mut b,
                    &proto::TransportAckArgs {
                        request_id: Some(rid),
                        ok: true,
                        error: proto::ErrorCode::NONE,
                    },
                );
                let tf = proto::TelemetryFrame::create(
                    &mut b,
                    &proto::TelemetryFrameArgs {
                        event_type: proto::TelemetryEvent::TransportAck,
                        event: Some(ack.as_union_value()),
                    },
                );
                b.finish_minimal(tf);
                b.finished_data().to_vec()
            } else if let Some(rd) = req.request_as_read_request() {
                let mut b = FlatBufferBuilder::new();
                let rid = b.create_string(rd.request_id().unwrap_or(""));
                let cur = b.create_string("");
                let item_json = b.create_string("{\"kind\":\"mock\"}");
                let items = vec![proto::ReadItem::create(
                    &mut b,
                    &proto::ReadItemArgs {
                        object_id: Some(rid),
                        summary_json: Some(item_json),
                    },
                )];
                let items_v = b.create_vector(&items);
                let resp = proto::ReadResponse::create(
                    &mut b,
                    &proto::ReadResponseArgs {
                        request_id: Some(rid),
                        revision,
                        items: Some(items_v),
                        next_cursor: Some(cur),
                        done: true,
                        error: proto::ErrorCode::NONE,
                    },
                );
                let ef = proto::EventFrame::create(
                    &mut b,
                    &proto::EventFrameArgs {
                        event_type: proto::ControlEvent::ReadResponse,
                        event: Some(resp.as_union_value()),
                    },
                );
                let env = proto::ControlEnvelope::create(
                    &mut b,
                    &proto::ControlEnvelopeArgs {
                        frame_type: proto::ControlFrame::EventFrame,
                        frame: Some(ef.as_union_value()),
                    },
                );
                b.finish_minimal(env);
                b.finished_data().to_vec()
            } else {
                continue;
            };
            let framed = encode_frame(&resp).unwrap();
            // TransportAck rides telemetry; everything else rides control.
            let sink = if resp.len() > 4 && flatbuffers::root::<proto::TelemetryFrame>(&resp).is_ok()
                && req.request_as_transport_request().is_some()
            {
                &mut telemetry
            } else {
                &mut control
            };
            if sink.write_all(&framed).await.is_err() {
                break;
            }
            let _ = sink.flush().await;
        }
        match control.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => reader.push(&buf[..n]),
        }
    }
}
