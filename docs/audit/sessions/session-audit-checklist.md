# Session Audit Checklist

Last updated: April 14, 2026

Use this checklist when auditing any session type. The goal is to evaluate both independent correctness and system interplay.

## Identity

- What session family is this?
- Is it project-scoped, worktree-scoped, workflow-managed, or recovery-state only?
- Which provider family is active?
- Is the launch mode `fresh` or `resume`?

## Entry Path

- What frontend action started this session?
- Which store slice owns the handoff?
- Which Tauri command and supervisor route are used?
- What must already exist for launch to succeed?

## Runtime Contract

- What is the expected root path?
- What launch profile and env inputs are required?
- Which direct systems are touched during launch?
- Which artifacts are created?

## Live Operation

- How does output reach the frontend?
- Which component owns the active terminal view?
- Which state objects are authoritative while the session is live?
- What refreshes should happen while the session is active?

## Exit And Failure

- How should an intentional stop behave?
- How should an unexpected exit behave?
- Which history records, events, and artifacts should exist afterward?
- What cleanup is required immediately?

## Recovery

- Is native resume expected?
- If not, what recovery-context relaunch should happen?
- Which UI surfaces expose recovery details?
- What makes this session recoverable versus terminal?

## Whole-System Interplay

- Which other systems depend on this session being correct?
- Which systems can break this session indirectly?
- What stale-state symptoms would show up elsewhere if this session fails?
- What logs or diagnostics should confirm behavior?

## Verification

- Which existing automated tests cover this session path?
- Which gaps remain?
- What exact repro or smoke test would validate the intended behavior?
