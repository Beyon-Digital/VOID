# T64 — Musician usability protocol (BLOCKED, runnable script)

**Status: BLOCKED — needs a human musician session.** T64 is a human
task ("a musician who didn't build VOID completes a session task");
no synthetic substitute is honest. This document is the runnable
protocol so the gate can be executed later and the result recorded
against TEST_MATRIX T64.

## Setup

1. Build the app bundle on the target machine; start VOID fresh.
2. Pick a participant who has never driven this build. Tell them only
   the goal below — no walkthrough, no hints. Screen + session
   recording on.
3. Have a second terminal open for the observation log template.

## Task card (read verbatim to the participant)

> "Import any audio clip onto a track. Ask VOID to continue the idea —
> get a few suggested continuations, listen to them, and put the one
> you like into the song. Then split the clip, lock the last two bars,
> and ask for one more suggestion. Undo whatever you dislike. Save the
> project."

## Observation protocol — what the observer records

| Step | Success signal | Failure signal | Time |
|---|---|---|---|
| Import audio | clip visible on a track | participant can't find import | ___ |
| Request proposals | proposal list appears, each named by model + context | no entry point found / job never finishes | ___ |
| Audition a candidate | can hear/preview a proposal before accepting | no preview → must accept blind | ___ |
| Accept best | notes land in the clip as one undoable transaction | accept fails / stale loop | ___ |
| Split + lock | locked range renders + is honored | lock state unclear or ignored | ___ |
| Regenerate | new proposal reflects the edited region | stale proposals not swept / wrong context | ___ |
| Undo | one gesture rolls back the whole accept | partial undo / orphan notes | ___ |
| Save | checkpoint written + reopen shows it | save lost or unchecked | ___ |

Per step also record: time-to-complete, wrong turns taken, verbatim
participant comment, and whether the UI *told the truth* (job status,
stale/rejected reasons, license flag on musicgen).

## Scoring

- PASS: all 8 steps completed without moderator help; UI states
  matched backend truth throughout.
- PARTIAL: completed with moderator hints or one false UI state.
- FAIL: blocked step, data loss, or a state the UI misrepresented.

## Known hazards for the observer to watch (pre-registered)

- MusicGen outputs are CC-BY-NC-4.0 — the participant should see the
  license flag before audition/accept, not after.
- There is **no preview/audition layer yet** (NEEDS §12): "listen to
  them" currently means accepting to the clip and playing the result —
  record that friction honestly; it is a real gap, not a test failure
  to fix silently.
- Proposal stale/rejected copy: does the participant understand *why*
  a proposal disappeared after a region edit?

## Result record

Copy this table into `docs/verification/F2/T64_RESULT.md` per session
(date, participant profile, build sha, per-step table, PASS/PARTIAL/
FAIL, raw notes). T64 stays BLOCKED until a result file exists.
