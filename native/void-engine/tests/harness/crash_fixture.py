#!/usr/bin/env python3
"""T24 — engine-hosted plugin crash fixture.

Inserts a plugin instance whose plugin_uid is "void.crash" (test fixture —
arms the worker's crash flag), then sends PLAY. The worker must abort on
playback start; the supervisor must observe clean socket death and a non-zero
exit (SIGABRT). Prints PASS/FAIL per assertion; exit 0 only if all pass.

Usage: PYTHONPATH=<gen'd voidproto dir> python3 crash_fixture.py <void-engine-bin>
"""
import os, sys, socket, struct, subprocess, tempfile, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import flatbuffers  # noqa: E402
import voidproto.ControlEnvelope as _ControlEnvelope
from voidproto.ControlEnvelope import ControlEnvelope  # noqa: E402
from voidproto.ControlFrame import ControlFrame  # noqa: E402
import voidproto.RequestFrame as _RequestFrame
from voidproto.RequestFrame import RequestFrame  # noqa: E402
from voidproto.EventFrame import EventFrame  # noqa: E402
from voidproto.ControlEvent import ControlEvent  # noqa: E402
from voidproto.ControlRequest import ControlRequest  # noqa: E402
import voidproto.PersistentCommand as _PersistentCommand
from voidproto.PersistentCommand import PersistentCommand  # noqa: E402
from voidproto.PersistentOp import PersistentOp  # noqa: E402
import voidproto.CreateProjectOp as _CreateProjectOp
import voidproto.AddTrackOp as _AddTrackOp
from voidproto.TrackKind import TrackKind  # noqa: E402
import voidproto.InsertPluginOp as _InsertPluginOp
from voidproto.WorkerHello import WorkerHello  # noqa: E402
import voidproto.TransportRequest as _TransportRequest
from voidproto.TransportOp import TransportOp  # noqa: E402

BIN = sys.argv[1]
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


def build_command(op_type, op_builder, cid, expected_rev):
    b = flatbuffers.Builder(0)
    cid_s = b.CreateString(cid)
    txn = b.CreateString("txn-" + cid)
    pid = b.CreateString("t-proj")
    op_off = op_builder(b)
    _PersistentCommand.PersistentCommandStart(b)
    _PersistentCommand.PersistentCommandAddCommandId(b, cid_s)
    _PersistentCommand.PersistentCommandAddTransactionId(b, txn)
    _PersistentCommand.PersistentCommandAddProjectId(b, pid)
    _PersistentCommand.PersistentCommandAddEngineEpoch(b, 1)
    _PersistentCommand.PersistentCommandAddExpectedRevision(b, expected_rev)
    _PersistentCommand.PersistentCommandAddOpType(b, op_type)
    _PersistentCommand.PersistentCommandAddOp(b, op_off)
    pc = _PersistentCommand.PersistentCommandEnd(b)

    _RequestFrame.RequestFrameStart(b)
    _RequestFrame.RequestFrameAddRequestType(b, ControlRequest.PersistentCommand)
    _RequestFrame.RequestFrameAddRequest(b, pc)
    rf = _RequestFrame.RequestFrameEnd(b)

    _ControlEnvelope.ControlEnvelopeStart(b)
    _ControlEnvelope.ControlEnvelopeAddFrameType(b, ControlFrame.RequestFrame)
    _ControlEnvelope.ControlEnvelopeAddFrame(b, rf)
    b.Finish(_ControlEnvelope.ControlEnvelopeEnd(b))
    return bytes(b.Output())


def build_transport(op, rid):
    b = flatbuffers.Builder(0)
    rid_s = b.CreateString(rid)
    _TransportRequest.TransportRequestStart(b)
    _TransportRequest.TransportRequestAddRequestId(b, rid_s)
    _TransportRequest.TransportRequestAddOp(b, op)
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


def main():
    tmp = tempfile.mkdtemp(prefix="void-crash-")
    ctrl_path, tele_path = os.path.join(tmp, "c.sock"), os.path.join(tmp, "t.sock")
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(ctrl_path); listener.listen(1)
    tele_listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    tele_listener.bind(tele_path); tele_listener.listen(1)

    env = dict(os.environ)
    env.update(VOID_CONTROL_SOCK=ctrl_path, VOID_TELEMETRY_SOCK=tele_path,
               VOID_WORKER_TOKEN="tok-t24", VOID_WORKER_INSTANCE_ID="inst-t24")

    proc = subprocess.Popen([BIN], env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    listener.settimeout(20)
    sock, _ = listener.accept()
    tele_listener.settimeout(20)
    tele_sock, _ = tele_listener.accept()

    env_ = ControlEnvelope.GetRootAsControlEnvelope(recv_frame(sock), 0)
    wh = WorkerHello(); wh.Init(env_.Frame().Bytes, env_.Frame().Pos)
    check("hello", wh.EngineEpoch() == 1)

    container = os.path.join(tmp, "proj")
    os.makedirs(container, exist_ok=True)

    def op_create(b):
        nm = b.CreateString("crash-proj"); cd = b.CreateString(container)
        _CreateProjectOp.CreateProjectOpStart(b)
        _CreateProjectOp.CreateProjectOpAddName(b, nm)
        _CreateProjectOp.CreateProjectOpAddContainerDir(b, cd)
        return _CreateProjectOp.CreateProjectOpEnd(b)

    def op_track(b):
        tid = b.CreateString("t1")
        _AddTrackOp.AddTrackOpStart(b)
        _AddTrackOp.AddTrackOpAddTrackId(b, tid)
        _AddTrackOp.AddTrackOpAddKind(b, TrackKind.AUDIO)
        return _AddTrackOp.AddTrackOpEnd(b)

    def op_crash_plugin(b):
        tid = b.CreateString("t1")
        inst = b.CreateString("plug-crash-1")
        fmt = b.CreateString("VST3")
        uid = b.CreateString("void.crash")
        _InsertPluginOp.InsertPluginOpStart(b)
        _InsertPluginOp.InsertPluginOpAddTrackId(b, tid)
        _InsertPluginOp.InsertPluginOpAddSlot(b, 0)
        _InsertPluginOp.InsertPluginOpAddPluginInstanceId(b, inst)
        _InsertPluginOp.InsertPluginOpAddFormat(b, fmt)
        _InsertPluginOp.InsertPluginOpAddPluginUid(b, uid)
        return _InsertPluginOp.InsertPluginOpEnd(b)

    seq = [
        ("create", build_command(PersistentOp.CreateProjectOp, op_create, "c1", 0)),
        ("add-track", build_command(PersistentOp.AddTrackOp, op_track, "c2", 1)),
        ("insert void.crash", build_command(PersistentOp.InsertPluginOp,
                                            op_crash_plugin, "c3", 2)),
    ]
    for name, frame in seq:
        send_frame(sock, frame)
        got = recv_frame(sock)
        e = ControlEnvelope.GetRootAsControlEnvelope(got, 0)
        ef = EventFrame(); ef.Init(e.Frame().Bytes, e.Frame().Pos)
        from voidproto.CommandReceipt import CommandReceipt
        r = CommandReceipt(); r.Init(ef.Event().Bytes, ef.Event().Pos)
        msg = r.Message().decode() if r.Message() else ""
        check(f"{name} -> status={r.Status()} msg={msg}",
              r.Status() == 0, f"err={r.Error()}")

    # Arm confirmed; PLAY triggers the in-process crash.
    send_frame(sock, build_transport(TransportOp.PLAY, "tp1"))
    deadline = time.time() + 15
    died = False
    while time.time() < deadline:
        rc = proc.poll()
        if rc is not None:
            died = True
            check("worker died on PLAY", rc < 0,
                  f"exit={rc} (expect negative signal; -6=SIGABRT)")
            break
        time.sleep(0.1)
    if not died:
        check("worker died on PLAY", False, "still alive after 15s")
        proc.terminate()

    try:
        recv_frame(sock, 3)
        check("control socket died", False, "frame received after crash")
    except (EOFError, socket.timeout, ConnectionResetError):
        check("control socket died", True, "EOF/reset")
    except Exception as e:
        check("control socket died", True, repr(e))

    out = b""
    try:
        out = proc.communicate(timeout=5)[0]
    except Exception:
        proc.kill()
    tail = (out or b"").decode(errors="replace").splitlines()[-3:]
    for line in tail:
        print("#", line)
    print(f"# {sum(results)}/{len(results)} checks passed")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
