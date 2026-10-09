//! ModuleRuntime — owns the Host (engine + epoch ticker), the
//! registry and the live instance map. This is the surface callers
//! use: `load`, `unload`, `revoke`, `hot_reload`, `cancel`.

use std::collections::BTreeMap;

use crate::capability::Capability;
use crate::error::{Result, WasmError};
use crate::host::{CancelToken, Host, ModuleInstance};
use crate::policy::HostPolicy;
use crate::registry::{EntryStatus, ModuleRegistry, RegistryRow};

/// One live module under the runtime.
pub struct LiveModule {
    pub instance: ModuleInstance,
    pub version: String,
}

impl LiveModule {
    pub fn cancel_token(&self) -> CancelToken {
        self.instance.cancel_token()
    }
}

/// Outcome record for a completed hot reload.
#[derive(Debug, Clone)]
pub struct ReloadReport {
    pub module_id: String,
    pub from_version: String,
    pub to_version: String,
    /// Bytes of guest state carried across; None when the module
    /// doesn't declare `state`.
    pub state_bytes: Option<usize>,
}

pub struct ModuleRuntime {
    host: Host,
    registry: ModuleRegistry,
    /// module_id → live instance (one live version per module id).
    live: BTreeMap<String, LiveModule>,
}

impl ModuleRuntime {
    pub fn new(registry: ModuleRegistry, policy: HostPolicy) -> Result<ModuleRuntime> {
        Ok(ModuleRuntime {
            host: Host::new(policy)?,
            registry,
            live: BTreeMap::new(),
        })
    }

    pub fn registry(&self) -> &ModuleRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut ModuleRegistry {
        &mut self.registry
    }

    pub fn live(&self, module_id: &str) -> Option<&LiveModule> {
        self.live.get(module_id)
    }

    pub fn live_mut(&mut self, module_id: &str) -> Option<&mut LiveModule> {
        self.live.get_mut(module_id)
    }

    pub fn live_version(&self, module_id: &str) -> Option<String> {
        self.live.get(module_id).map(|l| l.version.clone())
    }

    /// Instantiate a registered module (latest active version unless
    /// `version` pins one). Reload over a live id uses `hot_reload`,
    /// not this — plain `load` refuses to displace a live instance.
    pub fn load(&mut self, module_id: &str, version: Option<&str>) -> Result<()> {
        if self.live.contains_key(module_id) {
            return Err(WasmError::InvalidManifest(format!(
                "{module_id} already live — use hot_reload"
            )));
        }
        let ver = match version {
            Some(v) => v.to_string(),
            None => self.registry.latest_active(module_id)?,
        };
        let spec = self
            .registry
            .load_spec(module_id, &ver, self.host.policy())?;
        let instance = self.host.instantiate(&spec)?;
        self.live.insert(
            module_id.to_string(),
            LiveModule {
                instance,
                version: ver,
            },
        );
        Ok(())
    }

    /// Drop a live instance (clean unload; guest memory dies with it).
    pub fn unload(&mut self, module_id: &str) -> Result<()> {
        self.live
            .remove(module_id)
            .map(|_| ())
            .ok_or_else(|| WasmError::NotFound(format!("{module_id} not live")))
    }

    /// Registry revoke + unload of any live instances of the revoked
    /// versions. Revoked entries can never instantiate again (T97:
    /// revoke → unload).
    pub fn revoke(
        &mut self,
        module_id: &str,
        version: Option<&str>,
        reason: &str,
    ) -> Result<Vec<String>> {
        let revoked = self.registry.revoke(module_id, version, reason)?;
        if let Some(live) = self.live.get(module_id) {
            if version.is_none() || version == Some(live.version.as_str()) {
                let _ = self.live.remove(module_id);
            }
        }
        Ok(revoked)
    }

    /// Hot reload: instantiate the target version, carry state across,
    /// then swap. On ANY failure the old instance stays live — the
    /// last-known-good module and its state remain recoverable (T97).
    pub fn hot_reload(
        &mut self,
        module_id: &str,
        to_version: Option<&str>,
    ) -> Result<ReloadReport> {
        let old_ver = self
            .live_version(module_id)
            .ok_or_else(|| WasmError::NotFound(format!("{module_id} not live")))?;
        let new_ver = match to_version {
            Some(v) => v.to_string(),
            None => self.registry.latest_active(module_id)?,
        };
        if new_ver == old_ver {
            return Err(WasmError::HotReloadFailed(format!(
                "{module_id} already at {old_ver}"
            )));
        }
        // Build the new instance BEFORE touching the old one.
        let spec = self
            .registry
            .load_spec(module_id, &new_ver, self.host.policy())
            .map_err(|e| WasmError::HotReloadFailed(format!("resolve {new_ver}: {e}")))?;
        let mut next = self
            .host
            .instantiate(&spec)
            .map_err(|e| WasmError::HotReloadFailed(format!("instantiate {new_ver}: {e}")))?;
        // Carry state only across modules that both declare `state`.
        let old_live = self.live.get_mut(module_id).unwrap();
        let mut state_bytes = None;
        if old_live
            .instance
            .manifest()
            .has_capability(Capability::State)
            && spec.manifest.has_capability(Capability::State)
        {
            let state = old_live
                .instance
                .state_save()
                .map_err(|e| WasmError::HotReloadFailed(format!("state save on {old_ver}: {e}")))?;
            next.state_restore(&state).map_err(|e| {
                WasmError::HotReloadFailed(format!("state restore on {new_ver}: {e}"))
            })?;
            state_bytes = Some(state.len());
        }
        // Swap: old instance drops (guest memory freed), new goes live.
        self.live.insert(
            module_id.to_string(),
            LiveModule {
                instance: next,
                version: new_ver.clone(),
            },
        );
        Ok(ReloadReport {
            module_id: module_id.to_string(),
            from_version: old_ver,
            to_version: new_ver,
            state_bytes,
        })
    }

    /// Cancel the in-flight guest call of a live module (the flag is
    /// checked at every epoch tick — ~1 ms worst case).
    pub fn cancel(&mut self, module_id: &str) -> Result<()> {
        let live = self
            .live
            .get(module_id)
            .ok_or_else(|| WasmError::NotFound(format!("{module_id} not live")))?;
        live.instance.cancel_token().cancel();
        Ok(())
    }

    /// Registry rows for UI/listing.
    pub fn catalog(&self) -> Vec<RegistryRow> {
        self.registry.list()
    }

    /// Is this module row currently live?
    pub fn is_live(&self, module_id: &str) -> bool {
        self.live.contains_key(module_id)
    }

    /// Row status for studio display: live / revoked / installed.
    pub fn row_status(&self, row: &RegistryRow) -> &'static str {
        if row.status == EntryStatus::Revoked {
            "revoked"
        } else if self.live_version(&row.module_id).as_deref() == Some(row.version.as_str()) {
            "live"
        } else {
            "installed"
        }
    }
}
