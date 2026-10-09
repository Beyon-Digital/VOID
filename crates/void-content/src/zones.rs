//! Multisample zone model (W19 / SND-03, T73).
//!
//! A `Multisample` is a set of `Zone`s: each zone owns a key range,
//! velocity range and layer, and references one payload file (by
//! pack-relative path + sha256). Zones sharing a `round_robin` group id
//! rotate — the rotation is *deterministic seeded*: the same seed and
//! the same trigger sequence produce the same zone order, so renders
//! and replays are reproducible (CONTRACTS §1 determinism spirit).
//!
//! The model here is the policy layer — where a voice goes and which
//! sample it uses. The audio callback that renders it is engine-side
//! (docs/content-rights/NEEDS.md).

use crate::error::{ContentError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One playable zone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Zone {
    /// Stable zone id inside the instrument.
    pub zone_id: String,
    /// Inclusive MIDI key range [lo, hi], 0..=127.
    pub key_lo: u8,
    pub key_hi: u8,
    /// Inclusive velocity range [lo, hi], 0..=127.
    pub vel_lo: u8,
    pub vel_hi: u8,
    /// Layer this zone belongs to (0 = main). Selectors can restrict a
    /// trigger to one layer (articulation) or search all layers.
    pub layer: u16,
    /// Root key — the pitch the sample was recorded at.
    pub root_key: u8,
    /// Playback tuning offset in cents (i16, not float — exact wire data).
    #[serde(default)]
    pub tune_cents: i16,
    /// Zone gain in hundredths of dB (i16 — exact, no float drift).
    #[serde(default)]
    pub gain_cdb: i16,
    /// Pack-relative path of the sample file.
    pub file: String,
    /// Expected sha256 of that file (relink/integrity anchor).
    pub file_sha256: String,
    /// Zones sharing a group id rotate round-robin per trigger.
    #[serde(default)]
    pub round_robin: Option<String>,
    /// Sample loop mode.
    #[serde(default)]
    pub loop_mode: LoopMode,
}

impl Zone {
    pub fn validate(&self) -> Result<()> {
        let bad = |m: &str| ContentError::InvalidZone(m.into());
        if self.zone_id.is_empty() || self.zone_id.len() > 120 {
            return Err(bad("zoneId missing/oversized"));
        }
        if self.key_lo > self.key_hi || self.key_hi > 127 {
            return Err(bad("key range invalid"));
        }
        if self.vel_lo > self.vel_hi || self.vel_hi > 127 {
            return Err(bad("velocity range invalid"));
        }
        if self.root_key > 127 {
            return Err(bad("rootKey out of range"));
        }
        if !crate::manifest::is_safe_pack_path(&self.file) {
            return Err(ContentError::UnsafePath(self.file.clone()));
        }
        if !crate::manifest::is_sha256_hex(&self.file_sha256) {
            return Err(bad("fileSha256 not sha256 hex"));
        }
        Ok(())
    }

    pub fn covers(&self, key: u8, velocity: u8) -> bool {
        key >= self.key_lo
            && key <= self.key_hi
            && velocity >= self.vel_lo
            && velocity <= self.vel_hi
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopMode {
    #[default]
    Off,
    Forward,
    /// Loop region is engine data; the model only records it exists.
    Alternate,
}

/// Deterministic seeded rotation for one round-robin group.
///
/// The base order is the zone list order permuted once by a seeded
/// xorshift64* shuffle (same construction family as workers/symbolic —
/// deterministic across platforms and runs). Each trigger then takes the
/// next element of the permuted order — a counter, not a dice roll, so
/// every zone is visited before any repeats.
#[derive(Debug, Clone)]
pub struct RoundRobin {
    order: Vec<usize>,
    next: usize,
}

/// xorshift64* — tiny deterministic PRNG used for seeded shuffles.
fn xs64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *state = x;
    x.wrapping_mul(0x2545F4914F6CDD1D)
}

impl RoundRobin {
    /// Build the rotation for `n` members under `seed`.
    pub fn new(n: usize, seed: u64) -> Self {
        let mut order: Vec<usize> = (0..n).collect();
        if n > 1 {
            // Fisher–Yates with xorshift64*; seed 0 is legal but folded
            // so 0 doesn't produce the identity-prone zero state.
            let mut st = seed.wrapping_add(0x9E3779B97F4A7C15);
            for i in (1..n).rev() {
                let j = (xs64(&mut st) % ((i + 1) as u64)) as usize;
                order.swap(i, j);
            }
        }
        Self { order, next: 0 }
    }

    /// Next member index (rotates forever).
    pub fn take(&mut self) -> Option<usize> {
        if self.order.is_empty() {
            return None;
        }
        let idx = self.order[self.next];
        self.next = (self.next + 1) % self.order.len();
        Some(idx)
    }

    /// Deterministic position inspection for tests/diagnostics.
    pub fn order(&self) -> &[usize] {
        &self.order
    }
}

/// Which layers a trigger may draw from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerSelector {
    /// Only this layer id.
    Only(u16),
    /// Any layer (articulation not asserted).
    Any,
}

/// A zone set with deterministic round-robin state.
#[derive(Debug, Clone)]
pub struct Multisample {
    zones: Vec<Zone>,
    /// group id -> rotation state, built lazily per encountered group.
    rotations: HashMap<String, RoundRobin>,
    seed: u64,
}

impl Multisample {
    pub fn new(zones: Vec<Zone>, seed: u64) -> Result<Self> {
        for z in &zones {
            z.validate()?;
        }
        Ok(Self {
            zones,
            rotations: HashMap::new(),
            seed,
        })
    }

    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }

    /// Resolve a trigger to a zone: candidates are zones covering
    /// (key, velocity) and matching the layer selector, taken in
    /// zone-list order. The first candidate that belongs to a
    /// round-robin group rotates inside *its* group — group members
    /// share identical coverage (enforced by `validate_groups`), so the
    /// whole group is always a candidate when one is. A leading
    /// ungrouped candidate wins outright.
    pub fn select(&mut self, key: u8, velocity: u8, layer: LayerSelector) -> Option<&Zone> {
        let mut first: Option<usize> = None;
        let mut first_group: Option<String> = None;
        for (i, z) in self.zones.iter().enumerate() {
            if !z.covers(key, velocity) {
                continue;
            }
            match layer {
                LayerSelector::Only(l) if z.layer != l => continue,
                _ => {}
            }
            first = Some(i);
            first_group = z.round_robin.clone();
            break;
        }
        match first_group {
            None => first.map(|i| &self.zones[i]),
            Some(g) => {
                let members: Vec<usize> = self
                    .zones
                    .iter()
                    .enumerate()
                    .filter(|(_, z)| z.round_robin.as_deref() == Some(g.as_str()))
                    .map(|(i, _)| i)
                    .collect();
                if members.len() <= 1 {
                    return members.first().map(|i| &self.zones[*i]);
                }
                // Seed per group: base seed folded with a stable hash of
                // the group id so different groups don't share a phase.
                let gseed = self.seed ^ fnv1a(g.as_bytes());
                let rot = self
                    .rotations
                    .entry(g.clone())
                    .or_insert_with(|| RoundRobin::new(members.len(), gseed));
                rot.take().map(|k| &self.zones[members[k]])
            }
        }
    }

    /// Structural invariant: every zone in one round-robin group must
    /// cover the identical key/velocity/layer rectangle — a group is a
    /// same-trigger alternative set, not a partial-overlap shortcut.
    /// Call after construction when loading an instrument definition.
    pub fn validate_groups(&self) -> Result<()> {
        let mut groups: HashMap<&str, &Zone> = HashMap::new();
        for z in &self.zones {
            let Some(g) = z.round_robin.as_deref() else {
                continue;
            };
            match groups.get(g) {
                None => {
                    groups.insert(g, z);
                }
                Some(first) => {
                    let same = first.key_lo == z.key_lo
                        && first.key_hi == z.key_hi
                        && first.vel_lo == z.vel_lo
                        && first.vel_hi == z.vel_hi
                        && first.layer == z.layer;
                    if !same {
                        return Err(ContentError::InvalidZone(format!(
                            "round-robin group {g:?} has members with different coverage"
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

/// FNV-1a 64 — stable string hash for group seeding.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(id: &str, rr: Option<&str>) -> Zone {
        Zone {
            zone_id: id.into(),
            key_lo: 36,
            key_hi: 36,
            vel_lo: 0,
            vel_hi: 127,
            layer: 0,
            root_key: 36,
            tune_cents: 0,
            gain_cdb: 0,
            file: format!("samples/{id}.wav"),
            file_sha256: void_assets::hex_sha256(id.as_bytes()),
            round_robin: rr.map(|s| s.into()),
            loop_mode: LoopMode::Off,
        }
    }

    #[test]
    fn round_robin_is_deterministic_per_seed() {
        let a = RoundRobin::new(4, 42);
        let b = RoundRobin::new(4, 42);
        assert_eq!(a.order(), b.order());
        let c = RoundRobin::new(4, 7);
        // Different seed very likely permutes differently; not asserted
        // as inequality (could coincide) — determinism is the contract.
        assert_eq!(c.order().len(), 4);
    }

    #[test]
    fn round_robin_visits_all_before_repeating() {
        let mut rr = RoundRobin::new(3, 99);
        let mut seen = [false; 3];
        for _ in 0..3 {
            seen[rr.take().unwrap()] = true;
        }
        assert!(seen.iter().all(|s| *s));
    }

    #[test]
    fn select_rotates_within_group() {
        let zones = vec![
            zone("a", Some("g")),
            zone("b", Some("g")),
            zone("c", Some("g")),
        ];
        let mut ms = Multisample::new(zones, 7).unwrap();
        let picks: Vec<String> = (0..6)
            .map(|_| {
                ms.select(36, 100, LayerSelector::Any)
                    .unwrap()
                    .zone_id
                    .clone()
            })
            .collect();
        // Same three zones, deterministic cyclic order, each once per pass.
        let mut first_pass = picks[..3].to_vec();
        first_pass.sort();
        assert_eq!(first_pass, vec!["a", "b", "c"]);
        assert_eq!(picks[..3], picks[3..]);
    }

    #[test]
    fn layer_selector_restricts() {
        let mut z = zone("main", None);
        z.layer = 0;
        let mut alt = zone("alt", None);
        alt.layer = 1;
        let mut ms = Multisample::new(vec![z, alt], 1).unwrap();
        assert_eq!(
            ms.select(36, 100, LayerSelector::Only(1)).unwrap().zone_id,
            "alt"
        );
        assert!(ms.select(36, 100, LayerSelector::Only(9)).is_none());
    }

    #[test]
    fn velocity_ranges_partition() {
        let mut soft = zone("soft", None);
        soft.vel_hi = 63;
        let mut hard = zone("hard", None);
        hard.vel_lo = 64;
        let mut ms = Multisample::new(vec![soft, hard], 1).unwrap();
        assert_eq!(
            ms.select(36, 30, LayerSelector::Any).unwrap().zone_id,
            "soft"
        );
        assert_eq!(
            ms.select(36, 100, LayerSelector::Any).unwrap().zone_id,
            "hard"
        );
    }
}
