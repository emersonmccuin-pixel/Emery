# Workflow Stage Session

Last updated: April 15, 2026

## Purpose

A workflow-stage session is a fresh, workflow-managed session launched to execute one resolved stage of a workflow run.

## Expected Behavior

- Resolve a frozen workflow definition before launch.
- Launch a fresh session for the stage on the managed worktree.
- Apply stage-specific vault bindings and provider/model directives.
- Send the directive message, wait for a reply, persist stage output, then terminate the stage session.
- Advance or fail the run consistently with the stage result.

## Actual Implementation

### Entry Path

- `WorkflowsPanel` acts as the workflow control plane: it hydrates workflow catalog and runs when the workflows surface opens, then starts runs from an adopted workflow plus a root work item.
- `workflowSlice.startWorkflowRun()` is the client-side owner of the launch request. It sets an in-flight key, invokes `start_workflow_run`, then refreshes workflow runs, worktrees, and live sessions through `refreshSelectedProjectData()`.
- The desktop UI does not launch workflow-stage sessions directly; it starts workflow runs and then the supervisor dispatch loop launches stage sessions behind the scenes.

### Runtime Handoff

- `lib.rs` forwards `start_workflow_run` to `SupervisorClient`.
- `SupervisorClient` calls the supervisor `workflow/run/start` route.
- The supervisor route validates the target, then `workflow.rs` creates the run and pending stage records inside a DB transaction.
- After the run is created, the supervisor dispatch loop launches each stage session on the run worktree, sends the directive, waits for a reply, records the result, and advances or fails the run.

### Stage And Session Coupling

- Workflow-stage sessions are not a separate runtime family in the backend.
- Stage rows carry `worktree_id`, `session_id`, `agent_name`, `thread_id`, and message ids, which means workflow-stage state is tied to generic session/runtime state rather than a distinct workflow-session protocol.
- Stage progress is derived from directive/reply messaging and workflow persistence, not from a dedicated workflow terminal stream.

### Live Operation

- Stage session output still flows through the generic session poller path.
- `LiveTerminal` and the terminal event system do not know about workflow stages specifically; they only see generic sessions attached to worktree targets.
- `App.tsx` refreshes workflow runs when the workflows view is active and the selected project emits `terminal-exit` or live-session changes, which keeps workflow observability out of the panel itself but still makes stage progress visibility partially indirect.

### Failure And Cleanup

- Launch, directive, wait, and persistence failures can all fail the workflow run.
- Cleanup is asymmetric: some failures call workflow failure paths, while session termination and runtime cleanup still flow through generic session infrastructure.
- Generic session crash handling and auto-restart logic can overlap conceptually with workflow-run failure handling because workflow-stage sessions still occupy the shared worktree/session target model.

## Direct Systems

- `workflow.rs`
- supervisor workflow dispatch path
- `SessionRegistry`
- session records
- workflow run + stage tables
- vault binding resolution
- agent messages and thread ids

## Indirect Systems

- worktrees and work items
- history/event surfaces
- diagnostics
- recovery if a workflow-managed session fails mid-stage

## Key Handoffs

- workflow UI/start action -> `start_workflow_run`
- resolved workflow snapshot -> stage session launch
- stage session -> directive message send
- reply wait -> stage result persistence
- stage result -> run status update -> stage session termination

## Main Risks

- workflow-stage sessions depend on several other systems being correct before a launch even begins
- workflow-stage sessions are not modeled as a distinct runtime family, so workflow correctness depends on generic session semantics remaining aligned
- workflow progress is partly inferred from message semantics and delayed run refreshes instead of one dedicated stage-state stream

## Expected Vs Actual Summary

### Expected

- A workflow run should freeze a resolved workflow definition, then launch stage sessions in a controlled, observable sequence.
- Each stage session should have explicit stage context, clear failure handling, and deterministic state transitions back into the run timeline.
- Workflow-stage behavior should remain understandable even though it reuses the generic session runtime.

### Actual

- The launch and stage loop are operationally clear once they reach the supervisor, but workflow-stage state is still coupled to generic session rows, worktree targets, and message semantics.
- The workflow control plane owns run visibility, yet the actual stage session output and failures still flow through generic terminal/session infrastructure.
- Failure handling works, but cleanup and post-failure state transitions are split across workflow failure paths and generic session termination behavior.

## Confirmed Mismatches

- Workflow-stage sessions are not expressed as a first-class runtime family; they are worktree-bound sessions annotated back into workflow stage rows.
- Stage progress is tied to dispatcher/agent message semantics rather than a dedicated workflow-stage protocol.
- Workflow visibility still depends on visible-context refresh triggered by `terminal-exit` and live-session changes, which means stage progress can lag behind the underlying message exchange.
- Shared latest-session selection and generic crash handling can overlap with workflow-stage recovery expectations.

## Open Questions

- Should workflow-stage sessions remain an annotation on generic session state, or should the session-system contract name them more explicitly?
- Which failures should be treated as workflow-run failures only, and which should also trigger generic session recovery paths?
- Does workflow observability need a more direct event path than run refresh on `terminal-exit` and live-session count changes?

## What To Audit Next

- reproduce the workflow-stage smoke path: start run, observe stage launch, receive reply, persist result, and complete or fail the run
- test concurrent workflow-run start and confirm whether the active-run precheck can race
- inspect a stage crash between directive dispatch and result persistence
- verify stage secret delivery/redaction and the resulting diagnostics/run timeline
