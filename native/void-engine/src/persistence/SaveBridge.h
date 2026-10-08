// Minimal checkpoint write for F0: engine.tracktionedit + app-state.json +
// receipts + manifest.json + CURRENT pointer swap, inside the container dir.
// The full W05 state machine (staging quarantine, lineage, DB reconcile) is
// owned by the persistence lane; this never claims more than a verified
// file-complete checkpoint on the local filesystem.
#pragma once
