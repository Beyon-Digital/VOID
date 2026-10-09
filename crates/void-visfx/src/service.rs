//! VisualGenService — declarative scene spec → void-jobs visual-
//! generation job → validated SceneDoc + sha256 assets → accept/
//! reject lifecycle (T85). Mirrors void-proposals' flow (W13):
//! request admits + submits + persists pending; the caller runs via
//! JobRunner; collect folds the RunOutcome into the record; accept
//! revalidates before committing.

use crate::error::{Result, VisFxError};
use crate::record::{
    ArtifactBrief, FailedFacts, PendingFacts, ReadyFacts, RejectedFacts, StaleFacts,
    VisGenProvenance, VisGenRecord, VisGenStatus,
};
use crate::shader::{PresetRef, ShaderRegistry};
use crate::spec::{parse_scene_doc, SceneGenSpec, DOC_FILE};
use crate::store::{utc_now, VisGenStore};
use sha2::Digest;
use std::path::Path;
use void_assets::AssetStore;
use void_jobs::{
    decide, Admit, JobDb, JobKind, JobRecord, JobSpec, JobStatus, Reservations, RunOutcome,
};

/// In-flight cap (mirrors proposals' MAX_PROPOSALS).
pub const MAX_PENDING_VIS: usize = 8;
/// Local symbolic runtime (same contract as W13 — the coordinator
/// resolves the real worker binary at run time).
pub const SYMBOLIC_RUNTIME_ID: &str = "local:visual-gen";

pub struct PendingVis {
    pub record: VisGenRecord,
    pub spec: JobSpec,
}

pub struct VisualGenService {
    pub records: VisGenStore,
    pub registry: ShaderRegistry,
    pub store: AssetStore,
}

/// Revalidation facts the coordinator supplies at accept time —
/// context must still match and the job's assets must still verify
/// (same shape as W13's Revalidation).
#[derive(Debug, Clone)]
pub struct Revalidation {
    pub project_id: String,
    /// Current context hash — stale records can't be accepted.
    pub current_context_sha256: String,
    /// Layer names still present in the target program channel (doc
    /// program layers targeting a missing slot are dropped).
    pub live_layer_names: Vec<String>,
    /// Target preview channel exists.
    pub preview_channel_exists: bool,
}

#[derive(Debug)]
pub struct VisAcceptPlan {
    pub transaction_id: String,
    /// Layer names the accept will create/replace.
    pub layer_names: Vec<String>,
    /// Doc layer names dropped by revalidation (no live target).
    pub dropped_layers: Vec<String>,
    /// Action count from the doc.
    pub action_count: usize,
    /// Artifact shas that verify in the store.
    pub verified_artifacts: Vec<String>,
}

impl VisualGenService {
    pub fn new(store: AssetStore, project_root: &Path) -> Result<Self> {
        Ok(Self {
            records: VisGenStore::new(project_root)?,
            registry: ShaderRegistry::open(project_root)?,
            store,
        })
    }

    /// Admit + submit + persist a pending record. Returns the record +
    /// drafted JobSpec for the caller to run via JobRunner.
    pub fn request(
        &mut self,
        db: &JobDb,
        project_id: &str,
        revision: u64,
        context_sha256: &str,
        spec_in: SceneGenSpec,
        deadline_monotonic_ns: u64,
    ) -> Result<PendingVis> {
        let _ = deadline_monotonic_ns;
        spec_in.validate()?;
        // cap in-flight
        let pending = self
            .records
            .list_for_project(project_id)?
            .iter()
            .filter(|r| r.status == VisGenStatus::Pending)
            .count();
        if pending >= MAX_PENDING_VIS {
            return Err(VisFxError::Busy(format!(
                "{pending} pending vis-gen records"
            )));
        }

        let record_id = uuid::Uuid::new_v4().to_string();
        let job_id = uuid::Uuid::new_v4().to_string();
        let spec_sha = spec_in.sha256();

        let job = JobSpec {
            job_id: job_id.clone(),
            project_id: project_id.into(),
            source_revision: revision.to_string(),
            context_sha256: context_sha256.into(),
            kind: JobKind::VisualGeneration,
            runtime_id: SYMBOLIC_RUNTIME_ID.into(),
            runtime_sha256: "0".repeat(64),
            model_id: None,
            model_sha256: None,
            inputs: Vec::new(),
            parameters: serde_json::to_value(&spec_in)?,
            reservations: Reservations {
                ram_bytes: (1u64 << 30).to_string(),
                vram_bytes: "0".into(),
                cpu_threads: 2,
            },
            deadline_monotonic_ns: deadline_monotonic_ns.to_string(),
            output_scope_token: format!("vis-{record_id}"),
            cloud_consent_id: None,
        };
        job.validate()?;

        match decide(db, &job)? {
            Admit::StartNow | Admit::Queue => {}
            Admit::Full => {
                return Err(VisFxError::Busy("job queue full".into()));
            }
        }
        db.submit(&job)?;

        let rec = VisGenRecord {
            schema_version: 1,
            record_id,
            project_id: project_id.into(),
            revision: revision.to_string(),
            context_sha256: context_sha256.into(),
            spec_sha256: spec_sha,
            spec: spec_in,
            job_id,
            status: VisGenStatus::Pending,
            pending: Some(PendingFacts {
                submitted_at: utc_now(),
            }),
            ready: None,
            failed: None,
            rejected: None,
            stale: None,
            transaction_id: None,
            supersedes: None,
        };
        self.records.save(&rec)?;
        Ok(PendingVis {
            record: rec,
            spec: job,
        })
    }

    /// After `runner.run(spec)` finishes: fold the outcome into the
    /// record. Succeeded → parse+validate scene.json + verify declared
    /// artifacts + run embedded shader bodies through the registry →
    /// `ready`. Failed/cancelled → `failed`/`stale(cancelled)`. A
    /// rejected shader body fails the generation — no fake success.
    pub fn collect(
        &mut self,
        record_id: &str,
        job: &JobRecord,
        outcome: &RunOutcome,
    ) -> Result<VisGenRecord> {
        let mut rec = self.records.get(record_id)?;
        match outcome {
            RunOutcome::Succeeded {
                artifacts,
                warnings,
                ..
            } if job.status == JobStatus::Succeeded => {
                let collected = (|| -> std::result::Result<(SceneDocOwned, String), VisFxError> {
                    let doc_art =
                        artifacts
                            .iter()
                            .find(|a| a.name == DOC_FILE)
                            .ok_or_else(|| {
                                VisFxError::InvalidDocument(format!("no {DOC_FILE} artifact"))
                            })?;
                    let doc_bytes = std::fs::read(&doc_art.asset_abs)?;
                    let doc = parse_scene_doc(&doc_bytes)?;
                    Ok((
                        SceneDocOwned {
                            doc,
                            sha: crate::spec::hex(&sha2::Sha256::digest(&doc_bytes)),
                            asset_rel: doc_art.asset_rel.clone(),
                        },
                        doc_art.sha256.clone(),
                    ))
                })();
                let (doc_owned, doc_sha) = match collected {
                    Ok(v) => v,
                    Err(e) => {
                        rec.failed = Some(FailedFacts {
                            at: utc_now(),
                            code: "invalid_document".into(),
                            message: e.to_string(),
                        });
                        rec.transition(VisGenStatus::Failed)?;
                        self.records.save(&rec)?;
                        return Ok(rec);
                    }
                };
                let doc = doc_owned.doc;

                // declared media artifacts must be produced artifacts.
                if let Some(bad) = doc.layers.iter().find_map(|l| {
                    l.media.as_ref().and_then(|m| {
                        (!artifacts.iter().any(|a| a.name == m.artifact))
                            .then(|| m.artifact.clone())
                    })
                }) {
                    rec.failed = Some(FailedFacts {
                        at: utc_now(),
                        code: "invalid_document".into(),
                        message: format!("artifact {bad:?} not produced by job"),
                    });
                    rec.transition(VisGenStatus::Failed)?;
                    self.records.save(&rec)?;
                    return Ok(rec);
                }

                // embedded shader bodies → registry pipeline (T84 seam)
                for layer in &doc.layers {
                    if let Some(g) = &layer.generator {
                        if let Some(body) = &g.shader_body {
                            let rep = self
                                .registry
                                .compile_body(&format!("scene:{}", rec.record_id), body);
                            if !rep.ok() {
                                rec.failed = Some(FailedFacts {
                                    at: utc_now(),
                                    code: "shader_rejected".into(),
                                    message: format!(
                                        "layer {:?} shader failed {}: {}",
                                        layer.name,
                                        rep.stage.label(),
                                        rep.detail.unwrap_or_default()
                                    ),
                                });
                                rec.transition(VisGenStatus::Failed)?;
                                self.records.save(&rec)?;
                                return Ok(rec);
                            }
                        }
                        if let Some(p) = &g.preset {
                            if void_visual::shaders::generator_body(p).is_none() {
                                rec.failed = Some(FailedFacts {
                                    at: utc_now(),
                                    code: "unknown_preset".into(),
                                    message: format!("layer {:?} preset {p:?}", layer.name),
                                });
                                rec.transition(VisGenStatus::Failed)?;
                                self.records.save(&rec)?;
                                return Ok(rec);
                            }
                        }
                    }
                }

                rec.ready = Some(ReadyFacts {
                    finished_at: utc_now(),
                    scene_doc_sha256: doc_owned.sha,
                    artifacts: artifacts
                        .iter()
                        .map(|a| ArtifactBrief {
                            filename: a.name.clone(),
                            sha256: a.sha256.clone(),
                            bytes: a.bytes,
                            asset_rel: a.asset_rel.clone(),
                        })
                        .collect(),
                    provenance: VisGenProvenance {
                        job_id: job.spec.job_id.clone(),
                        generator_id: doc.generator_id.clone(),
                        generator_version: doc.generator_version.clone(),
                        model_id: doc.model_id.clone(),
                        runtime_id: job.spec.runtime_id.clone(),
                        runtime_sha256: job.spec.runtime_sha256.clone(),
                        seed: doc.seed.clone(),
                        document_sha256: doc_sha,
                        analysis: doc.analysis.clone(),
                        document_asset: doc_owned.asset_rel,
                    },
                    scene: doc,
                });
                if !warnings.is_empty() {
                    rec.failed = Some(FailedFacts {
                        at: utc_now(),
                        code: "warnings".into(),
                        message: warnings.join("; "),
                    });
                }
                rec.transition(VisGenStatus::Ready)?;
            }
            RunOutcome::Cancelled { .. } => {
                rec.transition(VisGenStatus::Stale)?;
                rec.stale = Some(StaleFacts {
                    at: utc_now(),
                    cause: "job_cancelled".into(),
                });
            }
            RunOutcome::Failed { error, .. } => {
                rec.transition(VisGenStatus::Failed)?;
                rec.failed = Some(FailedFacts {
                    at: utc_now(),
                    code: "job_failed".into(),
                    message: error.clone(),
                });
            }
            _ => {
                rec.transition(VisGenStatus::Failed)?;
                rec.failed = Some(FailedFacts {
                    at: utc_now(),
                    code: "job_failed".into(),
                    message: "job did not reach succeeded".into(),
                });
            }
        }
        self.records.save(&rec)?;
        Ok(rec)
    }

    /// Plan an accept — revalidates context + targets + artifact
    /// presence before producing the (transaction, drop) plan.
    pub fn plan_accept(&self, record_id: &str, reval: &Revalidation) -> Result<VisAcceptPlan> {
        let rec = self.records.get(record_id)?;
        if rec.status != VisGenStatus::Ready {
            return Err(VisFxError::InvalidTransition {
                from: rec.status.label(),
                to: "accepted",
            });
        }
        if rec.project_id != reval.project_id {
            return Err(VisFxError::Revalidation("project changed".into()));
        }
        if rec.context_sha256 != reval.current_context_sha256 {
            return Err(VisFxError::Stale("context changed — re-request".into()));
        }
        if !reval.preview_channel_exists {
            return Err(VisFxError::Revalidation("preview channel missing".into()));
        }
        let ready = rec.ready.as_ref().expect("ready facts");
        let mut dropped = Vec::new();
        let mut names = Vec::new();
        for l in &ready.scene.layers {
            if l.channel == "program" && !reval.live_layer_names.contains(&l.name) {
                dropped.push(l.name.clone());
            } else {
                names.push(l.name.clone());
            }
        }
        let mut verified = Vec::new();
        for a in &ready.artifacts {
            if self.store.find(&a.sha256).is_some() {
                verified.push(a.sha256.clone());
            }
        }
        Ok(VisAcceptPlan {
            transaction_id: uuid::Uuid::new_v4().to_string(),
            layer_names: names,
            dropped_layers: dropped,
            action_count: ready.scene.actions.len(),
            verified_artifacts: verified,
        })
    }

    /// Commit an accepted record — the coordinator applies the
    /// transaction itself (one transaction per gesture).
    pub fn commit_accepted(
        &mut self,
        record_id: &str,
        transaction_id: &str,
    ) -> Result<VisGenRecord> {
        let mut rec = self.records.get(record_id)?;
        rec.transaction_id = Some(transaction_id.into());
        rec.transition(VisGenStatus::Accepted)?;
        self.records.save(&rec)?;
        Ok(rec)
    }

    pub fn reject(&mut self, record_id: &str, reason: Option<String>) -> Result<VisGenRecord> {
        let mut rec = self.records.get(record_id)?;
        rec.rejected = Some(RejectedFacts {
            at: utc_now(),
            reason,
        });
        rec.transition(VisGenStatus::Rejected)?;
        self.records.save(&rec)?;
        Ok(rec)
    }

    /// Mark every pending/ready record of a project stale (context
    /// change, project edit, job cancel).
    pub fn mark_stale(&mut self, project_id: &str, cause: &str) -> Result<usize> {
        let mut n = 0;
        for mut rec in self.records.list_for_project(project_id)? {
            if matches!(rec.status, VisGenStatus::Pending | VisGenStatus::Ready) {
                rec.stale = Some(StaleFacts {
                    at: utc_now(),
                    cause: cause.into(),
                });
                rec.transition(VisGenStatus::Stale)?;
                self.records.save(&rec)?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Preset resolution convenience for the studio/coordinator.
    pub fn resolve_preset(&mut self, req: &PresetRef) -> crate::shader::ResolvedShader {
        self.registry.resolve_or_fallback(req)
    }
}

struct SceneDocOwned {
    doc: crate::spec::SceneDoc,
    sha: String,
    asset_rel: String,
}
