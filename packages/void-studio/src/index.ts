export * from './viewport';
export * from './store';
export * from './bind';
export * from './hooks';
export * from './workspaces';
export * from './timeline';
export * from './piano-roll';
export * from './instruments';
export * from './mixer';
export * from './export';
// './export' and './jobs' both define `isTerminal` for different status
// types — './export' keeps the bare name; jobs' version is qualified.
export { isTerminal as isAiJobTerminal } from './jobs/job';
export {
  type JobStatus,
  type JobKind,
  type JobArtifact,
  type JobCard,
  type SubmitJobOp,
  type CancelJobOp,
  type JobSpecEnvelope,
  parseJobCard,
  applyJobEventToCard,
  jobEventOf,
  cancelJobOp,
  submitJobOp,
  jobListRequest,
  modelListRequest,
} from './jobs/job';
export * from './jobs/models';
export * from './jobs/store';
export * from './i18n';
export * from './templates';
export * from './recents';
export * from './notes';
export * from './relink';
export * from './onboarding';
export * from './proposals';
// Re-export the vanilla→react bridge so apps can consume the feature
// stores above without declaring a direct zustand dependency.
export { useStore } from 'zustand';
export * from './generation';
export * from './gestures';
export * from './patterns';
export * from './harmony';
export * from './learn';
export * from './takes';
export * from './arrangement';
export * from './scenes';
export * from './visuals';
