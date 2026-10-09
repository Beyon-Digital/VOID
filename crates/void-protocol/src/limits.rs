//! Bounded-resource limits from CONTRACTS.md §2. Tunable engineering
//! defaults — measured evidence may justify changes, recorded in tracking.

/// Max bytes for a single control/telemetry frame.
pub const CONTROL_FRAME_MAX: u32 = 1024 * 1024; // 1 MiB

/// Max pending persistent commands per project before BUSY.
pub const PENDING_MUTATIONS_MAX: usize = 128;

/// Max objects per paged view read.
pub const VIEW_PAGE_MAX_OBJECTS: usize = 2_000;

/// Max bytes per paged view read.
pub const VIEW_PAGE_MAX_BYTES: usize = 512 * 1024;

/// Telemetry publication ceiling per subscription.
pub const METER_RATE_MAX_HZ: f64 = 30.0;

/// Worker handshake deadline.
pub const HANDSHAKE_TIMEOUT_MS: u64 = 10_000;

/// Bounded restart attempts per worker before declaring failure.
pub const WORKER_RESTART_MAX: u32 = 3;
