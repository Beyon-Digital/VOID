#!/usr/bin/env python3
"""VOID engine-lane smoke test: a minimal supervisor stub.

Binds the control + telemetry UDS sockets, spawns `void-engine`, verifies the
WorkerHello handshake, then drives a PersistentCommand sequence and asserts
CommandReceipt outcomes. Prints one PASS/FAIL line per assertion; exits 0 only
when every check passed.

Usage: python3 supervisor_stub.py <path-to-void-engine-binary>
Requires: flatbuffers (pip), generated bindings under ./voidproto (flatc --python).
"""
import os, sys, socket, struct, subprocess, tempfile, time, hashlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import flatbuffers  # noqa: E402
import voidproto.ControlEnvelope as _ControlEnvelope
from voidproto.ControlEnvelope import ControlEnvelope  # noqa: E402
from voidproto.ControlFrame import ControlFrame  # noqa: E402
import voidproto.RequestFrame as _RequestFrame
from voidproto.RequestFrame import RequestFrame  # noqa: E402
from voidproto.EventFrame import EventFrame  # noqa: E402
from voidproto.ControlRequest import ControlRequest  # noqa: E402
from voidproto.ControlEvent import ControlEvent  # noqa: E402
import voidproto.PersistentCommand as _PersistentCommand
from voidproto.PersistentCommand import PersistentCommand  # noqa: E402
from voidproto.PersistentOp import PersistentOp  # noqa: E402
import voidproto.CreateProjectOp as _CreateProjectOp
from voidproto.CreateProjectOp import CreateProjectOp  # noqa: E402
import voidproto.AddTrackOp as _AddTrackOp
from voidproto.AddTrackOp import AddTrackOp  # noqa: E402
from voidproto.TrackKind import TrackKind  # noqa: E402
import voidproto.InsertMidiClipOp as _InsertMidiClipOp
from voidproto.InsertMidiClipOp import InsertMidiClipOp  # noqa: E402
import voidproto.InsertNoteOp as _InsertNoteOp
from voidproto.InsertNoteOp import InsertNoteOp  # noqa: E402
import voidproto.SetTempoOp as _SetTempoOp
from voidproto.SetTempoOp import SetTempoOp  # noqa: E402
import voidproto.SetTrackGainOp as _SetTrackGainOp
from voidproto.SetTrackGainOp import SetTrackGainOp  # noqa: E402
import voidproto.SetTrackMuteOp as _SetTrackMuteOp
from voidproto.SetTrackMuteOp import SetTrackMuteOp  # noqa: E402
from voidproto.SetLoopRangeOp import SetLoopRangeOp  # noqa: E402
import voidproto.SaveProjectOp as _SaveProjectOp
from voidproto.SaveProjectOp import SaveProjectOp  # noqa: E402
import voidproto.TransportRequest as _TransportRequest
from voidproto.TransportRequest import TransportRequest  # noqa: E402
from voidproto.TransportOp import TransportOp  # noqa: E402
from voidproto.CommandReceipt import CommandReceipt  # noqa: E402
from voidproto.WorkerHello import WorkerHello  # noqa: E402
from voidproto.WorkerKind import WorkerKind  # noqa: E402
from voidproto.TransportAck import TransportAck  # noqa: E402

BIN = sys.argv[1] if len(sys.argv) > 1 else \
    "build/void-engine/void-engine_artefacts/Debug/void-engine"
ROOT = os.path.dirname(os.path.abspath(__file__))
BASE = os.path.abspath(os.path.join(ROOT, "../../.."))

results = []
def check(name, ok, detail=""):
    results.append(ok)
    print(("PASS" if ok else "FAIL") + f" {name} {detail}")

def send_frame(sock, buf):
    sock.sendall(struct.pack("<I", len(buf)) + buf)

def recv_frame(sock, timeout=15):
    sock.settimeout(timeout)
    hdr = b""
    while len(hdr) < 4:
        chunk = sock.recv(4 - len(hdr))
        if not chunk:
            raise EOFError("socket closed")
        hdr += chunk
    n = struct.unpack("<I", hdr)[0]
    data = b""
    while len(data) < n:
        data += sock.recv(n - len(data))
    return data

# --- flatbuffer builders ------------------------------------------------------
def persistent(env_op_type, op_off, cid, expected_rev=-1, project_id="t-proj"):
    b = flatbuffers.Builder(0)
    if isinstance(op_off, int):
        op = op_off
    else:
        op = op_off
    b.Finish(b"")  # placeholder never used
    return b


def build_command(op_type, op_builder, cid, project_id="t-proj", expected_rev=0,
                  epoch=1):
    """op_builder: fn(builder)->offset for the op's table."""
    b = flatbuffers.Builder(0)
    cid_s = b.CreateString(cid)
    txn = b.CreateString("txn-" + cid)
    pid = b.CreateString(project_id)
    op_off = op_builder(b)
    _PersistentCommand.PersistentCommandStart(b)
    _PersistentCommand.PersistentCommandAddCommandId(b, cid_s)
    _PersistentCommand.PersistentCommandAddTransactionId(b, txn)
    _PersistentCommand.PersistentCommandAddProjectId(b, pid)
    _PersistentCommand.PersistentCommandAddEngineEpoch(b, epoch)
    _PersistentCommand.PersistentCommandAddExpectedRevision(b, expected_rev)
    _PersistentCommand.PersistentCommandAddOpType(b, op_type)
    _PersistentCommand.PersistentCommandAddOp(b, op_off)
    cmd = _PersistentCommand.PersistentCommandEnd(b)
    _RequestFrame.RequestFrameStart(b)
    _RequestFrame.RequestFrameAddRequestType(b, ControlRequest.PersistentCommand)
    _RequestFrame.RequestFrameAddRequest(b, cmd)
    rf = _RequestFrame.RequestFrameEnd(b)
    _ControlEnvelope.ControlEnvelopeStart(b)
    _ControlEnvelope.ControlEnvelopeAddFrameType(b, ControlFrame.RequestFrame)
    _ControlEnvelope.ControlEnvelopeAddFrame(b, rf)
    b.Finish(_ControlEnvelope.ControlEnvelopeEnd(b))
    return bytes(b.Output())


def build_transport(op, req_id, position_ticks=0, cycle_start=0, cycle_end=0):
    b = flatbuffers.Builder(0)
    rid = b.CreateString(req_id)
    _TransportRequest.TransportRequestStart(b)
    _TransportRequest.TransportRequestAddRequestId(b, rid)
    _TransportRequest.TransportRequestAddOp(b, op)
    _TransportRequest.TransportRequestAddPositionTicks(b, position_ticks)
    _TransportRequest.TransportRequestAddCycleStartTicks(b, cycle_start)
    _TransportRequest.TransportRequestAddCycleEndTicks(b, cycle_end)
    tr = _TransportRequest.TransportRequestEnd(b)
    _RequestFrame.RequestFrameStart(b)
    _RequestFrame.RequestFrameAddRequestType(b, ControlRequest.TransportRequest)
    _RequestFrame.RequestFrameAddRequest(b, tr)
    rf = _RequestFrame.RequestFrameEnd(b)
    _ControlEnvelope.ControlEnvelopeStart(b)
    _ControlEnvelope.ControlEnvelopeAddFrameType(b, ControlFrame.RequestFrame)
    _ControlEnvelope.ControlEnvelopeAddFrame(b, rf)
    b.Finish(_ControlEnvelope.ControlEnvelopeEnd(b))
    return bytes(b.Output())


def read_receipt(frame_bytes):
    env = ControlEnvelope.GetRootAsControlEnvelope(frame_bytes, 0)
    if env.FrameType() != ControlFrame.EventFrame:
        return ("other", env.FrameType())
    ef = EventFrame(); ef.Init(env.Frame().Bytes, env.Frame().Pos)
    if ef.EventType() != ControlEvent.CommandReceipt:
        return ("event", ef.EventType())
    r = CommandReceipt(); r.Init(ef.Event().Bytes, ef.Event().Pos)
    err = r.Error()
    msg = ""
    try:
        msg = r.Message().decode() if r.Message() else ""
    except Exception:
        pass
    return ("receipt", r.CommandId().decode(), r.Status(), err, r.Revision(), msg)


def await_receipt(sock, cid, timeout=15):
    deadline = time.time() + timeout
    while time.time() < deadline:
        got = read_receipt(recv_frame(sock, timeout))
        if got[0] == "receipt" and got[1] == cid:
            return got
    raise TimeoutError(cid)


def _mk_add_track(b):
    tid = b.CreateString("t1"); nm = b.CreateString("Keys")
    _AddTrackOp.AddTrackOpStart(b)
    _AddTrackOp.AddTrackOpAddTrackId(b, tid)
    _AddTrackOp.AddTrackOpAddKind(b, TrackKind.INSTRUMENT)
    _AddTrackOp.AddTrackOpAddName(b, nm)
    return _AddTrackOp.AddTrackOpEnd(b)

def _mk_midi_clip(b):
    cid = b.CreateString("clip1"); tid = b.CreateString("t1")
    _InsertMidiClipOp.InsertMidiClipOpStart(b)
    _InsertMidiClipOp.InsertMidiClipOpAddClipId(b, cid)
    _InsertMidiClipOp.InsertMidiClipOpAddTrackId(b, tid)
    _InsertMidiClipOp.InsertMidiClipOpAddStartTicks(b, 0)
    _InsertMidiClipOp.InsertMidiClipOpAddLengthTicks(b, 61_440_000)
    return _InsertMidiClipOp.InsertMidiClipOpEnd(b)

def _mk_note(b):
    cid = b.CreateString("clip1"); nid = b.CreateString("n1")
    _InsertNoteOp.InsertNoteOpStart(b)
    _InsertNoteOp.InsertNoteOpAddClipId(b, cid)
    _InsertNoteOp.InsertNoteOpAddNoteId(b, nid)
    _InsertNoteOp.InsertNoteOpAddPitch(b, 60)
    _InsertNoteOp.InsertNoteOpAddVelocity(b, 100)
    _InsertNoteOp.InsertNoteOpAddStartTicks(b, 0)
    _InsertNoteOp.InsertNoteOpAddLengthTicks(b, 960_000)
    return _InsertNoteOp.InsertNoteOpEnd(b)

def _mk_mute(b):
    tid = b.CreateString("t1")
    _SetTrackMuteOp.SetTrackMuteOpStart(b)
    _SetTrackMuteOp.SetTrackMuteOpAddTrackId(b, tid)
    _SetTrackMuteOp.SetTrackMuteOpAddMuted(b, True)
    return _SetTrackMuteOp.SetTrackMuteOpEnd(b)

def _mk_gain(b):
    tid = b.CreateString("t1")
    _SetTrackGainOp.SetTrackGainOpStart(b)
    _SetTrackGainOp.SetTrackGainOpAddTrackId(b, tid)
    _SetTrackGainOp.SetTrackGainOpAddGainLinear(b, 0.5)
    return _SetTrackGainOp.SetTrackGainOpEnd(b)

def main():
    tmp = tempfile.mkdtemp(prefix="void-sock-")
    ctrl_path = os.path.join(tmp, "control.sock")
    tele_path = os.path.join(tmp, "tele.sock")
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(ctrl_path)
    listener.listen(1)
    tele_listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    tele_listener.bind(tele_path)
    tele_listener.listen(1)

    env = dict(os.environ)
    env["VOID_CONTROL_SOCK"] = ctrl_path
    env["VOID_TELEMETRY_SOCK"] = tele_path
    env["VOID_WORKER_TOKEN"] = "tok-smoke"
    env["VOID_WORKER_INSTANCE_ID"] = "inst-smoke-1"

    print(f"# spawning {BIN}")
    proc = subprocess.Popen([BIN], env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    listener.settimeout(20)
    sock, _ = listener.accept()
    tele_listener.settimeout(20)
    tele_sock, _ = tele_listener.accept()

    # 1. WorkerHello must be the first frame.
    hello_frame = recv_frame(sock)
    env_ = ControlEnvelope.GetRootAsControlEnvelope(hello_frame, 0)
    check("first frame is WorkerHello",
          env_.FrameType() == ControlFrame.WorkerHello)
    wh = WorkerHello(); wh.Init(env_.Frame().Bytes, env_.Frame().Pos)
    check("worker kind ENGINE", wh.WorkerKind() == WorkerKind.ENGINE)
    check("protocol 1.0",
          wh.ProtocolMajor() == 1 and wh.ProtocolMinor() == 0)
    check("launch token echoed", wh.LaunchToken().decode() == "tok-smoke")
    check("engine_epoch 1", wh.EngineEpoch() == 1)

    container = os.path.join(tmp, "proj1")
    os.makedirs(container, exist_ok=True)

    def op_create_project(b):
        nm = b.CreateString("smoke-project")
        cd = b.CreateString(container)
        _CreateProjectOp.CreateProjectOpStart(b)
        _CreateProjectOp.CreateProjectOpAddName(b, nm)
        _CreateProjectOp.CreateProjectOpAddContainerDir(b, cd)
        _CreateProjectOp.CreateProjectOpAddSampleRate(b, 48000.0)
        _CreateProjectOp.CreateProjectOpAddInitialBpm(b, 120.0)
        return _CreateProjectOp.CreateProjectOpEnd(b)

    seq = [
        ("create-project", build_command(PersistentOp.CreateProjectOp,
                                          op_create_project, "c1"),
         "c1", [0]),
        ("dup-create", build_command(PersistentOp.CreateProjectOp,
                                     op_create_project, "c1"), "c1", [1]),  # DUPLICATE
        ("add-track", build_command(
            PersistentOp.AddTrackOp, _mk_add_track, "c2", expected_rev=1), "c2", [0]),
        ("insert-midi-clip", build_command(
            PersistentOp.InsertMidiClipOp, _mk_midi_clip,
            "c3", expected_rev=2), "c3", [0]),
        ("insert-note", build_command(
            PersistentOp.InsertNoteOp, _mk_note,
            "c4", expected_rev=3), "c4", [0]),
        ("set-tempo", build_command(
            PersistentOp.SetTempoOp,
            lambda b: (_SetTempoOp.SetTempoOpStart(b),
                       _SetTempoOp.SetTempoOpAddAtTicks(b, 0),
                       _SetTempoOp.SetTempoOpAddBpm(b, 132.0),
                       _SetTempoOp.SetTempoOpEnd(b))[-1],
            "c5", expected_rev=4), "c5", [0]),
        ("stale-revision", build_command(
            PersistentOp.SetTrackMuteOp,
            _mk_mute,
            "c6", expected_rev=999), "c6", [2]),  # REJECTED(STALE_REVISION)
        ("save-project", build_command(
            PersistentOp.SaveProjectOp,
            lambda b: (_SaveProjectOp.SaveProjectOpStart(b), _SaveProjectOp.SaveProjectOpEnd(b))[-1],
            "c7", expected_rev=5), "c7", [0]),
        ("cmd-id-reuse", build_command(
            PersistentOp.SetTrackGainOp, _mk_gain,
            "c2", expected_rev=6), "c2", [2]),  # REJECTED(COMMAND_ID_REUSE)
    ]

    for name, frame, cid, ok_statuses in seq:
        try:
            send_frame(sock, frame)
            got = await_receipt(sock, cid)
            check(name, got[2] in ok_statuses,
                  f"status={got[2]} err={got[3]} rev={got[4]} msg={got[5]}")
        except Exception as e:
            check(name, False, repr(e))
            break

    # Transport: PLAY + TransportAck
    try:
        send_frame(sock, build_transport(TransportOp.PLAY, "tp1"))
        try:
            frame = recv_frame(tele_sock, 10)
            check("telemetry frame after PLAY", True,
                  f"{len(frame)}B sha={hashlib.sha256(frame).hexdigest()[:12]}")
        except Exception as e:
            check("telemetry frame after PLAY", False, repr(e))
        send_frame(sock, build_transport(TransportOp.STOP, "tp2"))
    except Exception as e:
        check("transport send", False, repr(e))

    proc.terminate()
    try:
        out = proc.communicate(timeout=8)[0].decode(errors="replace")
    except subprocess.TimeoutExpired:
        proc.kill(); out = ""
    print("--- worker log (tail) ---")
    print("\n".join(out.strip().splitlines()[-30:]))
    print(f"worker exit: {proc.returncode}")

    n = len(results)
    print(f"# {sum(results)}/{n} checks passed")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
