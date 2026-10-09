#!/usr/bin/env python3
"""F5-N11 waveclip resolver evidence for void-engine.

Drives `void-engine` through the supervisor UDS like supervisor_stub.py:

  Scenario A — real media round-trip:
    create -> add track -> attach WAV asset -> insert audio clip -> save
    -> kill worker -> respawn -> open same container
    MUST APPLY (wave-clip `source` resolves via filePathResolver anchored
    at the checkpoint edit file; the F1 gap meant these resolved
    process-CWD-relative and loaded as silence).

  Scenario B — missing media is a typed error:
    delete the asset blob -> reopen
    MUST REJECT with ErrorCode.ASSET_MISSING, not open-to-silence/crash.

Usage: python3 waveclip_checks.py <path-to-void-engine-binary>
Requires: flatbuffers (pip), generated bindings under ./voidproto.
"""
import os, sys, socket, struct, subprocess, tempfile, time, wave

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
import voidproto.PersistentOp as _PersistentOp
from voidproto.PersistentOp import PersistentOp  # noqa: E402
import voidproto.CreateProjectOp as _CreateProjectOp
import voidproto.OpenProjectOp as _OpenProjectOp
import voidproto.AddTrackOp as _AddTrackOp
import voidproto.AttachAssetOp as _AttachAssetOp
import voidproto.InsertAudioClipOp as _InsertAudioClipOp
import voidproto.SaveProjectOp as _SaveProjectOp
from voidproto.TrackKind import TrackKind  # noqa: E402
from voidproto.AckStatus import AckStatus  # noqa: E402
from voidproto.ErrorCode import ErrorCode  # noqa: E402
from voidproto.WorkerHello import WorkerHello  # noqa: E402
from voidproto.WorkerKind import WorkerKind  # noqa: E402

BIN = sys.argv[1]
results = []


def check(name, ok, detail=""):
    results.append(ok)
    print(("PASS" if ok else "FAIL") + f" {name} {detail}")


def send_frame(sock, buf):
    sock.sendall(struct.pack("<I", len(buf)) + buf)


def recv_frame(sock, timeout=20):
    sock.settimeout(timeout)
    hdr = b""
    while len(hdr) < 4:
        hdr += sock.recv(4 - len(hdr))
    n = struct.unpack("<I", hdr)[0]
    data = b""
    while len(data) < n:
        data += sock.recv(n - len(data))
    return data


def build_command(op_type, op_builder, cid, expected_rev=0):
    b = flatbuffers.Builder(0)
    cid_s = b.CreateString(cid)
    txn = b.CreateString("txn-" + cid)
    pid = b.CreateString("waveclip-proj")
    op_off = op_builder(b)
    _PersistentCommand.PersistentCommandStart(b)
    _PersistentCommand.PersistentCommandAddCommandId(b, cid_s)
    _PersistentCommand.PersistentCommandAddTransactionId(b, txn)
    _PersistentCommand.PersistentCommandAddProjectId(b, pid)
    _PersistentCommand.PersistentCommandAddEngineEpoch(b, 1)
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


def read_receipt(frame_bytes):
    env = ControlEnvelope.GetRootAsControlEnvelope(frame_bytes, 0)
    if env.FrameType() != ControlFrame.EventFrame:
        return ("other",)
    ef = EventFrame(); ef.Init(env.Frame().Bytes, env.Frame().Pos)
    if ef.EventType() != ControlEvent.CommandReceipt:
        return ("event",)
    r = __import__("voidproto.CommandReceipt", fromlist=["CommandReceipt"]).CommandReceipt()
    r.Init(ef.Event().Bytes, ef.Event().Pos)
    return ("receipt", r.CommandId().decode(), r.Status(), r.Error(),
            r.Revision(), r.Message().decode() if r.Message() else "")


def await_receipt(sock, cid, timeout=20):
    deadline = time.time() + timeout
    while time.time() < deadline:
        got = read_receipt(recv_frame(sock, timeout))
        if got[0] == "receipt" and got[1] == cid:
            return got
    raise TimeoutError(cid)


def spawn(tmp):
    ctrl_path = os.path.join(tmp, "control.sock")
    tele_path = os.path.join(tmp, "tele.sock")
    for p in (ctrl_path, tele_path):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(ctrl_path)
    listener.listen(1)
    tele_listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    tele_listener.bind(tele_path)
    tele_listener.listen(1)
    env = dict(os.environ)
    env["VOID_CONTROL_SOCK"] = ctrl_path
    env["VOID_TELEMETRY_SOCK"] = tele_path
    env["VOID_WORKER_TOKEN"] = "tok-waveclip"
    env["VOID_WORKER_INSTANCE_ID"] = "inst-waveclip-1"
    proc = subprocess.Popen([BIN], env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    listener.settimeout(20)
    sock, _ = listener.accept()
    tele_listener.settimeout(20)
    tele_sock, _ = tele_listener.accept()
    hello = ControlEnvelope.GetRootAsControlEnvelope(recv_frame(sock), 0)
    assert hello.FrameType() == ControlFrame.WorkerHello
    return proc, sock, tele_sock


def make_wav(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with wave.open(path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(48000)
        frames = bytearray()
        for i in range(48000):  # 1 s sine-ish
            v = int(16000 * ((i * 440 * 2 * 3.14159 / 48000) % 1.0 - 0.5))
            frames += v.to_bytes(2, "little", signed=True) * 2
        w.writeframes(bytes(frames))


def op_create(container):
    def f(b):
        nm = b.CreateString("waveclip-project")
        cd = b.CreateString(container)
        _CreateProjectOp.CreateProjectOpStart(b)
        _CreateProjectOp.CreateProjectOpAddName(b, nm)
        _CreateProjectOp.CreateProjectOpAddContainerDir(b, cd)
        _CreateProjectOp.CreateProjectOpAddSampleRate(b, 48000.0)
        _CreateProjectOp.CreateProjectOpAddInitialBpm(b, 120.0)
        return _CreateProjectOp.CreateProjectOpEnd(b)
    return f


def op_open(container):
    def f(b):
        cd = b.CreateString(container)
        _OpenProjectOp.OpenProjectOpStart(b)
        _OpenProjectOp.OpenProjectOpAddContainerDir(b, cd)
        return _OpenProjectOp.OpenProjectOpEnd(b)
    return f


def op_add_track(b):
    tid = b.CreateString("track-1")
    _AddTrackOp.AddTrackOpStart(b)
    _AddTrackOp.AddTrackOpAddTrackId(b, tid)
    _AddTrackOp.AddTrackOpAddKind(b, TrackKind.AUDIO)
    return _AddTrackOp.AddTrackOpEnd(b)


def op_attach(rel_path):
    def f(b):
        aid = b.CreateString("asset-1")
        rel = b.CreateString(rel_path)
        mt = b.CreateString("audio/wav")
        _AttachAssetOp.AttachAssetOpStart(b)
        _AttachAssetOp.AttachAssetOpAddAssetId(b, aid)
        _AttachAssetOp.AttachAssetOpAddMediaType(b, mt)
        _AttachAssetOp.AttachAssetOpAddRelPath(b, rel)
        _AttachAssetOp.AttachAssetOpAddChannels(b, 2)
        _AttachAssetOp.AttachAssetOpAddDurationTicks(b, 960000 * 2)
        return _AttachAssetOp.AttachAssetOpEnd(b)
    return f


def op_insert_clip(b):
    cid = b.CreateString("clip-1")
    tid = b.CreateString("track-1")
    aid = b.CreateString("asset-1")
    _InsertAudioClipOp.InsertAudioClipOpStart(b)
    _InsertAudioClipOp.InsertAudioClipOpAddClipId(b, cid)
    _InsertAudioClipOp.InsertAudioClipOpAddTrackId(b, tid)
    _InsertAudioClipOp.InsertAudioClipOpAddAssetId(b, aid)
    _InsertAudioClipOp.InsertAudioClipOpAddStartTicks(b, 0)
    _InsertAudioClipOp.InsertAudioClipOpAddLengthTicks(b, 960000 * 2)
    _InsertAudioClipOp.InsertAudioClipOpAddOffsetTicks(b, 0)
    return _InsertAudioClipOp.InsertAudioClipOpEnd(b)


def op_save(b):
    _SaveProjectOp.SaveProjectOpStart(b)
    return _SaveProjectOp.SaveProjectOpEnd(b)


def main():
    tmp = tempfile.mkdtemp(prefix="void-waveclip-")
    container = os.path.join(tmp, "proj")
    sha = "f" * 64
    rel = os.path.join(sha, "take.wav")          # container/assets/sha256/<sha>/take.wav
    blob = os.path.join(container, "assets", "sha256", rel)

    # --- Scenario A: insert clip, save, kill, reopen --------------------------
    proc, sock, tele = spawn(tmp)
    steps = [
        ("create-project", PersistentOp.CreateProjectOp, op_create(container), "w1", 0),
        ("add-track", PersistentOp.AddTrackOp, op_add_track, "w2", 1),
    ]
    for name, t, f, cid, rev in steps:
        send_frame(sock, build_command(t, f, cid, expected_rev=rev))
        got = await_receipt(sock, cid)
        check(name, got[2] == AckStatus.APPLIED, f"status={got[2]} err={got[3]} msg={got[5]}")

    make_wav(blob)  # asset exists before attach (coordinator ingest order)
    for name, t, f, cid, rev in [
        ("attach-asset", PersistentOp.AttachAssetOp, op_attach(rel), "w3", 2),
        ("insert-audio-clip", PersistentOp.InsertAudioClipOp, op_insert_clip, "w4", 3),
        ("save-project", PersistentOp.SaveProjectOp, op_save, "w5", 4),
    ]:
        send_frame(sock, build_command(t, f, cid, expected_rev=rev))
        got = await_receipt(sock, cid)
        check(name, got[2] == AckStatus.APPLIED, f"status={got[2]} err={got[3]} msg={got[5]}")

    # The persisted AUDIOCLIP source must be a relative anchor, not absolute.
    import glob
    edits = glob.glob(os.path.join(container, "checkpoints", "*", "engine.tracktionedit"))
    edits += [os.path.join(container, "engine.tracktionedit")]
    src_line = ""
    for e in edits:
        if os.path.isfile(e):
            text = open(e).read()
            if "AUDIOCLIP" in text:
                for line in text.splitlines():
                    if "source=" in line:
                        src_line = line.strip()
    check("AUDIOCLIP persisted in checkpoint", src_line != "", src_line[:110])
    check("source is container-relative", "assets/sha256" in src_line and "../" in src_line,
          src_line[:110])

    proc.terminate()
    try:
        proc.communicate(timeout=8)
    except subprocess.TimeoutExpired:
        proc.kill()

    # --- Reopen on a fresh worker: wave-clip refs must resolve -----------------
    proc, sock, tele = spawn(tmp)
    send_frame(sock, build_command(PersistentOp.OpenProjectOp, op_open(container), "w6", expected_rev=0))
    got = await_receipt(sock, "w6")
    check("reopen with media resolves wave clip", got[2] == AckStatus.APPLIED,
          f"status={got[2]} err={got[3]} msg={got[5]}")

    # --- Scenario B: missing blob -> typed ASSET_MISSING ----------------------
    os.remove(blob)
    proc.terminate()
    try:
        proc.communicate(timeout=8)
    except subprocess.TimeoutExpired:
        proc.kill()
    proc, sock, tele = spawn(tmp)
    send_frame(sock, build_command(PersistentOp.OpenProjectOp, op_open(container), "w7", expected_rev=0))
    got = await_receipt(sock, "w7")
    check("missing blob -> REJECTED + ASSET_MISSING",
          got[2] == AckStatus.REJECTED and got[3] == ErrorCode.ASSET_MISSING,
          f"status={got[2]} err={got[3]} msg={got[5]}")

    proc.terminate()
    try:
        out = proc.communicate(timeout=8)[0].decode(errors="replace")
    except subprocess.TimeoutExpired:
        proc.kill(); out = ""
    print("--- worker log (tail) ---")
    print("\n".join(out.strip().splitlines()[-15:]))
    print(f"worker exit: {proc.returncode}")

    n = len(results)
    print(f"# {sum(results)}/{n} checks passed")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
