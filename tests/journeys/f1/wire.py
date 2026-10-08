"""VOID engine wire client for F1 journey evidence.

Speaks the real control + telemetry UDS protocol (protocol/void_control.fbs,
FlatBuffers, length-prefixed frames) to a spawned `void-engine` worker — the
same contract `native/void-engine/tests/harness/supervisor_stub.py` verifies.
No in-process shortcuts: every document mutation is a PersistentCommand on the
socket; every read is a ReadRequest; transport goes through TransportRequest.

Requires: flatbuffers (PYTHONPATH=third_party/flatbuffers/python) and the
generated bindings in native/void-engine/tests/harness/voidproto.
"""
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import time

HARNESS = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                       "../../../native/void-engine/tests/harness")
sys.path.insert(0, os.path.abspath(HARNESS))

import flatbuffers  # noqa: E402
import voidproto.ControlEnvelope as _ControlEnvelope  # noqa: E402
from voidproto.ControlEnvelope import ControlEnvelope  # noqa: E402
from voidproto.ControlFrame import ControlFrame  # noqa: E402
import voidproto.RequestFrame as _RequestFrame  # noqa: E402
from voidproto.RequestFrame import RequestFrame  # noqa: E402
from voidproto.EventFrame import EventFrame  # noqa: E402
from voidproto.ControlRequest import ControlRequest  # noqa: E402
from voidproto.ControlEvent import ControlEvent  # noqa: E402
import voidproto.PersistentCommand as _PersistentCommand  # noqa: E402
from voidproto.PersistentCommand import PersistentCommand  # noqa: E402
from voidproto.PersistentOp import PersistentOp  # noqa: E402
import voidproto.TransportRequest as _TransportRequest  # noqa: E402
from voidproto.TransportRequest import TransportRequest  # noqa: E402
import voidproto.ReadRequest as _ReadRequest  # noqa: E402
from voidproto.ReadRequest import ReadRequest  # noqa: E402
import voidproto.TelemetryFrame as _TelemetryFrame  # noqa: E402
from voidproto.TelemetryFrame import TelemetryFrame  # noqa: E402
from voidproto.TelemetryEvent import TelemetryEvent  # noqa: E402
from voidproto.ClockSnapshot import ClockSnapshot  # noqa: E402
from voidproto.CommandReceipt import CommandReceipt  # noqa: E402
from voidproto.ReadResponse import ReadResponse  # noqa: E402
from voidproto.SaveResultEvent import SaveResultEvent  # noqa: E402
from voidproto.WorkerHello import WorkerHello  # noqa: E402
from voidproto.WorkerKind import WorkerKind  # noqa: E402

# Op tables (builders import lazily by name to keep one map).
import voidproto.CreateProjectOp as _CreateProjectOp  # noqa: E402
import voidproto.OpenProjectOp as _OpenProjectOp  # noqa: E402
import voidproto.CloseProjectOp as _CloseProjectOp  # noqa: E402
import voidproto.AddTrackOp as _AddTrackOp  # noqa: E402
import voidproto.RemoveTrackOp as _RemoveTrackOp  # noqa: E402
import voidproto.SetTrackNameOp as _SetTrackNameOp  # noqa: E402
import voidproto.SetTrackGainOp as _SetTrackGainOp  # noqa: E402
import voidproto.SetTrackPanOp as _SetTrackPanOp  # noqa: E402
import voidproto.SetTrackMuteOp as _SetTrackMuteOp  # noqa: E402
import voidproto.SetTrackSoloOp as _SetTrackSoloOp  # noqa: E402
import voidproto.InsertAudioClipOp as _InsertAudioClipOp  # noqa: E402
import voidproto.InsertMidiClipOp as _InsertMidiClipOp  # noqa: E402
import voidproto.RemoveClipOp as _RemoveClipOp  # noqa: E402
import voidproto.MoveClipOp as _MoveClipOp  # noqa: E402
import voidproto.TrimClipOp as _TrimClipOp  # noqa: E402
import voidproto.SplitClipOp as _SplitClipOp  # noqa: E402
import voidproto.InsertNoteOp as _InsertNoteOp  # noqa: E402
import voidproto.RemoveNoteOp as _RemoveNoteOp  # noqa: E402
import voidproto.SetNoteOp as _SetNoteOp  # noqa: E402
import voidproto.SetTempoOp as _SetTempoOp  # noqa: E402
import voidproto.SetTimeSignatureOp as _SetTimeSignatureOp  # noqa: E402
import voidproto.SetLoopRangeOp as _SetLoopRangeOp  # noqa: E402
import voidproto.SaveProjectOp as _SaveProjectOp  # noqa: E402
import voidproto.CreateCheckpointOp as _CreateCheckpointOp  # noqa: E402
import voidproto.UndoOp as _UndoOp  # noqa: E402
import voidproto.RedoOp as _RedoOp  # noqa: E402
import voidproto.AttachAssetOp as _AttachAssetOp  # noqa: E402
import voidproto.InsertPluginOp as _InsertPluginOp  # noqa: E402
import voidproto.RemovePluginOp as _RemovePluginOp  # noqa: E402
import voidproto.SetPluginParamOp as _SetPluginParamOp  # noqa: E402

_OP_MOD = {
    "CreateProjectOp": _CreateProjectOp, "OpenProjectOp": _OpenProjectOp,
    "CloseProjectOp": _CloseProjectOp, "AddTrackOp": _AddTrackOp,
    "RemoveTrackOp": _RemoveTrackOp, "SetTrackNameOp": _SetTrackNameOp,
    "SetTrackGainOp": _SetTrackGainOp, "SetTrackPanOp": _SetTrackPanOp,
    "SetTrackMuteOp": _SetTrackMuteOp, "SetTrackSoloOp": _SetTrackSoloOp,
    "InsertAudioClipOp": _InsertAudioClipOp,
    "InsertMidiClipOp": _InsertMidiClipOp, "RemoveClipOp": _RemoveClipOp,
    "MoveClipOp": _MoveClipOp, "TrimClipOp": _TrimClipOp,
    "SplitClipOp": _SplitClipOp, "InsertNoteOp": _InsertNoteOp,
    "RemoveNoteOp": _RemoveNoteOp, "SetNoteOp": _SetNoteOp,
    "SetTempoOp": _SetTempoOp, "SetTimeSignatureOp": _SetTimeSignatureOp,
    "SetLoopRangeOp": _SetLoopRangeOp, "SaveProjectOp": _SaveProjectOp,
    "CreateCheckpointOp": _CreateCheckpointOp, "UndoOp": _UndoOp,
    "RedoOp": _RedoOp, "AttachAssetOp": _AttachAssetOp,
    "InsertPluginOp": _InsertPluginOp, "RemovePluginOp": _RemovePluginOp,
    "SetPluginParamOp": _SetPluginParamOp,
}
_OP_ENUM = {name: getattr(PersistentOp, name) for name in _OP_MOD}

TICKS_PER_QUARTER = 960_000
BAR = 4 * TICKS_PER_QUARTER  # 4/4


def send_frame(sock, buf):
    sock.sendall(struct.pack("<I", len(buf)) + buf)


def recv_frame(sock, timeout=20):
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


def build_command(op_name, fill, cid, project_id, expected_rev, epoch=1):
    """fill: fn(builder) -> op table offset."""
    b = flatbuffers.Builder(0)
    cid_s = b.CreateString(cid)
    txn = b.CreateString("txn-" + cid)
    pid = b.CreateString(project_id)
    op_off = fill(b)
    mod = _OP_MOD[op_name]
    start = getattr(mod, op_name + "Start")
    _PersistentCommand.PersistentCommandStart(b)
    _PersistentCommand.PersistentCommandAddCommandId(b, cid_s)
    _PersistentCommand.PersistentCommandAddTransactionId(b, txn)
    _PersistentCommand.PersistentCommandAddProjectId(b, pid)
    _PersistentCommand.PersistentCommandAddEngineEpoch(b, epoch)
    _PersistentCommand.PersistentCommandAddExpectedRevision(b, expected_rev)
    _PersistentCommand.PersistentCommandAddOpType(b, _OP_ENUM[op_name])
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


def build_transport(op, req_id, position_ticks=-1, cycle_start=0, cycle_end=0):
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


def build_read(view, req_id, cursor="", track_id="", start=-1, end=-1):
    b = flatbuffers.Builder(0)
    rid = b.CreateString(req_id)
    cur = b.CreateString(cursor)
    tid = b.CreateString(track_id)
    _ReadRequest.ReadRequestStart(b)
    _ReadRequest.ReadRequestAddRequestId(b, rid)
    _ReadRequest.ReadRequestAddView(b, view)
    _ReadRequest.ReadRequestAddCursor(b, cur)
    _ReadRequest.ReadRequestAddTrackId(b, tid)
    _ReadRequest.ReadRequestAddStartTicks(b, start)
    _ReadRequest.ReadRequestAddEndTicks(b, end)
    rr = _ReadRequest.ReadRequestEnd(b)
    _RequestFrame.RequestFrameStart(b)
    _RequestFrame.RequestFrameAddRequestType(b, ControlRequest.ReadRequest)
    _RequestFrame.RequestFrameAddRequest(b, rr)
    rf = _RequestFrame.RequestFrameEnd(b)
    _ControlEnvelope.ControlEnvelopeStart(b)
    _ControlEnvelope.ControlEnvelopeAddFrameType(b, ControlFrame.RequestFrame)
    _ControlEnvelope.ControlEnvelopeAddFrame(b, rf)
    b.Finish(_ControlEnvelope.ControlEnvelopeEnd(b))
    return bytes(b.Output())


def parse_event(frame_bytes):
    """-> (kind, obj) where kind in hello/receipt/read/scan/other."""
    env = ControlEnvelope.GetRootAsControlEnvelope(frame_bytes, 0)
    ft = env.FrameType()
    if ft == ControlFrame.WorkerHello:
        wh = WorkerHello()
        wh.Init(env.Frame().Bytes, env.Frame().Pos)
        return ("hello", wh)
    if ft != ControlFrame.EventFrame:
        return ("other", ft)
    ef = EventFrame()
    ef.Init(env.Frame().Bytes, env.Frame().Pos)
    et = ef.EventType()
    if et == ControlEvent.CommandReceipt:
        r = CommandReceipt()
        r.Init(ef.Event().Bytes, ef.Event().Pos)
        return ("receipt", r)
    if et == ControlEvent.ReadResponse:
        r = ReadResponse()
        r.Init(ef.Event().Bytes, ef.Event().Pos)
        return ("read", r)
    return ("event", et)


def parse_telemetry(frame_bytes):
    env = TelemetryFrame.GetRootAsTelemetryFrame(frame_bytes, 0)
    et = env.EventType()
    if et == TelemetryEvent.ClockSnapshot:
        c = ClockSnapshot()
        c.Init(env.Event().Bytes, env.Event().Pos)
        return ("clock", c)
    if et == TelemetryEvent.SaveResultEvent:
        s = SaveResultEvent()
        s.Init(env.Event().Bytes, env.Event().Pos)
        return ("save", s)
    if et == TelemetryEvent.TransportAck:
        return ("ack", et)
    return ("tele", et)


class Worker:
    """One void-engine process bound to supervisor control+telemetry sockets."""

    def __init__(self, binary, token=None, instance=None, log_path=None):
        self.binary = binary
        self.token = token or "tok-f1"
        self.instance = instance or "inst-f1-1"
        self.log_path = log_path
        self.tmp = tempfile.mkdtemp(prefix="void-f1-sock-")
        self.proc = None
        self.ctrl = None
        self.tele = None
        self.hello = None

    def start(self):
        ctrl_path = os.path.join(self.tmp, "control.sock")
        tele_path = os.path.join(self.tmp, "tele.sock")
        self._li = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self._li.bind(ctrl_path)
        self._li.listen(1)
        self._tli = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self._tli.bind(tele_path)
        self._tli.listen(1)
        env = dict(os.environ)
        env["VOID_CONTROL_SOCK"] = ctrl_path
        env["VOID_TELEMETRY_SOCK"] = tele_path
        env["VOID_WORKER_TOKEN"] = self.token
        env["VOID_WORKER_INSTANCE_ID"] = self.instance
        out = open(self.log_path, "wb") if self.log_path else subprocess.PIPE
        self._logfh = out if self.log_path else None
        self.proc = subprocess.Popen([self.binary], env=env,
                                     stdout=out, stderr=subprocess.STDOUT)
        self._li.settimeout(20)
        self.ctrl, _ = self._li.accept()
        self._tli.settimeout(20)
        self.tele, _ = self._tli.accept()
        kind, wh = parse_event(recv_frame(self.ctrl))
        if kind != "hello":
            raise RuntimeError(f"first frame not WorkerHello: {kind}")
        self.hello = wh
        return wh

    def command(self, op_name, fill, cid, project_id, expected_rev):
        send_frame(self.ctrl,
                   build_command(op_name, fill, cid, project_id, expected_rev))
        deadline = time.time() + 20
        while time.time() < deadline:
            kind, obj = parse_event(recv_frame(self.ctrl, 20))
            if kind == "receipt" and obj.CommandId().decode() == cid:
                msg = obj.Message()
                return {
                    "status": obj.Status(), "error": obj.Error(),
                    "revision": obj.Revision(), "epoch": obj.EngineEpoch(),
                    "message": msg.decode() if msg else "",
                }
        raise TimeoutError(f"receipt {cid}")

    def transport(self, op, req_id, **kw):
        send_frame(self.ctrl, build_transport(op, req_id, **kw))

    def read(self, view, req_id, **kw):
        send_frame(self.ctrl, build_read(view, req_id, **kw))
        deadline = time.time() + 20
        while time.time() < deadline:
            kind, obj = parse_event(recv_frame(self.ctrl, 20))
            if kind == "read" and obj.RequestId().decode() == req_id:
                items = {}
                for i in range(obj.ItemsLength()):
                    it = obj.Items(i)
                    raw = it.SummaryJson()
                    try:
                        summ = json.loads(raw.decode()) if raw else {}
                    except Exception:
                        summ = {"_raw": raw.decode() if raw else ""}
                    items[it.ObjectId().decode()] = summ
                nc = obj.NextCursor()
                return {
                    "items": items, "done": obj.Done(),
                    "next_cursor": nc.decode() if nc else "",
                    "error": obj.Error(), "revision": obj.Revision(),
                }
        raise TimeoutError(f"read {req_id}")

    def read_all(self, view, prefix, **kw):
        """Page through a view until done."""
        out = {}
        cursor = ""
        page = 0
        while True:
            r = self.read(view, f"{prefix}-p{page}", cursor=cursor, **kw)
            out.update(r["items"])
            if r["done"] or not r["next_cursor"]:
                break
            cursor = r["next_cursor"]
            page += 1
        return out

    def tele_recv(self, timeout=10):
        return parse_telemetry(recv_frame(self.tele, timeout))

    def rss_kb(self):
        try:
            with open(f"/proc/{self.proc.pid}/status") as f:
                for line in f:
                    if line.startswith("VmRSS:"):
                        return int(line.split()[1])
        except FileNotFoundError:
            return -1
        return -1

    def kill(self, sig="KILL"):
        if self.proc and self.proc.poll() is None:
            self.proc.kill() if sig == "KILL" else self.proc.terminate()
        try:
            self.proc.wait(timeout=8)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=8)
        if self._logfh:
            self._logfh.close()
        return self.proc.returncode
