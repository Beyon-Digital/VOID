//! Tauri command handlers: WebView <-> void-app coordinator <-> engine worker.
//!
//! All persistent edits flow: DTO -> codec -> coordinator preflight
//! (validate/dedup/epoch/revision) -> control socket -> worker applies ->
//! CommandReceipt -> ledger commit -> DTO back to the caller.

use std::collections::HashMap;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

use serde_json::{json, Value};

use void_app::{Coordinator, ProjectHandle, ProjectState};
use void_protocol::proto;
use void_protocol::receipts::StoredReceipt;
use void_worker::{
    ControlReader, ControlWriter, Supervisor, SupervisorConfig, TelemetryChannel, WorkerHandle,
};

use crate::codec;

const RECEIPT_TIMEOUT_MS: u64 = 5_000;

/// Live engine binding. `handle` owns the child process (killed on drop-path
/// via shutdown()); `writer` is the serialized mutation lane to the worker.
struct EngineSlot {
    handle: Mutex<WorkerHandle>,
    writer: Mutex<ControlWriter>,
    engine_epoch: u64,
    worker_id: String,
}

pub struct AppState {
    pub coordinator: Mutex<Coordinator>,
    engine: Mutex<Option<Arc<EngineSlot>>>,
    pending: Mutex<HashMap<String, oneshot::Sender<Value>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            coordinator: Mutex::new(Coordinator::new()),
            engine: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn engine_status(state: State<'_, Arc<AppState>>) -> Result<Value, String> {
    let guard = state.engine.lock().await;
    Ok(match guard.as_ref() {
        Some(e) => json!({
            "attached": true,
            "worker_id": e.worker_id,
            "engine_epoch": e.engine_epoch.to_string(),
            "state": format!("{:?}", e.handle.lock().await.state()),
        }),
        None => json!({ "attached": false }),
    })
}

/// Spawn the engine worker (or any worker implementing the VOID handshake).
/// The supervisor enforces per-launch sockets, env-token auth, and the
/// protocol version before this returns ready.
#[tauri::command]
pub async fn spawn_engine(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    executable: String,
) -> Result<Value, String> {
    // Stop any current engine first.
    if let Some(old) = state.engine.lock().await.take() {
        old.handle.lock().await.shutdown().await;
    }

    let sup = Supervisor::new(SupervisorConfig {
        executable: executable.clone().into(),
        args: vec![],
        env: vec![],
        launch_base: std::env::var("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("void"),
        handshake_timeout: std::time::Duration::from_millis(
            void_protocol::limits::HANDSHAKE_TIMEOUT_MS,
        ),
        restart: void_worker::RestartPolicy::default(),
    });
    let mut handle = sup
        .spawn()
        .await
        .map_err(|e| format!("engine spawn failed: {e}"))?;

    let control = handle
        .take_control()
        .ok_or_else(|| "engine handshake returned no control channel".to_string())?;
    let telemetry = handle
        .take_telemetry()
        .ok_or_else(|| "engine handshake returned no telemetry channel".to_string())?;
    let (writer, reader) = control.into_split();

    let slot = Arc::new(EngineSlot {
        worker_id: handle.worker_id.clone(),
        engine_epoch: handle.engine_epoch,
        handle: Mutex::new(handle),
        writer: Mutex::new(writer),
    });
    *state.engine.lock().await = Some(slot.clone());
    info!(worker = %slot.worker_id, epoch = slot.engine_epoch, "engine attached");

    spawn_dispatcher(app.clone(), state.inner().clone(), reader, slot.clone());
    spawn_telemetry_pump(app, state.inner().clone(), telemetry);
    Ok(json!({
        "attached": true,
        "worker_id": slot.worker_id,
        "engine_epoch": slot.engine_epoch.to_string(),
    }))
}

#[tauri::command]
pub async fn stop_engine(state: State<'_, Arc<AppState>>) -> Result<Value, String> {
    if let Some(old) = state.engine.lock().await.take() {
        fail_pending(&state, "WORKER_FAILED: engine stopped").await;
        old.handle.lock().await.shutdown().await;
    }
    Ok(json!({ "attached": false }))
}

async fn fail_pending(state: &AppState, msg: &str) {
    let mut p = state.pending.lock().await;
    for (_, tx) in p.drain() {
        let _ = tx.send(json!({
            "kind": "CommandReceipt",
            "status": "OUTCOME_UNKNOWN",
            "error": "WORKER_FAILED",
            "message": msg,
        }));
    }
}

async fn send_frame(slot: &EngineSlot, payload: &[u8]) -> Result<(), String> {
    slot.writer
        .lock()
        .await
        .send(payload)
        .await
        .map_err(|e| e.to_string())
}

async fn await_reply(state: &AppState, key: String) -> Result<Value, String> {
    let (tx, rx) = oneshot::channel();
    {
        let mut p = state.pending.lock().await;
        if p.len() >= void_protocol::limits::PENDING_MUTATIONS_MAX {
            return Err("BUSY: too many pending requests".into());
        }
        p.insert(key.clone(), tx);
    }
    match tokio::time::timeout(std::time::Duration::from_millis(RECEIPT_TIMEOUT_MS), rx).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(_)) => Err("dispatcher dropped".into()),
        Err(_) => {
            state.pending.lock().await.remove(&key);
            // Timeout != failure: the worker may still apply it. Surface as
            // OUTCOME_UNKNOWN rather than fabricating a success.
            Err("reply timeout (outcome unknown)".into())
        }
    }
}

#[tauri::command]
pub async fn send_command(state: State<'_, Arc<AppState>>, dto: Value) -> Result<Value, String> {
    let payload = codec::persistent_command_json(&dto).map_err(|e| e.to_string())?;
    let cmd_id = dto
        .get("command_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let project_id = dto
        .get("project_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let is_lifecycle = {
        let op = dto.get("op").and_then(Value::as_object);
        op.map(|o| {
            o.keys()
                .next()
                .map(|k| matches!(k.as_str(), "CreateProjectOp" | "OpenProjectOp"))
                .unwrap_or(false)
        })
        .unwrap_or(false)
    };

    // Preflight gate (validate + dedup + epoch + revision). Lifecycle ops on a
    // not-yet-registered project skip the project checks inside preflight.
    {
        let mut c = state.coordinator.lock().await;
        if is_lifecycle && c.registry.get(&project_id).is_none() {
            c.registry.register(ProjectHandle {
                project_id: project_id.clone(),
                container_dir: dto
                    .get("container_dir")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                state: ProjectState::Registered,
                pending: Default::default(),
            });
        }
        let env =
            flatbuffers::root::<proto::ControlEnvelope>(&payload).map_err(|e| e.to_string())?;
        let cmd = env
            .frame_as_request_frame()
            .and_then(|f| f.request_as_persistent_command())
            .ok_or_else(|| "internal: envelope did not carry a PersistentCommand".to_string())?;
        let hash = void_protocol::receipts::payload_hash("persistent", &payload);
        if let Some((code, message)) = c.preflight(cmd, &hash, is_lifecycle) {
            // NONE+empty message = duplicate of an applied command.
            if code == proto::ErrorCode::NONE {
                return Ok(json!({
                    "kind": "CommandReceipt",
                    "command_id": cmd_id,
                    "status": "DUPLICATE",
                    "error": "NONE",
                }));
            }
            return Ok(json!({
                "kind": "CommandReceipt",
                "command_id": cmd_id,
                "status": "REJECTED",
                "error": code.variant_name().unwrap_or("BAD_REQUEST"),
                "message": message,
            }));
        }
    }

    let slot = {
        let g = state.engine.lock().await;
        g.clone().ok_or_else(|| "engine not attached".to_string())?
    };
    send_frame(&slot, &payload).await?;
    let receipt = await_reply(&state, format!("cmd:{cmd_id}")).await?;

    // Commit ledger + attach-state transitions on APPLIED.
    if receipt.get("status").and_then(Value::as_str) == Some("APPLIED") {
        let mut c = state.coordinator.lock().await;
        c.on_receipt(
            &project_id,
            &StoredReceipt {
                command_id: cmd_id.clone(),
                payload_hash: void_protocol::receipts::payload_hash("persistent", &payload),
                status: proto::AckStatus::APPLIED,
                revision: receipt
                    .get("revision")
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                engine_epoch: slot.engine_epoch,
            },
        );
        if is_lifecycle {
            if let Some(p) = c.registry.get_mut(&project_id) {
                p.state = ProjectState::Attached {
                    engine_epoch: slot.engine_epoch,
                };
            }
        }
    } else if is_lifecycle {
        // Definitive non-APPLIED outcome for a lifecycle op: drop the
        // `Registered` placeholder so the id isn't stuck as "still
        // attaching" forever. (A reply timeout leaves it Registered — the
        // outcome is genuinely unknown, and a retry now passes preflight
        // anyway since lifecycle ops are allowed through `Registered`.)
        let mut c = state.coordinator.lock().await;
        if c.registry
            .get(&project_id)
            .map(|p| matches!(p.state, ProjectState::Registered))
            .unwrap_or(false)
        {
            c.registry.remove(&project_id);
        }
    }
    Ok(receipt)
}

#[tauri::command]
pub async fn send_transport(state: State<'_, Arc<AppState>>, dto: Value) -> Result<Value, String> {
    let rid = dto
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let payload = codec::transport_request_json(&dto).map_err(|e| e.to_string())?;
    let slot = {
        let g = state.engine.lock().await;
        g.clone().ok_or_else(|| "engine not attached".to_string())?
    };
    send_frame(&slot, &payload).await?;
    // PANIC is acknowledged by send — never block behind the read queue.
    if dto.get("op").and_then(Value::as_str) == Some("PANIC") {
        return Ok(json!({ "accepted": true }));
    }
    await_reply(&state, format!("tack:{rid}")).await
}

#[tauri::command]
pub async fn read_view(state: State<'_, Arc<AppState>>, dto: Value) -> Result<Value, String> {
    let rid = dto
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let payload = codec::read_request_json(&dto).map_err(|e| e.to_string())?;
    let slot = {
        let g = state.engine.lock().await;
        g.clone().ok_or_else(|| "engine not attached".to_string())?
    };
    send_frame(&slot, &payload).await?;
    await_reply(&state, format!("read:{rid}")).await
}

// -- background tasks --------------------------------------------------------

/// Control-socket dispatcher: routes worker events to pending waiters and
/// mirrors them on the `void://control` UI event bus.
fn spawn_dispatcher(
    app: AppHandle,
    state: Arc<AppState>,
    mut reader: ControlReader,
    slot: Arc<EngineSlot>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            match reader.recv().await {
                Ok(frame) => match codec::event_frame_json(&frame) {
                    Ok(Some(ev)) => dispatch_event(&state, &app, ev).await,
                    Ok(None) => {}
                    Err(e) => warn!("control event decode: {e}"),
                },
                Err(_) => {
                    warn!("control channel closed for {}", slot.worker_id);
                    let mut g = state.engine.lock().await;
                    if let Some(cur) = g.as_ref() {
                        if Arc::ptr_eq(cur, &slot) {
                            *g = None;
                        }
                    }
                    drop(g);
                    fail_pending(&state, "WORKER_FAILED: control channel closed").await;
                    let _ = app.emit("void://engine-lost", json!({"worker_id": slot.worker_id}));
                    break;
                }
            }
        }
    });
}

/// Telemetry pump: lossy by contract — decode and emit to the WebView; the
/// worker is responsible for the <=30Hz bound, we forward what arrives.
/// TransportAcks also complete pending transport waiters.
fn spawn_telemetry_pump(app: AppHandle, state: Arc<AppState>, mut telemetry: TelemetryChannel) {
    tauri::async_runtime::spawn(async move {
        while let Ok(frame) = telemetry.recv().await {
            match codec::telemetry_json(&frame) {
                Ok(Some(v)) => {
                    if v.get("kind").and_then(Value::as_str) == Some("TransportAck") {
                        let key = format!(
                            "tack:{}",
                            v.get("request_id").and_then(Value::as_str).unwrap_or("")
                        );
                        if let Some(tx) = state.pending.lock().await.remove(&key) {
                            let _ = tx.send(v.clone());
                        }
                    }
                    let _ = app.emit("void://telemetry", v);
                }
                Ok(None) => {}
                Err(e) => warn!("telemetry decode: {e}"),
            }
        }
    });
}

async fn dispatch_event(state: &AppState, app: &AppHandle, ev: Value) {
    let kind = ev.get("kind").and_then(Value::as_str).unwrap_or("");
    let key = match kind {
        "CommandReceipt" => Some(format!(
            "cmd:{}",
            ev.get("command_id").and_then(Value::as_str).unwrap_or("")
        )),
        "ReadResponse" => Some(format!(
            "read:{}",
            ev.get("request_id").and_then(Value::as_str).unwrap_or("")
        )),
        "TransportAck" => Some(format!(
            "tack:{}",
            ev.get("request_id").and_then(Value::as_str).unwrap_or("")
        )),
        _ => None,
    };
    if let Some(k) = key {
        if let Some(tx) = state.pending.lock().await.remove(&k) {
            let _ = tx.send(ev.clone());
        }
    }
    let _ = app.emit("void://control", ev);
}
