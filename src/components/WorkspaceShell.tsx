import { useState, Suspense, type FormEvent } from 'react'
import { Settings, Plus, ChevronLeft, ChevronRight } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import DocumentsPanel from './DocumentsPanel'
import LiveTerminal from './LiveTerminal'
import WorkItemsPanel from './WorkItemsPanel'

type WorkspaceShellProps = {
  state: any
  actions: any
}

function WorkspaceShell({ state, actions }: WorkspaceShellProps) {
  const {
    storageInfo,
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
    sessionRailRevision,
    hasSelectedProjectLiveSession,
    launchBlockedByMissingRoot,
    selectedProjectLaunchLabel,
    selectedTerminalLaunchLabel,
    projectName,
    projectRootPath,
    projectError,
    isProjectCreateOpen,
    editProjectName,
    editProjectRootPath,
    projectUpdateError,
    isProjectEditorOpen,
    profileLabel,
    profileExecutable,
    profileArgs,
    profileEnvJson,
    profileError,
    isProfileFormOpen,
    isDocumentsManagerOpen,
    isAgentGuideOpen,
    activeView,
    isProjectRailCollapsed,
    isSessionRailCollapsed,
    isCreatingProject,
    isUpdatingProject,
    isCreatingProfile,
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
    setProjectUpdateError,
    setSelectedLaunchProfileId,
    setProjectName,
    setProjectRootPath,
    setIsProjectCreateOpen,
    setEditProjectName,
    setEditProjectRootPath,
    setIsProjectEditorOpen,
    setProfileLabel,
    setProfileExecutable,
    setProfileArgs,
    setProfileEnvJson,
    setIsProfileFormOpen,
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
    submitProjectUpdate,
    submitLaunchProfile,
    launchWorkspaceGuide,
    stopSession,
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

  const [projectFilter, setProjectFilter] = useState('')

  const filteredProjects = projectFilter
    ? projects.filter((p: any) =>
        p.name.toLowerCase().includes(projectFilter.toLowerCase())
      )
    : projects

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
              >
                Projects
              </button>
              <button
                className="rail-toggle rail-toggle--accent"
                type="button"
                onClick={() => {
                  setIsProjectRailCollapsed(false)
                  setIsProjectCreateOpen(true)
                }}
              >
                +
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
                  >
                    <Plus size={14} />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => setIsProjectRailCollapsed(true)}
                    title="Collapse"
                  >
                    <ChevronLeft size={14} />
                  </Button>
                </div>
              </div>

              <div className="rail-filter">
                <Input
                  value={projectFilter}
                  onChange={(e) => setProjectFilter(e.target.value)}
                  placeholder="Filter..."
                />
              </div>

              <ScrollArea className="project-list project-list--minimal">
                {filteredProjects.length === 0 ? (
                  <div className="empty-state empty-state--rail">
                    {projects.length === 0
                      ? 'No projects yet.'
                      : 'No matches.'}
                  </div>
                ) : (
                  filteredProjects.map((project: any) => (
                    <button
                      key={project.id}
                      className={`project-card project-card--minimal ${
                        project.id === selectedProjectId
                          ? 'project-card--active'
                          : ''
                      }`}
                      type="button"
                      onClick={() => selectProject(project.id)}
                    >
                      <span className="project-card__name">
                        {project.name}
                      </span>
                    </button>
                  ))
                )}
              </ScrollArea>

              {isProjectCreateOpen ? (
                <form
                  className="stack-form stack-form--rail"
                  onSubmit={(event) =>
                    void submitProject(event as FormEvent<HTMLFormElement>)
                  }
                >
                  <div className="stack-form__header">
                    <h3>Add project</h3>
                  </div>

                  <label className="field">
                    <span>Name</span>
                    <input
                      value={projectName}
                      onChange={(event) => setProjectName(event.target.value)}
                      placeholder="Emery"
                    />
                  </label>

                  <label className="field">
                    <span>Root folder</span>
                    <div className="input-row">
                      <input
                        value={projectRootPath}
                        onChange={(event) =>
                          setProjectRootPath(event.target.value)
                        }
                        placeholder="E:\\Projects\\Emery"
                      />
                      <button
                        className="button button--secondary"
                        type="button"
                        onClick={() =>
                          browseForProjectFolder(
                            setProjectRootPath,
                            setProjectError,
                          )
                        }
                      >
                        Browse
                      </button>
                    </div>
                  </label>

                  {projectError ? (
                    <p className="form-error">{projectError}</p>
                  ) : null}

                  <div className="action-row">
                    <button
                      className="button button--primary"
                      disabled={isCreatingProject}
                      type="submit"
                    >
                      {isCreatingProject ? 'Saving...' : 'Create'}
                    </button>
                    <button
                      className="button button--secondary"
                      type="button"
                      onClick={() => setIsProjectCreateOpen(false)}
                    >
                      Cancel
                    </button>
                  </div>
                </form>
              ) : null}

              <div className="side-rail__footer">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => setActiveView('overview')}
                  title="Settings"
                >
                  <Settings size={14} />
                </Button>
              </div>
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
                    : 'Root missing. Rebind in Overview before launching.'}
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
              </nav>

              {activeView === 'terminal' ? (
                <section className="terminal-view">
                  <div className="terminal-view__toolbar">
                    <div className="terminal-view__summary">
                      <p className="panel__eyebrow">Terminal</p>
                      <strong>
                        {hasSelectedProjectLiveSession
                          ? sessionSnapshot.profileLabel
                          : selectedTerminalLaunchLabel}
                      </strong>
                      <p>
                        {selectedWorktree
                          ? hasSelectedProjectLiveSession
                            ? `Live session · ${selectedWorktree.branchName}`
                            : `Launch into worktree ${selectedWorktree.branchName}`
                          : hasSelectedProjectLiveSession
                            ? 'Live session · project root'
                            : 'Launch into project root'}
                      </p>
                    </div>

                    <div className="terminal-view__actions">
                      {!hasSelectedProjectLiveSession ? (
                        <label className="field field--inline">
                          <span>Account</span>
                          <select
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
                        </label>
                      ) : null}

                      <button
                        className="button button--secondary"
                        type="button"
                        onClick={() =>
                          setIsAgentGuideOpen((current: boolean) => !current)
                        }
                      >
                        {isAgentGuideOpen ? 'Hide guide' : 'Show guide'}
                      </button>

                      {hasSelectedProjectLiveSession ? (
                        <>
                          <button
                            className="button button--secondary"
                            disabled={!sessionSnapshot?.output}
                            type="button"
                            onClick={() => void copyTerminalOutput()}
                          >
                            Copy terminal
                          </button>
                          <button
                            className="button button--secondary"
                            disabled={isStoppingSession}
                            type="button"
                            onClick={() => void stopSession()}
                          >
                            {isStoppingSession ? 'Stopping...' : 'Stop session'}
                          </button>
                        </>
                      ) : (
                        <button
                          className="button button--primary"
                          disabled={
                            !selectedLaunchProfile ||
                            isLaunchingSession ||
                            launchBlockedByMissingRoot
                          }
                          type="button"
                          onClick={() => void launchWorkspaceGuide()}
                        >
                          {isLaunchingSession
                            ? 'Launching...'
                            : launchBlockedByMissingRoot
                              ? 'Rebind root'
                              : 'Launch terminal'}
                        </button>
                      )}
                    </div>
                  </div>

                  {isAgentGuideOpen ? (
                    <article className="guide-panel">
                      <div className="guide-panel__header">
                        <div>
                          <p className="panel__eyebrow">Startup guide</p>
                          <strong>{currentTerminalPromptLabel}</strong>
                        </div>
                        <span
                          className={`status-badge ${
                            bridgeReady
                              ? 'status-badge--ready'
                              : 'status-badge--stopped'
                          }`}
                        >
                          {bridgeReady
                            ? 'ready in terminal'
                            : 'launch to activate'}
                        </span>
                      </div>
                      <div className="guide-panel__actions">
                        <button
                          className="button button--secondary"
                          type="button"
                          onClick={() => void copyAgentStartupPrompt()}
                        >
                          {hasFocusedPrompt
                            ? 'Copy focus prompt'
                            : 'Copy prompt'}
                        </button>
                        <button
                          className="button button--primary"
                          disabled={!bridgeReady}
                          type="button"
                          onClick={() => void sendAgentStartupPrompt()}
                        >
                          {hasFocusedPrompt
                            ? 'Send focus prompt'
                            : 'Send to terminal'}
                        </button>
                        {hasFocusedPrompt ? (
                          <button
                            className="button button--secondary"
                            type="button"
                            onClick={() => setTerminalPromptDraft(null)}
                          >
                            Use workspace guide
                          </button>
                        ) : null}
                      </div>
                      {agentPromptMessage ? (
                        <p className="stack-form__note">
                          {agentPromptMessage}
                        </p>
                      ) : null}
                      <textarea
                        className="guide-panel__prompt"
                        readOnly
                        rows={8}
                        value={currentTerminalPrompt}
                      />
                      <div className="guide-panel__tools">
                        {projectCommanderTools.map((toolName: string) => (
                          <code key={toolName}>{toolName}</code>
                        ))}
                      </div>
                    </article>
                  ) : null}

                  {hasSelectedProjectLiveSession ? (
                    <Suspense
                      fallback={
                        <div className="terminal-loading">
                          Preparing terminal...
                        </div>
                      }
                    >
                      <LiveTerminal
                        snapshot={sessionSnapshot}
                        onSessionExit={handleSessionExit}
                      />
                    </Suspense>
                  ) : (
                    <div className="terminal-launch-card">
                      <div className="terminal-launch-card__copy">
                        <p className="panel__eyebrow">
                          {selectedWorktree
                            ? selectedWorktree.pathAvailable
                              ? 'Worktree ready'
                              : 'Worktree needs repair'
                            : selectedProject.rootAvailable
                              ? 'Ready to launch'
                              : 'Root needs rebind'}
                        </p>
                        <h3>
                          {selectedWorktree
                            ? selectedWorktree.branchName
                            : selectedProject.name}
                        </h3>
                        <p>
                          {selectedWorktree
                            ? selectedWorktree.pathAvailable
                              ? `Start Claude in ${selectedWorktree.worktreePath} for work item #${selectedWorktree.workItemId}.`
                              : 'The stored worktree path is missing. Start the work item again to recreate it.'
                            : selectedProject.rootAvailable
                              ? `Start Claude in ${selectedProject.rootPath} with the selected account.`
                              : 'The registered root path no longer exists. Rebind the project in Overview.'}
                        </p>
                      </div>
                      <div className="terminal-launch-card__meta">
                        <div className="terminal-launch-card__item">
                          <span>Account</span>
                          <strong>{selectedTerminalLaunchLabel}</strong>
                        </div>
                        <div className="terminal-launch-card__item">
                          <span>Open work</span>
                          <strong>{openWorkItemCount}</strong>
                        </div>
                        <div className="terminal-launch-card__item">
                          <span>Documents</span>
                          <strong>{documents.length}</strong>
                        </div>
                      </div>
                    </div>
                  )}
                </section>
              ) : null}

              {activeView === 'overview' ? (
                <section className="overview-view">
                  <div className="overview-grid">
                    <article className="overview-card overview-card--summary">
                      <div className="overview-card__header">
                        <div>
                          <p className="panel__eyebrow">Project status</p>
                          <strong>{selectedProject.name}</strong>
                        </div>
                      </div>
                      <div className="overview-metrics">
                        <div className="overview-metric overview-metric--wide">
                          <span>Root</span>
                          <code>{selectedProject.rootPath}</code>
                        </div>
                        {selectedWorktree ? (
                          <div className="overview-metric overview-metric--wide">
                            <span>Selected worktree</span>
                            <code>{selectedWorktree.worktreePath}</code>
                          </div>
                        ) : null}
                        <div className="overview-metric">
                          <span>Account</span>
                          <strong>{selectedProjectLaunchLabel}</strong>
                        </div>
                        <div className="overview-metric">
                          <span>Open work</span>
                          <strong>{openWorkItemCount}</strong>
                        </div>
                        <div className="overview-metric">
                          <span>Blocked</span>
                          <strong>{blockedWorkItemCount}</strong>
                        </div>
                        <div className="overview-metric">
                          <span>Documents</span>
                          <strong>{documents.length}</strong>
                        </div>
                        {storageInfo ? (
                          <div className="overview-metric overview-metric--wide">
                            <span>Shared DB</span>
                            <code>{storageInfo.dbPath}</code>
                          </div>
                        ) : null}
                      </div>
                    </article>

                    <article className="overview-card">
                      <div className="overview-card__header">
                        <div>
                          <p className="panel__eyebrow">Project settings</p>
                          <strong>
                            {selectedProject.rootAvailable
                              ? 'Edit project'
                              : 'Rebind root'}
                          </strong>
                        </div>
                        <button
                          className="button button--secondary button--compact"
                          type="button"
                          onClick={() =>
                            setIsProjectEditorOpen(
                              (current: boolean) => !current,
                            )
                          }
                        >
                          {isProjectEditorOpen
                            ? 'Hide'
                            : selectedProject.rootAvailable
                              ? 'Edit project'
                              : 'Rebind root'}
                        </button>
                      </div>

                      {isProjectEditorOpen ? (
                        <form
                          className="stack-form"
                          onSubmit={(event) =>
                            void submitProjectUpdate(
                              event as FormEvent<HTMLFormElement>,
                            )
                          }
                        >
                          {!selectedProject.rootAvailable ? (
                            <p className="form-error">
                              The registered root is missing. Pick the new
                              folder and save to repair launch.
                            </p>
                          ) : null}

                          <div className="field-grid">
                            <label className="field">
                              <span>Name</span>
                              <input
                                value={editProjectName}
                                onChange={(event) =>
                                  setEditProjectName(event.target.value)
                                }
                                placeholder="Emery"
                              />
                            </label>

                            <label className="field">
                              <span>Root folder</span>
                              <div className="input-row">
                                <input
                                  value={editProjectRootPath}
                                  onChange={(event) =>
                                    setEditProjectRootPath(event.target.value)
                                  }
                                  placeholder="E:\\Projects\\Emery"
                                />
                                <button
                                  className="button button--secondary"
                                  type="button"
                                  onClick={() =>
                                    browseForProjectFolder(
                                      setEditProjectRootPath,
                                      setProjectUpdateError,
                                    )
                                  }
                                >
                                  Browse
                                </button>
                              </div>
                            </label>
                          </div>

                          {projectUpdateError ? (
                            <p className="form-error">{projectUpdateError}</p>
                          ) : null}

                          <div className="action-row">
                            <button
                              className="button button--primary"
                              disabled={isUpdatingProject}
                              type="submit"
                            >
                              {isUpdatingProject
                                ? 'Saving...'
                                : selectedProject.rootAvailable
                                  ? 'Save changes'
                                  : 'Rebind project'}
                            </button>
                            <button
                              className="button button--secondary"
                              type="button"
                              onClick={() => setIsProjectEditorOpen(false)}
                            >
                              Cancel
                            </button>
                          </div>
                        </form>
                      ) : (
                        <p className="stack-form__note">
                          Keep the project name clean and rebind the root here
                          if the folder moves.
                        </p>
                      )}
                    </article>

                    <article className="overview-card">
                      <div className="overview-card__header">
                        <div>
                          <p className="panel__eyebrow">Launch profile</p>
                          <strong>{selectedProjectLaunchLabel}</strong>
                        </div>
                        <button
                          className="button button--secondary button--compact"
                          type="button"
                          onClick={() =>
                            setIsProfileFormOpen(
                              (current: boolean) => !current,
                            )
                          }
                        >
                          {isProfileFormOpen ? 'Hide add form' : 'Add account'}
                        </button>
                      </div>

                      <label className="field">
                        <span>Selected account</span>
                        <select
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
                      </label>

                      {isProfileFormOpen ? (
                        <form
                          className="stack-form"
                          onSubmit={(event) =>
                            void submitLaunchProfile(
                              event as FormEvent<HTMLFormElement>,
                            )
                          }
                        >
                          <label className="field">
                            <span>Label</span>
                            <input
                              value={profileLabel}
                              onChange={(event) =>
                                setProfileLabel(event.target.value)
                              }
                              placeholder="Claude Code / Work"
                            />
                          </label>

                          <label className="field">
                            <span>Executable</span>
                            <input
                              value={profileExecutable}
                              onChange={(event) =>
                                setProfileExecutable(event.target.value)
                              }
                              placeholder="claude"
                            />
                          </label>

                          <label className="field">
                            <span>Args</span>
                            <input
                              value={profileArgs}
                              onChange={(event) =>
                                setProfileArgs(event.target.value)
                              }
                              placeholder="--dangerously-skip-permissions"
                            />
                          </label>

                          <label className="field">
                            <span>Environment JSON</span>
                            <textarea
                              rows={4}
                              value={profileEnvJson}
                              onChange={(event) =>
                                setProfileEnvJson(event.target.value)
                              }
                              placeholder='{"ANTHROPIC_API_KEY":"..."}'
                            />
                          </label>

                          {profileError ? (
                            <p className="form-error">{profileError}</p>
                          ) : null}

                          <div className="action-row">
                            <button
                              className="button button--primary"
                              disabled={isCreatingProfile}
                              type="submit"
                            >
                              {isCreatingProfile
                                ? 'Saving...'
                                : 'Create profile'}
                            </button>
                            <button
                              className="button button--secondary"
                              type="button"
                              onClick={() => setIsProfileFormOpen(false)}
                            >
                              Cancel
                            </button>
                          </div>
                        </form>
                      ) : selectedLaunchProfile ? (
                        <div className="overview-inline-meta">
                          <code>{selectedLaunchProfile.executable}</code>
                          <code>
                            {selectedLaunchProfile.args || '(no args)'}
                          </code>
                        </div>
                      ) : (
                        <p className="stack-form__note">
                          Create or pick an account before launching Claude.
                        </p>
                      )}
                    </article>

                    <article className="overview-card overview-card--full">
                      <div className="overview-card__header">
                        <div>
                          <p className="panel__eyebrow">Context docs</p>
                          <strong>
                            {documents.length} documents attached to this
                            project
                          </strong>
                        </div>
                        <button
                          className="button button--secondary button--compact"
                          type="button"
                          onClick={() =>
                            setIsDocumentsManagerOpen(
                              (current: boolean) => !current,
                            )
                          }
                        >
                          {isDocumentsManagerOpen
                            ? 'Hide manager'
                            : 'Manage docs'}
                        </button>
                      </div>

                      {isDocumentsManagerOpen ? (
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
                      ) : recentDocuments.length > 0 ? (
                        <div className="document-preview-list">
                          {recentDocuments.map((document: any) => (
                            <article
                              key={document.id}
                              className="document-preview-card"
                            >
                              <strong>{document.title}</strong>
                              <p>
                                {document.workItemId === null
                                  ? 'Project-level context'
                                  : `Linked to work item #${document.workItemId}`}
                              </p>
                            </article>
                          ))}
                        </div>
                      ) : (
                        <div className="empty-state empty-state--rail">
                          No documents yet for this project.
                        </div>
                      )}
                    </article>
                  </div>
                </section>
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
              >
                {selectedProject?.name ?? 'Sessions'}
              </button>
            </div>
          ) : (
            <>
              <div className="side-rail__header">
                <span className="rail-label">
                  {selectedProject?.name?.toUpperCase() ?? 'NO PROJECT'}
                </span>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => setIsSessionRailCollapsed(true)}
                  title="Collapse"
                >
                  <ChevronRight size={14} />
                </Button>
              </div>

              {/* Dispatcher */}
              <section className="rail-section">
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
                    <span className="rail-label">DISPATCHER</span>
                    <Badge variant={isDispatcherRunning ? 'running' : 'offline'}>
                      {isDispatcherRunning ? 'RUNNING' : 'OFFLINE'}
                    </Badge>
                  </div>
                </button>
              </section>

              {/* Worktrees */}
              <section className="rail-section">
                <div className="rail-section__header">
                  <span className="rail-label">WORKTREES</span>
                  <span className="panel__count">
                    ({worktreeSessions.length})
                  </span>
                </div>

                {worktreeSessions.length === 0 ? (
                  <div className="empty-state empty-state--rail">
                    No worktrees yet.
                  </div>
                ) : (
                  <div
                    key={`worktree-session-list-${sessionRailRevision}`}
                    className="session-card-list"
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
                            <strong>{worktree.branchName}</strong>
                            <Badge
                              variant={
                                snapshot?.isRunning ? 'running' : 'offline'
                              }
                            >
                              {snapshot?.isRunning ? 'RUNNING' : 'OFFLINE'}
                            </Badge>
                          </div>
                          <p className="session-card__subtitle">
                            {worktree.workItemTitle}
                          </p>
                        </button>
                      ),
                    )}
                  </div>
                )}
              </section>
            </>
          )}
        </aside>
      </section>
    </main>
  )
}

export default WorkspaceShell
