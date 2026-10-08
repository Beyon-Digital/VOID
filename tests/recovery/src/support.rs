//! Fixture builders shared across the T17–T21 binaries.

use std::io::Write;
use std::path::{Path, PathBuf};
use void_assets::AssetStore;
use void_project::{
    AppState, PersistedHistory, ProjectMeta, ReceiptLog, ReceiptRecord, SaveReceipt, SaveRequest,
    SaveSession, SnapshotBundle, TransactionRecord,
};

pub const ENGINE_REV: &str =
    "tracktion-engine@e7607547293fac421e1a9ec9898410c1893f570e+mock-snapshot";

/// A real `.void` container on disk in a temp dir.
pub struct TempProject {
    pub _dir: tempfile::TempDir,
    pub root: PathBuf,
    pub project_id: String,
}

impl Default for TempProject {
    fn default() -> Self {
        Self::new()
    }
}

impl TempProject {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("song.void");
        std::fs::create_dir_all(&root).unwrap();
        let project_id = uuid::Uuid::new_v4().to_string();
        let meta = ProjectMeta::new(project_id.clone(), "recovery-fixture".into());
        void_project::create(&root, &meta).expect("create container");
        Self {
            _dir: dir,
            root,
            project_id,
        }
    }

    pub fn assets(&self) -> AssetStore {
        AssetStore::new(
            self.root.join("assets"),
            void_assets::ASSET_IMPORT_MAX_BYTES,
        )
        .expect("asset store")
    }

    /// A complete snapshot bundle at a revision — engine bytes, app state
    /// (with history/provenance), receipts, asset hashes.
    pub fn bundle(&self, revision: u64, asset_hashes: Vec<String>) -> SnapshotBundle {
        let mut history = PersistedHistory::default();
        for i in 1..=revision {
            history.record(TransactionRecord {
                transaction_id: uuid::Uuid::new_v4().to_string(),
                revision: i.to_string(),
                label: format!("edit #{i}"),
                engine_undo_token: format!("engine-undo-{i:04x}"),
                app_changes: serde_json::json!({"cueColor": i}),
            });
        }
        let app = AppState {
            project_id: self.project_id.clone(),
            revision: revision.to_string(),
            visual: serde_json::json!({"zoomPpq": 96, "selectedTrack": "track-1"}),
            history,
            alternatives: vec![serde_json::json!({"alt": "take-2", "rating": 4})],
        };
        SnapshotBundle {
            engine_snapshot: format!("TRACKTION_EDIT_SNAPSHOT rev={revision}").into_bytes(),
            app_state: app.serialize().unwrap(),
            command_receipts: receipts_bytes(&self.project_id, revision),
            asset_hashes,
        }
    }

    pub fn save_at(&self, revision: u64) -> SaveReceipt {
        self.save_with_parent(revision, None)
    }

    /// Save whose manifest records a parent checkpoint — real lineage.
    pub fn save_with_parent(&self, revision: u64, parent: Option<String>) -> SaveReceipt {
        let bundle = self.bundle(revision, vec![]);
        void_project::save(
            &self.root,
            SaveRequest {
                project_id: self.project_id.clone(),
                revision,
                engine_revision: ENGINE_REV.into(),
                parent_checkpoint_id: parent,
            },
            &bundle,
        )
        .expect("save")
    }

    /// A session with a deterministic checkpoint id for failpoint tests.
    pub fn session(
        &self,
        revision: u64,
        checkpoint_id: &str,
        parent: Option<String>,
    ) -> SaveSession {
        SaveSession::begin_with_id(
            &self.root,
            SaveRequest {
                project_id: self.project_id.clone(),
                revision,
                engine_revision: ENGINE_REV.into(),
                parent_checkpoint_id: parent,
            },
            checkpoint_id.to_string(),
        )
        .expect("begin session")
    }
}

pub fn receipts_bytes(project_id: &str, upto_revision: u64) -> Vec<u8> {
    let log = ReceiptLog {
        format_version: 1,
        project_id: project_id.into(),
        upto_revision: upto_revision.to_string(),
        receipts: (1..=upto_revision)
            .map(|i| ReceiptRecord {
                command_id: format!("cmd-{i:04}"),
                payload_hash: void_assets::hex_sha256(format!("payload-{i}").as_bytes()),
                status: "APPLIED".into(),
                revision: i.to_string(),
                engine_epoch: "1".into(),
            })
            .collect(),
    };
    void_project::serialize_log(&log).unwrap()
}

/// Minimal valid WAV body — real RIFF structure, generated not canned.
pub fn wav_bytes(channels: u16, sample_rate: u32, bits: u16, frames: u32) -> Vec<u8> {
    let data_len = (frames as usize) * (channels as usize) * (bits as usize / 8);
    let fmt_len = 16usize;
    let riff_len = 4 + (8 + fmt_len) + (8 + data_len);
    let mut w = Vec::with_capacity(8 + riff_len);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(riff_len as u32).to_le_bytes());
    w.extend_from_slice(b"WAVE");
    w.extend_from_slice(b"fmt ");
    w.extend_from_slice(&(fmt_len as u32).to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&channels.to_le_bytes());
    w.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * channels as u32 * (bits as u32 / 8);
    w.extend_from_slice(&byte_rate.to_le_bytes());
    w.extend_from_slice(&(channels * (bits / 8)).to_le_bytes()); // block align
    w.extend_from_slice(&bits.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data_len as u32).to_le_bytes());
    w.extend(std::iter::repeat_n(0u8, data_len));
    w
}

/// Corrupt WAV variants named by the defect they carry.
pub fn corrupt_wavs() -> Vec<(&'static str, Vec<u8>)> {
    let mut v = Vec::new();
    v.push(("not-riff", b"NOPE".to_vec()));
    v.push(("header-only", wav_bytes(2, 44100, 16, 1)[..16].to_vec()));
    // Truncated: declares a full frame but the data bytes are cut.
    let mut w = wav_bytes(2, 44100, 16, 100);
    w.truncate(w.len() - 200);
    v.push(("truncated-data", w));
    v.push(("zero-channels", wav_bytes(0, 44100, 16, 10)));
    v.push(("insane-channels", wav_bytes(255, 44100, 16, 10)));
    v.push(("zero-rate", wav_bytes(2, 0, 16, 10)));
    v.push(("bad-bits", wav_bytes(2, 44100, 12, 10)));
    // Missing data chunk: header stops before it.
    let mut w = wav_bytes(2, 44100, 16, 10);
    w.truncate(36);
    // keep RIFF size honest so the parser reports the missing chunk
    let riff_len = (w.len() - 8) as u32;
    w[4..8].copy_from_slice(&riff_len.to_le_bytes());
    v.push(("missing-data-chunk", w));
    v
}

/// A zip whose entry table contains every traversal/evil name the
/// importer must reject up front. One archive = one injected attack set.
pub fn evil_zip(path: &Path, extra_bytes: usize) -> Vec<String> {
    use zip::write::{SimpleFileOptions, ZipWriter};
    let f = std::fs::File::create(path).unwrap();
    let mut z = ZipWriter::new(f);
    let opts = SimpleFileOptions::default();
    let mut names = Vec::new();
    for name in [
        "../escape.txt",
        "/absolute.txt",
        "ok/../../sneak.txt",
        "win\\..\\evil.txt",
        "C:/drive.txt",
        "ctrl\u{0001}.txt",
    ] {
        z.start_file(name, opts).unwrap();
        z.write_all(b"x").unwrap();
        names.push(name.to_string());
    }
    // Symlink entry.
    z.add_symlink("linkout", "/etc/passwd", opts.unix_permissions(0o120777))
        .unwrap();
    names.push("linkout".into());
    // Oversized expansion entry.
    if extra_bytes > 0 {
        z.start_file("bomb.bin", opts).unwrap();
        z.write_all(&vec![0u8; extra_bytes]).unwrap();
        names.push("bomb.bin".into());
    }
    z.finish().unwrap();
    names
}

/// A clean zip with real payloads for the positive path.
pub fn good_zip(path: &Path) -> Vec<String> {
    use zip::write::{SimpleFileOptions, ZipWriter};
    let f = std::fs::File::create(path).unwrap();
    let mut z = ZipWriter::new(f);
    let opts = SimpleFileOptions::default();
    z.start_file("samples/kick.wav", opts).unwrap();
    z.write_all(&wav_bytes(2, 44100, 16, 32)).unwrap();
    z.start_file("readme.txt", opts).unwrap();
    z.write_all(b"fixture").unwrap();
    z.finish().unwrap();
    vec!["samples/kick.wav".into(), "readme.txt".into()]
}

/// Seed a recordings/<takeId>/ dir with a journal + chunks (one corrupt).
pub fn seed_recording(root: &Path, take_id: &str, corrupt_tail: bool) -> PathBuf {
    let dir = root.join("recordings").join(take_id);
    std::fs::create_dir_all(&dir).unwrap();
    let c1 = wav_bytes(2, 44100, 16, 64);
    // The journal records the full intended chunk; the file on disk is
    // torn short (crash during finalize) so it must fail verification.
    let c2_full = wav_bytes(2, 44100, 16, 64);
    let c2_disk = if corrupt_tail {
        &c2_full[..c2_full.len() - 10]
    } else {
        &c2_full[..]
    };
    std::fs::write(dir.join("chunk-0.wav"), &c1).unwrap();
    std::fs::write(dir.join("chunk-1.wav"), c2_disk).unwrap();
    let journal = serde_json::json!({
        "takeId": take_id,
        "finalized": !corrupt_tail,
        "chunks": [
            {"path": "chunk-0.wav", "sha256": void_assets::hex_sha256(&c1), "bytes": c1.len().to_string()},
            {"path": "chunk-1.wav", "sha256": void_assets::hex_sha256(&c2_full), "bytes": c2_full.len().to_string()},
        ]
    });
    std::fs::write(
        dir.join("journal.json"),
        serde_json::to_vec_pretty(&journal).unwrap(),
    )
    .unwrap();
    dir
}

pub fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("read fixture")
}
