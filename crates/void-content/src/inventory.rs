//! Stock-inventory descriptor model (W19 / SND-06).
//!
//! `content/inventory.json` is the authored descriptor set for VOID's
//! stock tools — one entry per planned processor/instrument/pack, keyed
//! to the STOCK-nn family rows in FEATURE_MAP.md. Descriptors carry ids,
//! musical-outcome labels and source/rights fields; they are *inventory
//! truth*, not a claim the DSP exists (that is `status`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::error::{ContentError, Result};
use crate::manifest::is_sha256_hex;

pub const INVENTORY_SCHEMA: &str = "void-stock-inventory/1";

/// The STOCK-nn family rows this inventory may cover (W19-tasked rows
/// of FEATURE_MAP's stock table; others belong to other lanes' scope).
pub const KNOWN_STOCK_IDS: &[&str] = &[
    "STOCK-01", "STOCK-02", "STOCK-03", "STOCK-04", "STOCK-05", "STOCK-06", "STOCK-07", "STOCK-08",
    "STOCK-09", "STOCK-10", "STOCK-11", "STOCK-12", "STOCK-13", "STOCK-14", "STOCK-15", "STOCK-16",
    "STOCK-17", "STOCK-18", "STOCK-19", "STOCK-20", "STOCK-21", "STOCK-22",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Effect,
    Instrument,
    Meter,
    Utility,
    SamplePack,
    PresetPack,
}

/// Provenance of the content/tooling. `licensed` and `commissioned`
/// entries require a rights record in docs/content-rights/ledger.json.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Original Beyon-Digital work — no third-party licence needed.
    Original,
    /// Third-party material under licence (IRs, sample sets).
    Licensed,
    /// Commissioned work where the agreement assigns rights.
    Commissioned,
}

/// How the entry may be distributed with the product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Distribution {
    /// Ships inside the app/installer.
    Bundled,
    /// Ships as a downloadable content pack (SND-06).
    Downloadable,
    /// Imported/captured by the user only — never redistributed.
    UserOnly,
}

/// Rights fields carried by every entry. `agreement_ref` points at the
/// ledger row (docs/content-rights/ledger.json) for licensed or
/// commissioned sources; it must be empty for `original`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rights {
    pub holder: String,
    #[serde(default)]
    pub attribution: Option<String>,
    pub distribution: Distribution,
    #[serde(default)]
    pub agreement_ref: Option<String>,
}

/// Honest state of the entry — no fake "supported" flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    /// Inventory descriptor only; implementation is engine-resident.
    DescriptorOnly,
    /// Blocked on a licence/rights agreement being executed.
    NeedsLicense,
    /// Descriptor + rights complete; DSP/engine implementation pending.
    NeedsDsp,
    /// Implemented and verified (listening + numerical evidence).
    Ready,
}

/// One authored stock descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockEntry {
    /// VOID slug for the tool/pack, e.g. "fx-stereo-delay".
    pub id: String,
    /// STOCK-nn family row this entry serves.
    pub stock: String,
    pub family: String,
    pub kind: EntryKind,
    pub name: String,
    /// Musical-outcome label (acceptance is musical coverage, not a
    /// copied competitor preset or UI).
    pub outcome: String,
    pub source: Source,
    /// Licence identifier; "VOID-ORIG" for original work.
    pub license_id: String,
    pub rights: Rights,
    /// Comparison-inventory names this entry covers (outcome parity).
    #[serde(default)]
    pub coverage_of: Vec<String>,
    pub status: EntryStatus,
    /// Sample-pack/IR bytes budget in decimal-u64 string, when the
    /// entry is a pack with a declared budget. Optional elsewhere.
    #[serde(default)]
    pub cache_budget_bytes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockInventory {
    pub schema: String,
    pub entries: Vec<StockEntry>,
}

impl StockInventory {
    pub fn parse(json: &str) -> Result<Self> {
        let inv: Self = serde_json::from_str(json)?;
        inv.validate()?;
        Ok(inv)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != INVENTORY_SCHEMA {
            return Err(ContentError::InvalidManifest(format!(
                "inventory schema must be {INVENTORY_SCHEMA}, got {}",
                self.schema
            )));
        }
        let mut ids = BTreeSet::new();
        for e in &self.entries {
            e.validate()?;
            if !ids.insert(e.id.as_str()) {
                return Err(ContentError::InvalidManifest(format!(
                    "duplicate inventory id {}",
                    e.id
                )));
            }
        }
        Ok(())
    }

    pub fn for_stock<'a>(&'a self, stock: &'a str) -> impl Iterator<Item = &'a StockEntry> {
        self.entries.iter().filter(move |e| e.stock == stock)
    }
}

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

impl StockEntry {
    pub fn validate(&self) -> Result<()> {
        if !is_slug(&self.id) {
            return Err(ContentError::InvalidManifest(format!(
                "entry id must be a slug, got {:?}",
                self.id
            )));
        }
        if !KNOWN_STOCK_IDS.contains(&self.stock.as_str()) {
            return Err(ContentError::InvalidManifest(format!(
                "unknown stock family {}",
                self.stock
            )));
        }
        if self.family.is_empty() || self.name.is_empty() || self.outcome.is_empty() {
            return Err(ContentError::InvalidManifest(format!(
                "entry {} needs family, name and outcome",
                self.id
            )));
        }
        if self.license_id.is_empty() {
            return Err(ContentError::InvalidManifest(format!(
                "entry {} needs a license_id",
                self.id
            )));
        }
        match self.source {
            Source::Original => {
                if self.license_id != "VOID-ORIG" {
                    return Err(ContentError::InvalidManifest(format!(
                        "original entry {} must use license_id VOID-ORIG, got {}",
                        self.id, self.license_id
                    )));
                }
                if self.rights.agreement_ref.is_some() {
                    return Err(ContentError::InvalidManifest(format!(
                        "original entry {} cannot carry an agreement_ref",
                        self.id
                    )));
                }
            }
            Source::Licensed | Source::Commissioned => {
                if self.license_id == "VOID-ORIG" || self.license_id.is_empty() {
                    return Err(ContentError::InvalidManifest(format!(
                        "non-original entry {} needs a real license_id",
                        self.id
                    )));
                }
                if self.rights.agreement_ref.is_none() {
                    return Err(ContentError::InvalidManifest(format!(
                        "entry {} is {:?} and needs an agreement_ref into the ledger",
                        self.id, self.source
                    )));
                }
            }
        }
        if self.rights.holder.is_empty() {
            return Err(ContentError::InvalidManifest(format!(
                "entry {} needs rights.holder",
                self.id
            )));
        }
        if let Some(b) = &self.cache_budget_bytes {
            if b.parse::<u64>().is_err() {
                return Err(ContentError::InvalidManifest(format!(
                    "entry {} cache_budget_bytes must be decimal u64",
                    self.id
                )));
            }
        }
        Ok(())
    }
}

/// Rights-ledger row (docs/content-rights/ledger.json): the executed-
/// agreement record a non-original entry's `agreement_ref` points at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseRow {
    /// Ledger id; entries reference this via rights.agreement_ref.
    pub id: String,
    /// Licence identifier carried on the inventory entry.
    pub license_id: String,
    /// Licensor / counterparty name.
    pub licensor: String,
    /// Human summary of granted rights (bundling, redistribution).
    pub grant: String,
    /// Status of the agreement — "required" until executed; never lie.
    pub status: LicenseStatus,
    /// Territory / usage bounds, free text.
    #[serde(default)]
    pub scope: Option<String>,
    /// SHA-256 of the executed agreement artifact when it exists.
    #[serde(default)]
    pub agreement_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    /// Identified licence need; no agreement executed yet.
    Required,
    /// Agreement executed; agreement_sha256 must be set.
    Executed,
    /// Previously granted, now revoked/expired — entry must not ship.
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseLedger {
    pub schema: String,
    pub rows: Vec<LicenseRow>,
}

impl LicenseLedger {
    pub const SCHEMA: &'static str = "void-license-ledger/1";

    pub fn parse(json: &str) -> Result<Self> {
        let l: Self = serde_json::from_str(json)?;
        l.validate()?;
        Ok(l)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != Self::SCHEMA {
            return Err(ContentError::InvalidManifest(format!(
                "ledger schema must be {}, got {}",
                Self::SCHEMA,
                self.schema
            )));
        }
        let mut ids = BTreeSet::new();
        for r in &self.rows {
            if r.id.is_empty()
                || r.license_id.is_empty()
                || r.licensor.is_empty()
                || r.grant.is_empty()
            {
                return Err(ContentError::InvalidManifest(
                    "ledger row needs id, license_id, licensor, grant".into(),
                ));
            }
            if !ids.insert(r.id.as_str()) {
                return Err(ContentError::InvalidManifest(format!(
                    "duplicate ledger id {}",
                    r.id
                )));
            }
            if r.status == LicenseStatus::Executed && r.agreement_sha256.is_none() {
                return Err(ContentError::InvalidManifest(format!(
                    "executed ledger row {} needs agreement_sha256",
                    r.id
                )));
            }
            if let Some(sha) = &r.agreement_sha256 {
                if !is_sha256_hex(sha) {
                    return Err(ContentError::InvalidManifest(format!(
                        "ledger row {} agreement_sha256 must be sha256 hex",
                        r.id
                    )));
                }
            }
        }
        Ok(())
    }

    /// Cross-check: every non-original inventory entry's agreement_ref
    /// resolves to a ledger row whose status is not Revoked.
    pub fn check_against(&self, inv: &StockInventory) -> Result<()> {
        let rows: std::collections::BTreeMap<&str, &LicenseRow> =
            self.rows.iter().map(|r| (r.id.as_str(), r)).collect();
        for e in &inv.entries {
            let Some(aref) = &e.rights.agreement_ref else {
                continue;
            };
            let row = rows.get(aref.as_str()).ok_or_else(|| {
                ContentError::InvalidManifest(format!(
                    "entry {} references missing ledger row {}",
                    e.id, aref
                ))
            })?;
            if row.license_id != e.license_id {
                return Err(ContentError::InvalidManifest(format!(
                    "entry {} license {} != ledger row {} license {}",
                    e.id, e.license_id, row.id, row.license_id
                )));
            }
            if row.status == LicenseStatus::Revoked {
                return Err(ContentError::InvalidManifest(format!(
                    "entry {} relies on revoked licence row {}",
                    e.id, row.id
                )));
            }
        }
        Ok(())
    }
}
