//! Project registry: open projects, their engine attachment, and pending
//! serialized mutation lane (bounded at PENDING_MUTATIONS_MAX).

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use void_protocol::limits::PENDING_MUTATIONS_MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectState {
    /// Persistent container exists on disk; engine may or may not be attached.
    Registered,
    /// Engine has the Edit loaded and accepts commands.
    Attached { engine_epoch: u64 },
    /// Engine died/crashed; project is recoverable from last checkpoint.
    Recoverable,
    Closed,
}

pub struct ProjectHandle {
    pub project_id: String,
    pub container_dir: PathBuf,
    pub state: ProjectState,
    /// Serialized pending mutation queue (one applying lane).
    pub pending: VecDeque<String>,
}

impl ProjectHandle {
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Returns false when the mutation lane is full (caller emits BUSY).
    pub fn try_enqueue(&mut self, command_id: String) -> bool {
        if self.pending.len() >= PENDING_MUTATIONS_MAX {
            return false;
        }
        self.pending.push_back(command_id);
        true
    }

    pub fn dequeue(&mut self) -> Option<String> {
        self.pending.pop_front()
    }
}

#[derive(Default)]
pub struct ProjectRegistry {
    projects: HashMap<String, ProjectHandle>,
}

impl ProjectRegistry {
    pub fn register(&mut self, handle: ProjectHandle) {
        self.projects.insert(handle.project_id.clone(), handle);
    }

    pub fn get(&self, project_id: &str) -> Option<&ProjectHandle> {
        self.projects.get(project_id)
    }

    pub fn get_mut(&mut self, project_id: &str) -> Option<&mut ProjectHandle> {
        self.projects.get_mut(project_id)
    }

    pub fn remove(&mut self, project_id: &str) -> Option<ProjectHandle> {
        self.projects.remove(project_id)
    }

    pub fn len(&self) -> usize {
        self.projects.len()
    }
}
