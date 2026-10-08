//! Reference mock worker: connects to the supervisor's sockets and sends
//! an authenticated WorkerHello, then echoes commands until killed.
//! Used by supervisor tests (T05/T06 evidence) and as the minimal example
//! of the worker-side protocol for the C++ engine.

use std::env;
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use void_protocol::frame::encode_frame;
use void_protocol::proto::{self, ControlFrame, WorkerHelloArgs};
use flatbuffers::FlatBufferBuilder;

#[tokio::main]
async fn main() {
    let control_path = env::var(void_worker::ENV_CONTROL_SOCK).expect("VOID_CONTROL_SOCK");
    let telemetry_path = env::var(void_worker::ENV_TELEMETRY_SOCK).expect("VOID_TELEMETRY_SOCK");
    let token = env::var(void_worker::ENV_WORKER_TOKEN).expect("VOID_WORKER_TOKEN");

    let mut control = UnixStream::connect(&control_path).await.unwrap();
    let _telemetry = UnixStream::connect(&telemetry_path).await.unwrap();

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

    // Stay alive; drop on kill. Read+discard requests (a real worker would answer).
    let mut buf = [0u8; 4096];
    loop {
        match control.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
}
