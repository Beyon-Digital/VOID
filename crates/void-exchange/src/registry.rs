//! Plugin compatibility registry (W20, HOST-01/02/05).
//!
//! A descriptor record is the stable identity + compatibility metadata of
//! one plugin build: format, format-unique uid, arch binaries, and the
//! version of the *state blobs* it writes. The registry is a plain JSON
//! document (`void-plugin-registry/1`) the coordinator owns; it records
//! observed hostability — it never claims a format is hostable that the
//! platform cannot run (AAX is recorded so it can be *rejected* honestly).
//!
//! What this does NOT do: load or execute plugins. Runtime hosting,
//! scanning and isolation are native-engine work (docs/exchange/NEEDS.md).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Plugin binary formats VOID knows about. `Aax` exists so its explicit
/// non-support is honest (W20: "never promise AAX hosting"). `Other`
/// preserves unknown formats for round-trip fidelity without pretending
/// to understand them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginFormat {
    Vst3,
    Vst2,
    Clap,
    Au,
    Aax,
    Builtin,
    #[serde(untagged)]
    Other(String),
}

impl PluginFormat {
    /// DAWproject device element names for this format.
    pub fn dawproject_element(&self) -> &'static str {
        match self {
            PluginFormat::Vst3 => "Vst3Plugin",
            PluginFormat::Vst2 => "Vst2Plugin",
            PluginFormat::Clap => "ClapPlugin",
            PluginFormat::Au => "AuPlugin",
            // No AAX element exists — and VOID writes none.
            PluginFormat::Aax => "Device",
            PluginFormat::Builtin => "BuiltinDevice",
            PluginFormat::Other(_) => "Device",
        }
    }

    /// Lowercase wire string ("vst3" | "clap" | ...) matching serde.
    pub fn as_str(&self) -> &str {
        match self {
            PluginFormat::Vst3 => "vst3",
            PluginFormat::Vst2 => "vst2",
            PluginFormat::Clap => "clap",
            PluginFormat::Au => "au",
            PluginFormat::Aax => "aax",
            PluginFormat::Builtin => "builtin",
            PluginFormat::Other(s) => s.as_str(),
        }
    }

    pub fn from_dawproject_element(name: &str) -> Self {
        match name {
            "Vst3Plugin" => PluginFormat::Vst3,
            "Vst2Plugin" => PluginFormat::Vst2,
            "ClapPlugin" => PluginFormat::Clap,
            "AuPlugin" => PluginFormat::Au,
            "BuiltinDevice" | "Equalizer" | "Compressor" | "NoiseGate" | "Limiter" => {
                PluginFormat::Builtin
            }
            "Device" => PluginFormat::Other("unknown".to_string()),
            other => PluginFormat::Other(other.to_string()),
        }
    }
}

/// dawproject deviceRole + VOID's own. `Other` keeps unmapped roles.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceRole {
    Instrument,
    NoteFx,
    AudioFx,
    Analyzer,
    #[serde(untagged)]
    Other(String),
}

impl DeviceRole {
    pub fn as_str(&self) -> &str {
        match self {
            DeviceRole::Instrument => "instrument",
            DeviceRole::NoteFx => "noteFX",
            DeviceRole::AudioFx => "audioFX",
            DeviceRole::Analyzer => "analyzer",
            DeviceRole::Other(s) => s.as_str(),
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "instrument" => DeviceRole::Instrument,
            "noteFX" => DeviceRole::NoteFx,
            "audioFX" => DeviceRole::AudioFx,
            "analyzer" => DeviceRole::Analyzer,
            other => DeviceRole::Other(other.to_string()),
        }
    }
}

/// A single plugin build. `plugin_uid` is the format-specific identity —
/// VST3 class UID hex, CLAP id, AU type/subtype/manufacturer triplet.
/// Two arch variants of "the same plugin" are two descriptors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDescriptor {
    pub format: PluginFormat,
    pub plugin_uid: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default)]
    pub device_role: Option<DeviceRole>,
    /// Architectures this build contains ("x86_64", "arm64", "universal").
    /// Empty = unknown.
    #[serde(default)]
    pub arch: Vec<String>,
    /// Version of the state blob format this plugin writes (e.g. VST3
    /// stream version, CLAP state version). None = unknown/unspecified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_format_version: Option<String>,
}

impl PluginDescriptor {
    /// Registry key — format + uid is the identity contract.
    pub fn key(&self) -> String {
        format!("{:?}/{}", self.format, self.plugin_uid).to_lowercase()
    }

    /// True when `self` can load state written by `other`: same identity,
    /// and state format versions either equal or one side unspecified
    /// (unknown versions are optimistic-compatible — a mismatch that a
    /// real host would reject is recorded, not hidden).
    pub fn can_load_state_of(&self, other: &PluginDescriptor) -> bool {
        if self.format != other.format || self.plugin_uid != other.plugin_uid {
            return false;
        }
        match (&self.state_format_version, &other.state_format_version) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }

    /// True when this build declares a binary for `arch`
    /// (e.g. "x86_64", "arm64"). `universal` satisfies any arch.
    pub fn supports_arch(&self, arch: &str) -> bool {
        self.arch.is_empty() || self.arch.iter().any(|a| a == "universal" || a == arch)
    }
}

/// One registry row: the descriptor plus observed compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryEntry {
    pub descriptor: PluginDescriptor,
    /// Platform ids this build has actually been observed hosted on
    /// ("macos-arm64", "windows-x86_64", "linux-x86_64"). Observed, not
    /// claimed — an empty list is an honest "never hosted".
    #[serde(default)]
    pub hosted_on: Vec<String>,
    /// Quarantined after a scan/runtime crash — kept listed, never loaded.
    #[serde(default)]
    pub quarantined: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompatibilityStatus {
    /// Descriptor registered and observed hosted on this platform.
    Compatible,
    /// Registered but never hosted on this platform (or arch missing).
    NotObservedOnPlatform,
    /// Registered with no compatible binary arch for this platform.
    ArchMismatch,
    /// Explicitly quarantined (scan or runtime crash).
    Quarantined,
    /// VOID never hosts this format (AAX) or cannot on this platform (AU
    /// off macOS). A hard no, not an unknown.
    FormatNotHostable,
    /// No registry record — unknown is reported, not assumed compatible.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityReport {
    pub status: CompatibilityStatus,
    pub reasons: Vec<String>,
}

/// `platform` ids are "<os>-<arch>" ("macos-arm64", "windows-x86_64",
/// "linux-x86_64"). The arch part is matched against descriptor.arch.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegistry {
    /// `void-plugin-registry/1`.
    pub format: String,
    #[serde(default)]
    pub plugins: BTreeMap<String, RegistryEntry>,
}

impl PluginRegistry {
    pub const FORMAT: &'static str = "void-plugin-registry/1";

    pub fn new() -> Self {
        Self {
            format: Self::FORMAT.to_string(),
            plugins: BTreeMap::new(),
        }
    }

    /// Insert or replace the row for this descriptor's key.
    pub fn upsert(&mut self, entry: RegistryEntry) {
        self.plugins.insert(entry.descriptor.key(), entry);
    }

    pub fn lookup(&self, format: &PluginFormat, plugin_uid: &str) -> Option<&RegistryEntry> {
        let probe = PluginDescriptor {
            format: format.clone(),
            plugin_uid: plugin_uid.to_string(),
            name: String::new(),
            vendor: None,
            version: None,
            device_role: None,
            arch: Vec::new(),
            state_format_version: None,
        };
        self.plugins.get(&probe.key())
    }

    /// Record a scan/runtime crash — the descriptor stays listed (a
    /// missing plugin is still preserved state) but is marked unsafe.
    pub fn mark_quarantined(&mut self, descriptor: &PluginDescriptor, reason: impl Into<String>) {
        let key = descriptor.key();
        let entry = self.plugins.entry(key).or_insert_with(|| RegistryEntry {
            descriptor: descriptor.clone(),
            hosted_on: Vec::new(),
            quarantined: false,
            quarantine_reason: None,
            notes: Vec::new(),
        });
        entry.quarantined = true;
        entry.quarantine_reason = Some(reason.into());
    }

    /// The compatibility answer for this build on this platform. Pure
    /// function of the descriptor + observed rows — never consults the
    /// network or filesystem.
    pub fn compatibility(
        &self,
        descriptor: &PluginDescriptor,
        platform: &str,
    ) -> CompatibilityReport {
        let mut reasons = Vec::new();

        // Hard format rules — recorded as explicit rejections, not silence.
        if descriptor.format == PluginFormat::Aax {
            return CompatibilityReport {
                status: CompatibilityStatus::FormatNotHostable,
                reasons: vec!["AAX hosting is not a VOID capability (Avid-only format)".into()],
            };
        }
        if descriptor.format == PluginFormat::Au && !platform.starts_with("macos") {
            return CompatibilityReport {
                status: CompatibilityStatus::FormatNotHostable,
                reasons: vec![format!(
                    "AU plugins are macOS-only; host platform is {platform:?}"
                )],
            };
        }

        let arch = platform.rsplit('-').next().unwrap_or("");
        if !descriptor.supports_arch(arch) {
            reasons.push(format!(
                "no binary for arch {arch:?} (declared: {})",
                descriptor.arch.join(",")
            ));
            return CompatibilityReport {
                status: CompatibilityStatus::ArchMismatch,
                reasons,
            };
        }

        match self.lookup(&descriptor.format, &descriptor.plugin_uid) {
            None => CompatibilityReport {
                status: CompatibilityStatus::Unknown,
                reasons: vec!["no registry record — plugin never scanned".into()],
            },
            Some(entry) if entry.quarantined => CompatibilityReport {
                status: CompatibilityStatus::Quarantined,
                reasons: vec![entry
                    .quarantine_reason
                    .clone()
                    .unwrap_or_else(|| "quarantined (no reason recorded)".into())],
            },
            Some(entry) => {
                let status = if entry.hosted_on.iter().any(|p| p == platform) {
                    CompatibilityStatus::Compatible
                } else {
                    reasons.push(format!(
                        "registered but never hosted on {platform:?} (observed: {})",
                        if entry.hosted_on.is_empty() {
                            "nowhere".to_string()
                        } else {
                            entry.hosted_on.join(",")
                        }
                    ));
                    CompatibilityStatus::NotObservedOnPlatform
                };
                CompatibilityReport { status, reasons }
            }
        }
    }

    pub fn to_json(&self) -> crate::error::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }

    pub fn parse(bytes: &[u8]) -> crate::error::Result<Self> {
        let reg: Self = serde_json::from_slice(bytes).map_err(|e| {
            crate::error::ExchangeError::Malformed(format!("plugin registry json: {e}"))
        })?;
        if reg.format != Self::FORMAT {
            return Err(crate::error::ExchangeError::Unsupported(format!(
                "registry format {:?} (expected {:?})",
                reg.format,
                Self::FORMAT
            )));
        }
        Ok(reg)
    }

    pub fn save_to(&self, path: &std::path::Path) -> crate::error::Result<()> {
        std::fs::write(path, self.to_json()?)?;
        Ok(())
    }

    pub fn load_from(path: &std::path::Path) -> crate::error::Result<Self> {
        Self::parse(&std::fs::read(path)?)
    }
}
