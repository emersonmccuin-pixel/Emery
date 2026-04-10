import { Suspense, type FormEvent } from 'react'
import { Settings, Plus, ChevronLeft, ChevronRight } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import DocumentsPanel from './DocumentsPanel'
import LiveTerminal from './LiveTerminal'
import SettingsPanel from './SettingsPanel'
import WorkItemsPanel from './WorkItemsPanel'

type WorkspaceShellProps = {
  state: any
  actions: any
}

function WorkspaceShell({ state, actions }: WorkspaceShellProps) {
  const {
    projects,
    selectedProject,
    selectedProjectId,
    selectedWorktree,
    selectedTerminalWorktreeId,
    selectedLaunchProfile,
    selectedLaunchProfileId,
    sessionSnapshot,
    sessionError,
    workItems,
    workItemError,
    documents,
    documentError,
    agentPromptMessage,
    currentTerminalPrompt,
    currentTerminalPromptLabel,
    hasFocusedPrompt,
    bridgeReady,
    openWorkItemCount,
    blockedWorkItemCount,
    recentDocuments,
    liveSessions,
    worktreeSessions,
    orphanedSessions,
    runtimeCleanupCandidates,
    staleWorktreeCleanupCandidates,
    recoveryActionCount,
    activeOrphanSessionId,
    activeCleanupPath,
    isRepairingCleanup,
    sessionRailRevision,
    hasSelectedProjectLiveSession,
    launchBlockedByMissingRoot,
    selectedProjectLaunchLabel,
    selectedTerminalLaunchLabel,
    projectName,
    projectRootPath,
    projectError,
    isProjectCreateOpen,
    isDocumentsManagerOpen,
    isAgentGuideOpen,
    activeView,
    isProjectRailCollapsed,
    isSessionRailCollapsed,
    isCreatingProject,
    isLaunchingSession,
    isStoppingSession,
    isLoadingWorkItems,
    isLoadingDocuments,
    startingWorkItemId,
    launchProfiles,
    projectCommanderTools,
  } = state

  const {
    setProjectError,
    setSelectedLaunchProfileId,
    setProjectName,
    setProjectRootPath,
    setIsProjectCreateOpen,
    setIsDocumentsManagerOpen,
    setIsAgentGuideOpen,
    setActiveView,
    setIsProjectRailCollapsed,
    setIsSessionRailCollapsed,
    setTerminalPromptDraft,
    selectProject,
    selectMainTerminal,
    selectWorktreeTerminal,
    browseForProjectFolder,
    submitProject,
    launchWorkspaceGuide,
    stopSession,
    terminateRecoveredSession,
    removeStaleArtifact,
    repairCleanupCandidates,
    copyAgentStartupPrompt,
    sendAgentStartupPrompt,
    copyTerminalOutput,
    handleSessionExit,
    createWorkItem,
    updateWorkItem,
    deleteWorkItem,
    startWorkItemInTerminal,
    createDocument,
    updateDocument,
    deleteDocument,
  } = actions

  // Derive dispatcher status from live sessions
  const isDispatcherRunning = liveSessions.some(
    ({ project, snapshot }: any) =>
      project.id === selectedProjectId && snapshot.isRunning
  )

  return (
    <main className="terminal-app">
      <section
        className={`terminal-layout ${
          isProjectRailCollapsed ? 'terminal-layout--left-collapsed' : ''
        } ${isSessionRailCollapsed ? 'terminal-layout--right-collapsed' : ''}`}
      >
        {/* ── LEFT PANEL: Projects ── */}
        <aside
          className={`side-rail side-rail--projects ${
            isProjectRailCollapsed ? 'side-rail--collapsed' : ''
          }`}
        >
          {isProjectRailCollapsed ? (
            <div className="side-rail__collapsed">
              <button
                className="rail-toggle"
                type="button"
                onClick={() => setIsProjectRailCollapsed(false)}
                title="Expand Projects"
              >
                <ChevronRight size={16} />
              </button>
            </div>
          ) : (
            <>
              <div className="side-rail__header">
                <span className="rail-label">PROJECTS</span>
                <div className="rail-header-actions">
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => setIsProjectCreateOpen((c: boolean) => !c)}
                    title="Add project"
                    className="h-6 w-6"
                  >
                    <Plus size={14} />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => setIsProjectRailCollapsed(true)}
                    title="Collapse"
                    className="h-6 w-6"
                  >
                    <ChevronLeft size={14} />
                  </Button>
                </div>
              </div>

              <ScrollArea className="flex-1 -mx-1 px-1">
                <div className="project-list project-list--minimal mt-2">
                  {projects.length === 0 ? (
                    <div className="empty-state empty-state--rail text-center py-4">
                      No projects yet.
                    </div>
                  ) : (
                    projects.map((project: any) => (
                      <button
                        key={project.id}
                        className={`project-card project-card--minimal w-full text-left truncate ${
                          project.id === selectedProjectId
                            ? 'project-card--active'
                            : ''
                        }`}
                        type="button"
                        onClick={() => selectProject(project.id)}
                      >
                        {project.name}
                      </button>
                    ))
                  )}
                </div>
              </ScrollArea>

              {isProjectCreateOpen ? (
                <div className="mt-4 border-t border-border pt-4">
                  <form
                    className="stack-form stack-form--rail"
                    onSubmit={(event) =>
                      void submitProject(event as FormEvent<HTMLFormElement>)
                    }
                  >
                    <div className="stack-form__header mb-2">
                      <h3 className="text-xs font-bold uppercase tracking-wider">New Project</h3>
                    </div>

                    <div className="space-y-3">
                      <label className="field">
                        <span className="text-[10px] uppercase text-muted-foreground">Name</span>
                        <Input
                          value={projectName}
                          onChange={(event) => setProjectName(event.target.value)}
                          placeholder="Project name"
                          className="h-8 text-xs"
                        />
                      </label>

                      <label className="field">
                        <span className="text-[10px] uppercase text-muted-foreground">Root folder</span>
                        <div className="flex gap-2">
                          <Input
                            value={projectRootPath}
                            onChange={(event) =>
                              setProjectRootPath(event.target.value)
                            }
                            placeholder="Path"
                            className="h-8 text-xs flex-1"
                          />
                          <Button
                            variant="outline"
                            size="sm"
                            type="button"
                            className="h-8 px-2 text-[10px]"
                            onClick={() =>
                              browseForProjectFolder(
                                setProjectRootPath,
                                setProjectError,
                              )
                            }
                          >
                            ...
                          </Button>
                        </div>
                      </label>

                      {projectError ? (
                        <p className="text-[10px] text-destructive">{projectError}</p>
                      ) : null}

                      <div className="flex gap-2">
                        <Button
                          variant="default"
                          size="sm"
                          disabled={isCreatingProject}
                          type="submit"
                          className="h-7 text-[10px] flex-1"
                        >
                          {isCreatingProject ? '...' : 'CREATE'}
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          type="button"
                          className="h-7 text-[10px] flex-1"
                          onClick={() => setIsProjectCreateOpen(false)}
                        >
                          CANCEL
                        </Button>
                      </div>
                    </div>
                  </form>
                </div>
              ) : null}
            </>
          )}
        </aside>

        {/* ── CENTER PANEL: Workspace ── */}
        <section className="center-stage">
          {!selectedProject ? (
            <div className="empty-state empty-state--center">
              Select a project or add one to begin.
            </div>
          ) : (
            <>
              {sessionError ? (
                <p className="form-error">{sessionError}</p>
              ) : null}
              {launchBlockedByMissingRoot ? (
                <p className="form-error">
                  {selectedWorktree
                    ? 'Worktree path missing. Start the work item again to recreate.'
                    : 'Root missing. Rebind in Settings before launching.'}
                </p>
              ) : null}

              <nav className="workspace-tabs workspace-tabs--shell">
                <button
                  className={`workspace-tab ${
                    activeView === 'terminal' ? 'workspace-tab--active' : ''
                  }`}
                  type="button"
                  onClick={() => setActiveView('terminal')}
                >
                  TERMINAL
                </button>
                <button
                  className={`workspace-tab ${
                    activeView === 'overview' ? 'workspace-tab--active' : ''
                  }`}
                  type="button"
                  onClick={() => setActiveView('overview')}
                >
                  OVERVIEW
                </button>
                <button
                  className={`workspace-tab ${
                    activeView === 'workItems' ? 'workspace-tab--active' : ''
                  }`}
                  type="button"
                  onClick={() => setActiveView('workItems')}
                >
                  WORK ITEMS
                  {workItems.length > 0 ? (
                    <span className="workspace-tab__count">
                      {workItems.length}
                    </span>
                  ) : null}
                </button>
                <button
                  className={`workspace-tab ${
                    activeView === 'settings' ? 'workspace-tab--active' : ''
                  }`}
                  type="button"
                  onClick={() => setActiveView('settings')}
                >
                  SETTINGS
                </button>
              </nav>

              {activeView === 'terminal' ? (
                <section className="terminal-view flex flex-col h-full">
                  <div className="terminal-view__toolbar flex items-center justify-between px-3 py-2 border-b border-border bg-card/50">
                    <div className="flex items-center gap-3">
                      <span className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground">Terminal</span>
                      <div className="h-4 w-[1px] bg-border" />
                      <span className="text-xs font-mono truncate max-w-[300px]">
                        {hasSelectedProjectLiveSession
                          ? sessionSnapshot.profileLabel
                          : selectedTerminalLaunchLabel}
                      </span>
                    </div>

                    <div className="flex items-center gap-2">
                      {!hasSelectedProjectLiveSession ? (
                        <div className="flex items-center gap-2 mr-2">
                          <span className="text-[10px] uppercase text-muted-foreground">Account</span>
                          <select
                            className="h-7 text-[10px] bg-background border border-border rounded px-2 min-w-[120px]"
                            value={
                              selectedLaunchProfileId === null
                                ? ''
                                : String(selectedLaunchProfileId)
                            }
                            onChange={(event) =>
                              setSelectedLaunchProfileId(
                                event.target.value === ''
                                  ? null
                                  : Number(event.target.value),
                              )
                            }
                          >
                            <option value="">Choose account</option>
                            {launchProfiles.map((profile: any) => (
                              <option key={profile.id} value={profile.id}>
                                {profile.label}
                              </option>
                            ))}
                          </select>
                        </div>
                      ) : null}

                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7 text-[10px] uppercase tracking-wider"
                        onClick={() =>
                          setIsAgentGuideOpen((current: boolean) => !current)
                        }
                      >
                        {isAgentGuideOpen ? 'Hide guide' : 'Guide'}
                      </Button>

                      {hasSelectedProjectLiveSession ? (
                        <div className="flex items-center gap-2">
                          <Button
                            variant="outline"
                            size="sm"
                            className="h-7 text-[10px] uppercase tracking-wider"
                            disabled={!sessionSnapshot?.output}
                            onClick={() => void copyTerminalOutput()}
                          >
                            Copy
                          </Button>
                          <Button
                            variant="destructive"
                            size="sm"
                            className="h-7 text-[10px] uppercase tracking-wider"
                            disabled={isStoppingSession}
                            onClick={() => void stopSession()}
                          >
                            {isStoppingSession ? '...' : 'STOP'}
                          </Button>
                        </div>
                      ) : (
                        <Button
                          variant="default"
                          size="sm"
                          className="h-7 text-[10px] font-bold uppercase tracking-widest bg-primary text-primary-foreground hover:bg-primary/90"
                          disabled={
                            !selectedLaunchProfile ||
                            isLaunchingSession ||
                            launchBlockedByMissingRoot
                          }
                          onClick={() => void launchWorkspaceGuide()}
                        >
                          {isLaunchingSession
                            ? 'LAUNCHING...'
                            : launchBlockedByMissingRoot
                              ? 'REBIND ROOT'
                              : 'LAUNCH'}
                        </Button>
                      )}
                    </div>
                  </div>

                  <ScrollArea className="flex-1 bg-black/20">
                    <div className="p-4 h-full">
                      {isAgentGuideOpen ? (
                        <article className="guide-panel mb-4 border border-border bg-card/30 p-4 rounded shadow-sm">
                          <div className="flex justify-between items-start mb-4">
                            <div>
                              <p className="text-[10px] uppercase text-muted-foreground mb-1">Startup guide</p>
                              <h4 className="text-sm font-bold">{currentTerminalPromptLabel}</h4>
                            </div>
                            <Badge variant={bridgeReady ? 'running' : 'offline'} className="text-[9px]">
                              {bridgeReady ? 'ATTACHED' : 'DETACHED'}
                            </Badge>
                          </div>
                          
                          <div className="flex gap-2 mb-4">
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-7 text-[10px] flex-1"
                              onClick={() => void copyAgentStartupPrompt()}
                            >
                              COPY PROMPT
                            </Button>
                            <Button
                              variant="default"
                              size="sm"
                              className="h-7 text-[10px] flex-1"
                              disabled={!bridgeReady}
                              onClick={() => void sendAgentStartupPrompt()}
                            >
                              SEND TO TERMINAL
                            </Button>
                            {hasFocusedPrompt ? (
                              <Button
                                variant="ghost"
                                size="sm"
                                className="h-7 text-[10px]"
                                onClick={() => setTerminalPromptDraft(null)}
                              >
                                RESET
                              </Button>
                            ) : null}
                          </div>

                          <textarea
                            className="w-full bg-black/40 border border-border rounded p-3 font-mono text-xs leading-relaxed mb-4 focus:ring-1 focus:ring-primary/30 outline-none"
                            readOnly
                            rows={10}
                            value={currentTerminalPrompt}
                          />

                          <div className="flex flex-wrap gap-1.5 mt-2">
                            {projectCommanderTools.map((toolName: string) => (
                              <code key={toolName} className="text-[9px] bg-muted/50 px-1.5 py-0.5 rounded border border-border/50 opacity-60">
                                {toolName}
                              </code>
                            ))}
                          </div>
                          {agentPromptMessage ? (
                            <p className="mt-4 text-[10px] italic text-primary/80 animate-pulse">
                              {agentPromptMessage}
                            </p>
                          ) : null}
                        </article>
                      ) : null}

                      {hasSelectedProjectLiveSession ? (
                        <div className="h-full min-h-[500px] border border-border/20 rounded-sm overflow-hidden bg-black shadow-2xl">
                          <Suspense
                            fallback={
                              <div className="flex items-center justify-center h-full text-muted-foreground font-mono text-xs animate-pulse">
                                [ SYSTEM ] Preparing secure terminal bridge...
                              </div>
                            }
                          >
                            <LiveTerminal
                              snapshot={sessionSnapshot}
                              onSessionExit={handleSessionExit}
                            />
                          </Suspense>
                        </div>
                      ) : (
                        <div className="flex flex-col items-center justify-center h-full min-h-[400px] text-center max-w-md mx-auto">
                          <div className="w-16 h-16 rounded-full bg-primary/5 flex items-center justify-center mb-6 border border-primary/10">
                            <Settings className="text-primary/40" size={32} />
                          </div>
                          <h3 className="text-lg font-bold mb-2 tracking-tight">
                            {selectedWorktree ? selectedWorktree.branchName : selectedProject.name}
                          </h3>
                          <p className="text-xs text-muted-foreground leading-relaxed mb-8">
                            {selectedWorktree
                              ? selectedWorktree.pathAvailable
                                ? `Ready to start work on #${selectedWorktree.workItemId} in a isolated worktree environment.`
                                : 'The worktree directory is missing. Please recreate it from the Work Items tab.'
                              : `Primary session ready in ${selectedProject.rootPath}. Choose an account to begin.`}
                          </p>
                          
                          <div className="grid grid-cols-2 gap-4 w-full">
                            <div className="p-3 bg-card/30 border border-border rounded-sm">
                              <span className="block text-[9px] uppercase text-muted-foreground mb-1">Items</span>
                              <span className="text-sm font-mono font-bold">{openWorkItemCount}</span>
                            </div>
                            <div className="p-3 bg-card/30 border border-border rounded-sm">
                              <span className="block text-[9px] uppercase text-muted-foreground mb-1">Docs</span>
                              <span className="text-sm font-mono font-bold">{documents.length}</span>
                            </div>
                          </div>
                          
                          <Button
                            variant="default"
                            size="lg"
                            className="mt-8 w-full h-11 font-bold uppercase tracking-[0.2em] bg-primary text-primary-foreground hover:bg-primary/90"
                            disabled={!selectedLaunchProfile || isLaunchingSession || launchBlockedByMissingRoot}
                            onClick={() => void launchWorkspaceGuide()}
                          >
                            {isLaunchingSession ? 'INITIALIZING...' : 'BOOT SESSION'}
                          </Button>
                        </div>
                      )}
                    </div>
                  </ScrollArea>
                </section>
              ) : null}

              {activeView === 'overview' ? (
                <ScrollArea className="flex-1">
                  <div className="p-4 space-y-6">
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                      <article className="overview-card border border-border bg-card/30 rounded p-4">
                        <div className="flex justify-between items-start mb-4">
                          <div>
                            <p className="text-[10px] uppercase text-muted-foreground mb-1">Project status</p>
                            <h4 className="text-sm font-bold">{selectedProject.name}</h4>
                          </div>
                          <Button
                            variant="outline"
                            size="sm"
                            className="h-6 text-[10px]"
                            onClick={() => setActiveView('settings')}
                          >
                            OPEN SETTINGS
                          </Button>
                        </div>

                        <div className="space-y-2">
                          <div className="flex flex-col">
                            <span className="text-[9px] uppercase text-muted-foreground">Path</span>
                            <code className="text-[10px] break-all opacity-80">{selectedProject.rootPath}</code>
                          </div>
                          <div className="flex items-center gap-4">
                            <div className="flex flex-col">
                              <span className="text-[9px] uppercase text-muted-foreground">Items</span>
                              <span className="text-xs font-mono">{openWorkItemCount}</span>
                            </div>
                            <div className="flex flex-col">
                              <span className="text-[9px] uppercase text-muted-foreground">Docs</span>
                              <span className="text-xs font-mono">{documents.length}</span>
                            </div>
                            <div className="flex flex-col">
                              <span className="text-[9px] uppercase text-muted-foreground">Blocked</span>
                              <span className="text-xs font-mono">{blockedWorkItemCount}</span>
                            </div>
                          </div>
                        </div>
                      </article>

                      <article className="overview-card border border-border bg-card/30 rounded p-4">
                        <div className="flex justify-between items-start mb-4">
                          <div>
                            <p className="text-[10px] uppercase text-muted-foreground mb-1">Runtime account</p>
                            <h4 className="text-sm font-bold">{selectedProjectLaunchLabel}</h4>
                          </div>
                          <Button
                            variant="outline"
                            size="sm"
                            className="h-6 text-[10px]"
                            onClick={() => setActiveView('settings')}
                          >
                            MANAGE
                          </Button>
                        </div>

                        <div className="space-y-4">
                          <select
                            className="w-full h-8 text-xs bg-background border border-border rounded px-2"
                            value={selectedLaunchProfileId === null ? '' : String(selectedLaunchProfileId)}
                            onChange={(event) =>
                              setSelectedLaunchProfileId(
                                event.target.value === ''
                                  ? null
                                  : Number(event.target.value),
                              )
                            }
                          >
                            <option value="">Choose account</option>
                            {launchProfiles.map((profile: any) => (
                              <option key={profile.id} value={profile.id}>
                                {profile.label}
                              </option>
                            ))}
                          </select>

                          {selectedLaunchProfile && (
                            <div className="space-y-1 opacity-60">
                              <div className="flex items-center gap-2">
                                <span className="text-[9px] uppercase w-12">Exec</span>
                                <code className="text-[10px]">{selectedLaunchProfile.executable}</code>
                              </div>
                              <div className="flex items-center gap-2">
                                <span className="text-[9px] uppercase w-12">Args</span>
                                <code className="text-[10px] truncate">{selectedLaunchProfile.args || 'None'}</code>
                              </div>
                            </div>
                          )}

                          <p className="text-[10px] text-muted-foreground">
                            Full launch-profile editing, defaults, and cleanup
                            automation live in the Settings tab.
                          </p>
                        </div>
                      </article>
                    </div>

                    {/* Recovery & Cleanup */}
                    <article className="border border-border bg-card/20 rounded p-4">
                      <div className="flex justify-between items-center mb-4">
                        <div>
                          <p className="text-[10px] uppercase text-muted-foreground mb-1">Supervisor health</p>
                          <h4 className="text-sm font-bold flex items-center gap-2">
                            Recovery & Cleanup
                            <Badge variant="default" className="text-[9px] h-4">
                              {recoveryActionCount} PENDING
                            </Badge>
                          </h4>
                        </div>
                        {recoveryActionCount > 0 && (
                          <Button
                            variant="outline"
                            size="sm"
                            className="h-7 text-[10px] uppercase tracking-wider"
                            onClick={() => void repairCleanupCandidates()}
                            disabled={isRepairingCleanup}
                          >
                            {isRepairingCleanup ? 'REPAIRING...' : 'REPAIR ALL SAFE'}
                          </Button>
                        )}
                      </div>

                      {recoveryActionCount === 0 ? (
                        <div className="text-center py-8 border border-dashed border-border rounded bg-black/10">
                          <p className="text-[10px] text-muted-foreground uppercase tracking-widest">No recovery actions required</p>
                        </div>
                      ) : (
                        <div className="space-y-4">
                          {orphanedSessions.length > 0 && (
                            <div className="space-y-2">
                              <span className="text-[9px] font-bold uppercase tracking-wider text-muted-foreground">Orphaned Sessions</span>
                              <div className="grid grid-cols-1 gap-2">
                                {orphanedSessions.map((session: any) => (
                                  <div key={session.id} className="flex items-center justify-between p-2 bg-card/40 border border-border rounded">
                                    <div className="flex flex-col">
                                      <span className="text-[11px] font-bold">Session #{session.id}</span>
                                      <span className="text-[9px] opacity-60 truncate max-w-[300px]">{session.rootPath}</span>
                                    </div>
                                    <Button
                                      variant="ghost"
                                      size="sm"
                                      className="h-7 text-[10px] text-destructive hover:text-destructive hover:bg-destructive/10"
                                      onClick={() => void terminateRecoveredSession(session.id)}
                                      disabled={activeOrphanSessionId === session.id}
                                    >
                                      CLEANUP
                                    </Button>
                                  </div>
                                ))}
                              </div>
                            </div>
                          )}

                          {(runtimeCleanupCandidates.length > 0 || staleWorktreeCleanupCandidates.length > 0) && (
                            <div className="space-y-2">
                              <span className="text-[9px] font-bold uppercase tracking-wider text-muted-foreground">Stale Artifacts</span>
                              <div className="grid grid-cols-1 gap-2">
                                {[...runtimeCleanupCandidates, ...staleWorktreeCleanupCandidates].map((candidate: any) => (
                                  <div key={candidate.path} className="flex items-center justify-between p-2 bg-card/40 border border-border rounded">
                                    <div className="flex flex-col">
                                      <span className="text-[11px] font-bold truncate max-w-[300px]">{candidate.path.split(/[\\\/]/).pop()}</span>
                                      <span className="text-[9px] opacity-60">{candidate.reason}</span>
                                    </div>
                                    <Button
                                      variant="ghost"
                                      size="sm"
                                      className="h-7 text-[10px]"
                                      onClick={() => void removeStaleArtifact(candidate)}
                                      disabled={activeCleanupPath === candidate.path}
                                    >
                                      REMOVE
                                    </Button>
                                  </div>
                                ))}
                              </div>
                            </div>
                          )}
                        </div>
                      )}
                    </article>

                    {/* Documents Preview */}
                    <article className="border border-border bg-card/20 rounded p-4">
                      <div className="flex justify-between items-center mb-4">
                        <div>
                          <p className="text-[10px] uppercase text-muted-foreground mb-1">Knowledge base</p>
                          <h4 className="text-sm font-bold">{documents.length} Documents</h4>
                        </div>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 text-[10px]"
                          onClick={() => setIsDocumentsManagerOpen(!isDocumentsManagerOpen)}
                        >
                          {isDocumentsManagerOpen ? 'CLOSE MANAGER' : 'MANAGE DOCUMENTS'}
                        </Button>
                      </div>

                      {isDocumentsManagerOpen ? (
                        <div className="bg-background/40 rounded p-2 border border-border">
                          <DocumentsPanel
                            documents={documents}
                            error={documentError}
                            isLoading={isLoadingDocuments}
                            onCreate={createDocument}
                            onDelete={deleteDocument}
                            onUpdate={updateDocument}
                            project={selectedProject}
                            workItems={workItems}
                          />
                        </div>
                      ) : (
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
                          {recentDocuments.map((doc: any) => (
                            <div key={doc.id} className="p-3 bg-card/40 border border-border rounded hover:border-primary/30 transition-colors">
                              <h5 className="text-[11px] font-bold truncate mb-1">{doc.title}</h5>
                              <span className="text-[9px] opacity-60">
                                {doc.workItemId ? `#${doc.workItemId}` : 'Project Global'}
                              </span>
                            </div>
                          ))}
                          {recentDocuments.length === 0 && (
                            <p className="text-[10px] text-muted-foreground italic col-span-full py-4 text-center">No documents indexed for this project.</p>
                          )}
                        </div>
                      )}
                    </article>
                  </div>
                </ScrollArea>
              ) : null}

              {activeView === 'settings' ? (
                <ScrollArea className="flex-1">
                  <div className="p-4">
                    <SettingsPanel state={state} actions={actions} />
                  </div>
                </ScrollArea>
              ) : null}

              {activeView === 'workItems' ? (
                <WorkItemsPanel
                  error={workItemError}
                  isLoading={isLoadingWorkItems}
                  onCreate={createWorkItem}
                  onDelete={deleteWorkItem}
                  onStartInTerminal={startWorkItemInTerminal}
                  onUpdate={updateWorkItem}
                  project={selectedProject}
                  startingWorkItemId={startingWorkItemId}
                  workItems={workItems}
                />
              ) : null}
            </>
          )}
        </section>

        {/* ── RIGHT PANEL: Dispatcher + Worktrees ── */}
        <aside
          key={`session-rail-${sessionRailRevision}`}
          className={`side-rail side-rail--sessions ${
            isSessionRailCollapsed ? 'side-rail--collapsed' : ''
          }`}
        >
          {isSessionRailCollapsed ? (
            <div className="side-rail__collapsed">
              <button
                className="rail-toggle"
                type="button"
                onClick={() => setIsSessionRailCollapsed(false)}
                title="Expand Sessions"
              >
                <ChevronLeft size={16} />
              </button>
            </div>
          ) : (
            <>
              <div className="side-rail__header">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => setIsSessionRailCollapsed(true)}
                  title="Collapse"
                  className="h-6 w-6"
                >
                  <ChevronRight size={14} />
                </Button>
                <span className="rail-label">SESSIONS</span>
              </div>

              {/* Live Sessions (Dispatcher) */}
              <section className="rail-section mt-2">
                <div className="rail-section__header mb-2">
                  <span className="text-[10px] uppercase text-muted-foreground tracking-wider">Live</span>
                </div>
                <button
                  className={`dispatcher-card ${
                    selectedTerminalWorktreeId === null && hasSelectedProjectLiveSession
                      ? 'dispatcher-card--active'
                      : ''
                  }`}
                  type="button"
                  onClick={() => selectMainTerminal()}
                >
                  <div className="dispatcher-card__row">
                    <span className="text-xs font-bold uppercase truncate flex-1">
                      {selectedProject?.name ?? 'Project'}
                    </span>
                    <Badge variant={isDispatcherRunning ? 'running' : 'offline'} className="h-4 text-[9px] px-1">
                      {isDispatcherRunning ? 'LIVE' : 'IDLE'}
                    </Badge>
                  </div>
                </button>
              </section>

              {/* Worktree Sessions */}
              <section className="rail-section mt-4">
                <div className="rail-section__header mb-2">
                  <span className="text-[10px] uppercase text-muted-foreground tracking-wider">Worktrees</span>
                  <span className="text-[10px] text-muted-foreground ml-auto">({worktreeSessions.length})</span>
                </div>

                <ScrollArea className="flex-1 -mx-1 px-1">
                  {worktreeSessions.length === 0 ? (
                    <div className="empty-state empty-state--rail text-center py-4 text-xs italic">
                      No active worktrees.
                    </div>
                  ) : (
                    <div
                      key={`worktree-session-list-${sessionRailRevision}`}
                      className="session-card-list space-y-1"
                    >
                      {worktreeSessions.map(
                        ({ worktree, snapshot }: any) => (
                          <button
                            key={worktree.id}
                            className={`session-card ${
                              selectedTerminalWorktreeId === worktree.id
                                ? 'session-card--active'
                                : ''
                            }`}
                            type="button"
                            onClick={() =>
                              selectWorktreeTerminal(worktree.id)
                            }
                          >
                            <div className="session-card__header">
                              <span className="text-xs font-bold truncate flex-1">{worktree.branchName}</span>
                              <Badge
                                variant={
                                  snapshot?.isRunning ? 'running' : 'offline'
                                }
                                className="h-3.5 text-[8px] px-1"
                              >
                                {snapshot?.isRunning ? 'LIVE' : 'IDLE'}
                              </Badge>
                            </div>
                            <p className="session-card__subtitle text-[10px] truncate opacity-70">
                              {worktree.workItemTitle}
                            </p>
                          </button>
                        ),
                      )}
                    </div>
                  )}
                </ScrollArea>
              </section>
            </>
          )}
        </aside>
      </section>
    </main>
  )
}

export default WorkspaceShell
