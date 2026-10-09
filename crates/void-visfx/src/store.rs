//! VisGenStore — atomic per-record persistence, same layout contract
//! as void-proposals' store (`<dir>/<record_id>/record.json` via
//! tmp+rename).

use crate::error::{Result, VisFxError};
use crate::record::VisGenRecord;
use std::fs;
use std::path::{Path, PathBuf};

pub struct VisGenStore {
    dir: PathBuf,
}

impl VisGenStore {
    pub fn new(container: &Path) -> Result<Self> {
        let dir = container.join("visfx").join("generations");
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn save(&self, rec: &VisGenRecord) -> Result<()> {
        let d = self.dir.join(&rec.record_id);
        fs::create_dir_all(&d)?;
        let final_p = d.join("record.json");
        let tmp_p = d.join("record.json.tmp");
        fs::write(&tmp_p, serde_json::to_vec_pretty(rec)?)?;
        fs::rename(&tmp_p, &final_p)?;
        Ok(())
    }

    pub fn get(&self, record_id: &str) -> Result<VisGenRecord> {
        let p = self.dir.join(record_id).join("record.json");
        if !p.exists() {
            return Err(VisFxError::NotFound(record_id.into()));
        }
        Ok(serde_json::from_slice(&fs::read(&p)?)?)
    }

    pub fn list_for_project(&self, project_id: &str) -> Result<Vec<VisGenRecord>> {
        let mut out = Vec::new();
        for ent in fs::read_dir(&self.dir)? {
            let p = ent?.path().join("record.json");
            if p.exists() {
                let rec: VisGenRecord = serde_json::from_slice(&fs::read(&p)?)?;
                if rec.project_id == project_id {
                    out.push(rec);
                }
            }
        }
        Ok(out)
    }
}

/// RFC3339 UTC — duplicated from void-proposals store (small, stable).
pub fn utc_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // civil-from-days
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}
