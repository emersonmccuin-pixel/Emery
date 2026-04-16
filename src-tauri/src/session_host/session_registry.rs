use super::*;

impl Default for SessionRegistry {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            launching: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl Drop for SessionLaunchGuard {
    fn drop(&mut self) {
        if let Ok(mut launching) = self.launching.lock() {
            launching.remove(&self.target_key);
        }
    }
}

impl SessionTargetKey {
    fn from_target(target: &ProjectSessionTarget) -> Self {
        Self {
            project_id: target.project_id,
            worktree_id: target.worktree_id,
        }
    }

    fn from_launch_input(input: &LaunchSessionInput) -> Self {
        Self {
            project_id: input.project_id,
            worktree_id: input.worktree_id,
        }
    }
}

impl SessionRegistry {
    pub fn snapshot(&self, target: ProjectSessionTarget) -> AppResult<Option<SessionSnapshot>> {
        let session = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| "failed to access session registry".to_string())?;

            sessions
                .get(&SessionTargetKey::from_target(&target))
                .cloned()
        };

        Ok(session.map(|session| session.snapshot()))
    }

    pub fn poll_output(&self, input: SessionPollInput) -> AppResult<Option<SessionPollOutput>> {
        let session = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| "failed to access session registry".to_string())?;

            sessions
                .get(&SessionTargetKey::from_target(&ProjectSessionTarget {
                    project_id: input.project_id,
                    worktree_id: input.worktree_id,
                }))
                .cloned()
        };

        Ok(session.map(|session| session.poll_output(input.offset)))
    }

    pub fn list_running_snapshots(&self, project_id: i64) -> AppResult<Vec<SessionSnapshot>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to access session registry".to_string())?;

        let mut snapshots = sessions
            .values()
            .filter(|session| session.project_id == project_id && session.is_running())
            .map(|session| session.snapshot())
            .collect::<Vec<_>>();

        snapshots.sort_by(|left, right| right.started_at.cmp(&left.started_at));

        Ok(snapshots)
    }

    pub fn launch(
        &self,
        input: LaunchSessionInput,
        app_state: &AppState,
        supervisor_runtime: &SupervisorRuntimeInfo,
        source: &str,
    ) -> AppResult<SessionSnapshot> {
        let target_key = SessionTargetKey::from_launch_input(&input);
        let _launch_reservation = match self.acquire_launch_reservation(&target_key)? {
            LaunchReservation::Existing(existing) => {
                log::info!(
                    "session reattached — session_id={} project_id={} worktree_id={:?} profile={} root={} requested_by={}",
                    existing.session_record_id,
                    existing.project_id,
                    existing.worktree_id,
                    existing.profile_label,
                    existing.root_path,
                    source
                );
                try_append_session_event(
                    app_state,
                    existing.project_id,
                    Some(existing.session_record_id),
                    "session.reattached",
                    Some("session"),
                    Some(existing.session_record_id),
                    source,
                    &json!({
                        "projectId": existing.project_id,
                        "worktreeId": existing.worktree_id,
                        "launchProfileId": existing.launch_profile_id,
                        "profileLabel": existing.profile_label.clone(),
                        "rootPath": existing.root_path.clone(),
                        "startedAt": existing.started_at.clone(),
                    }),
                );
                return Ok(existing.snapshot());
            }
            LaunchReservation::Reserved(reservation) => reservation,
        };

        let project = app_state.get_project(input.project_id)?;
        let worktree = match input.worktree_id {
            Some(worktree_id) => {
                let worktree = app_state.get_worktree(worktree_id)?;

                if worktree.project_id != input.project_id {
                    return Err(AppError::invalid_input(format!(
                        "worktree #{worktree_id} does not belong to project #{}",
                        input.project_id
                    )));
                }

                Some(worktree)
            }
            None => None,
        };
        let profile = app_state.get_launch_profile(input.launch_profile_id)?;
        let started_at = now_timestamp_string();
        let startup_prompt = input
            .startup_prompt
            .as_deref()
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_default();
        let resume_session_id = input
            .resume_session_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let launch_root_path = worktree
            .as_ref()
            .map(|record| record.worktree_path.clone())
            .unwrap_or_else(|| project.root_path.clone());
        let is_resume_launch = resume_session_id.is_some()
            && matches!(
                profile.provider.as_str(),
                "claude_code" | "claude_agent_sdk" | "codex_sdk"
            );
        let provider_session_id =
            resolve_provider_session_id(profile.provider.as_str(), resume_session_id.as_deref());
        let launch_mode = if is_resume_launch { "resume" } else { "fresh" };
        let startup_prompt = if is_resume_launch {
            String::new()
        } else {
            startup_prompt
        };

        log::info!(
            "session launch requested — project_id={} worktree_id={:?} launch_profile_id={} profile={} root={} requested_by={} launch_mode={} provider_session_id={} has_startup_prompt={} model={} execution_mode={}",
            input.project_id,
            input.worktree_id,
            input.launch_profile_id,
            profile.label,
            launch_root_path,
            source,
            launch_mode,
            provider_session_id.as_deref().unwrap_or("none"),
            !startup_prompt.is_empty() && !is_resume_launch,
            input.model.as_deref().unwrap_or("default"),
            input.execution_mode.as_deref().unwrap_or("default")
        );

        if !Path::new(&launch_root_path).is_dir() {
            log::warn!(
                "session launch rejected — project_id={} worktree_id={:?} launch_profile_id={} root={} requested_by={} reason=missing_root",
                input.project_id,
                input.worktree_id,
                input.launch_profile_id,
                launch_root_path,
                source
            );
            return Err(if worktree.is_some() {
                AppError::not_found(
                    "selected worktree path no longer exists. Recreate the worktree before launching.",
                )
            } else {
                AppError::not_found(
                    "selected project root folder no longer exists. Rebind the project before launching.",
                )
            });
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: input.rows.max(10),
                cols: input.cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("failed to open pty: {error}"))?;

        let session_record = app_state.create_session_record(CreateSessionRecordInput {
            project_id: input.project_id,
            launch_profile_id: Some(input.launch_profile_id),
            worktree_id: input.worktree_id,
            process_id: None,
            supervisor_pid: None,
            provider: profile.provider.clone(),
            provider_session_id: provider_session_id.clone(),
            profile_label: profile.label.clone(),
            root_path: launch_root_path.clone(),
            state: "running".to_string(),
            startup_prompt: startup_prompt.clone(),
            started_at: started_at.clone(),
        })?;
        let mut launch_artifacts_guard =
            SessionLaunchArtifactsGuard::new(app_state.storage(), session_record.id);

        let app_settings = app_state.get_app_settings()?;
        let mut parsed_launch_env = match parse_launch_profile_env(&profile.env_json) {
            Ok(parsed) => parsed,
            Err(error) => {
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error,
                );
                return Err(error.into());
            }
        };
        parsed_launch_env.vault_bindings = match merge_launch_vault_bindings(
            parsed_launch_env.vault_bindings,
            &input.vault_env_bindings,
        ) {
            Ok(bindings) => bindings,
            Err(error) => {
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error,
                );
                return Err(error.into());
            }
        };
        let resolved_launch_bindings = match app_state
            .resolve_vault_access_bindings(
                parsed_launch_env.vault_bindings,
                source,
                Some(session_record.id),
                &format!("session_launch:{}", profile.provider),
            )
        {
            Ok(bindings) => bindings,
            Err(error) => {
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error.to_string(),
                );
                return Err(error);
            }
        };
        let resolved_launch_env = match ResolvedLaunchProfileEnv::materialize(
            parsed_launch_env.literal_env,
            resolved_launch_bindings,
            &app_state.storage(),
            session_record.id,
        ) {
            Ok(env) => env,
            Err(error) => {
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error,
                );
                return Err(error.into());
            }
        };
        let command = match build_launch_command(
            &project,
            worktree.as_ref(),
            &launch_root_path,
            &profile,
            &resolved_launch_env,
            &app_settings,
            &app_state.storage(),
            supervisor_runtime,
            (!startup_prompt.is_empty()).then_some(startup_prompt.as_str()),
            session_record.provider_session_id.as_deref(),
            is_resume_launch,
            session_record.id,
            input.model.as_deref(),
            input.execution_mode.as_deref(),
        ) {
            Ok(command) => command,
            Err(error) => {
                log::error!(
                    "session launch failed — stage=build_command project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} error={}",
                    project.id,
                    input.worktree_id,
                    profile.id,
                    session_record.id,
                    profile.label,
                    launch_root_path,
                    source,
                    error
                );
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error,
                );
                return Err(error.into());
            }
        };
        if let Err(error) = record_session_launch_vault_access_audit(
            app_state,
            &resolved_launch_env,
            &profile.provider,
            session_record.id,
        ) {
            log::error!(
                "session launch failed — stage=record_vault_access_audit project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} error={}",
                project.id,
                input.worktree_id,
                profile.id,
                session_record.id,
                profile.label,
                launch_root_path,
                source,
                error
            );
            mark_session_launch_failed(
                app_state,
                &project,
                &profile,
                &launch_root_path,
                input.worktree_id,
                &provider_session_id,
                launch_mode,
                source,
                session_record.id,
                &error.to_string(),
            );
            return Err(error);
        }
        let child = match pair.slave.spawn_command(command) {
            Ok(child) => child,
            Err(error) => {
                log::error!(
                    "session launch failed — stage=spawn_command project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} error={}",
                    project.id,
                    input.worktree_id,
                    profile.id,
                    session_record.id,
                    profile.label,
                    launch_root_path,
                    source,
                    error
                );
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error.to_string(),
                );
                return Err(AppError::supervisor(format!(
                    "failed to launch session: {error}"
                )));
            }
        };

        let mut killer = child.clone_killer();
        let child_process_id = child.process_id();
        let reader = match pair.master.try_clone_reader() {
            Ok(reader) => reader,
            Err(error) => {
                log::error!(
                    "session launch failed — stage=open_pty_reader project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} process_id={:?} error={}",
                    project.id,
                    input.worktree_id,
                    profile.id,
                    session_record.id,
                    profile.label,
                    launch_root_path,
                    source,
                    child_process_id,
                    error
                );
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error.to_string(),
                );
                if let Err(termination_error) =
                    terminate_failed_launch_process(&mut killer, child_process_id)
                {
                    log::warn!(
                        "failed to terminate session process after reader setup error — session_id={} profile={} process_id={:?} error={}",
                        session_record.id,
                        profile.label,
                        child_process_id,
                        termination_error
                    );
                }
                return Err(AppError::supervisor(format!(
                    "failed to open pty reader: {error}"
                )));
            }
        };
        let writer = match pair.master.take_writer() {
            Ok(writer) => writer,
            Err(error) => {
                log::error!(
                    "session launch failed — stage=open_pty_writer project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} process_id={:?} error={}",
                    project.id,
                    input.worktree_id,
                    profile.id,
                    session_record.id,
                    profile.label,
                    launch_root_path,
                    source,
                    child_process_id,
                    error
                );
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error.to_string(),
                );
                if let Err(termination_error) =
                    terminate_failed_launch_process(&mut killer, child_process_id)
                {
                    log::warn!(
                        "failed to terminate session process after writer setup error — session_id={} profile={} process_id={:?} error={}",
                        session_record.id,
                        profile.label,
                        child_process_id,
                        termination_error
                    );
                }
                return Err(AppError::supervisor(format!(
                    "failed to open pty writer: {error}"
                )));
            }
        };
        let process_id = child_process_id.map(i64::from);

        if let Err(error) =
            app_state.update_session_runtime_metadata(UpdateSessionRuntimeMetadataInput {
                id: session_record.id,
                process_id,
                supervisor_pid: Some(i64::from(supervisor_runtime.pid)),
            })
        {
            log::error!(
                "session launch failed — stage=persist_runtime_metadata project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} process_id={:?} error={}",
                project.id,
                input.worktree_id,
                profile.id,
                session_record.id,
                profile.label,
                launch_root_path,
                source,
                process_id,
                error
            );
            mark_session_launch_failed(
                app_state,
                &project,
                &profile,
                &launch_root_path,
                input.worktree_id,
                &provider_session_id,
                launch_mode,
                source,
                session_record.id,
                &error.to_string(),
            );
            if let Err(termination_error) =
                terminate_failed_launch_process(&mut killer, child.process_id())
            {
                log::warn!(
                    "failed to terminate session process after metadata persistence error — session_id={} profile={} process_id={:?} error={}",
                    session_record.id,
                    profile.label,
                    child.process_id(),
                    termination_error
                );
            }
            return Err(AppError::database(format!(
                "failed to persist session runtime metadata: {error}"
            )));
        }
        let vault_env_var_names = resolved_launch_env.vault_env_var_names();
        let vault_file_env_var_names = resolved_launch_env.file_env_var_names();
        let vault_env_entry_names = resolved_launch_env.env_vault_entry_names();
        let vault_file_entry_names = resolved_launch_env.file_vault_entry_names();
        let vault_env_binding_count = resolved_launch_env.env_binding_count();
        let vault_file_binding_count = resolved_launch_env.file_binding_count();
        let session_killer = child.clone_killer();

        let initial_activity = if startup_prompt.is_empty() {
            "session launched (idle)".to_string()
        } else {
            format!("startup prompt: {}", truncate_for_log(&startup_prompt, 200))
        };

        let session = Arc::new(HostedSession {
            session_record_id: session_record.id,
            project_id: input.project_id,
            worktree_id: input.worktree_id,
            launch_profile_id: input.launch_profile_id,
            profile_label: profile.label.clone(),
            root_path: launch_root_path.clone(),
            started_at,
            startup_prompt: startup_prompt.clone(),
            storage: app_state.storage(),
            last_activity: Mutex::new(initial_activity),
            output_state: Mutex::new(OutputBufferState {
                buffer: String::new(),
                start_offset: 0,
                end_offset: 0,
            }),
            exit_state: Mutex::new(None),
            child: Mutex::new(child),
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            killer: Mutex::new(session_killer),
        });

        {
            let mut sessions = match self.sessions.lock() {
                Ok(sessions) => sessions,
                Err(_) => {
                    let error = "failed to register session".to_string();
                    log::error!(
                        "session launch failed — stage=register_session project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} process_id={:?} error={}",
                        project.id,
                        input.worktree_id,
                        profile.id,
                        session_record.id,
                        profile.label,
                        launch_root_path,
                        source,
                        child_process_id,
                        error
                    );
                    mark_session_launch_failed(
                        app_state,
                        &project,
                        &profile,
                        &launch_root_path,
                        input.worktree_id,
                        &provider_session_id,
                        launch_mode,
                        source,
                        session_record.id,
                        &error,
                    );
                    if let Err(termination_error) =
                        terminate_failed_launch_process(&mut killer, child_process_id)
                    {
                        log::warn!(
                            "failed to terminate session process after registration error — session_id={} profile={} process_id={:?} error={}",
                            session_record.id,
                            profile.label,
                            child_process_id,
                            termination_error
                        );
                    }
                    return Err(AppError::supervisor(error));
                }
            };
            sessions.insert(target_key, Arc::clone(&session));
        }
        launch_artifacts_guard.disarm();

        try_append_session_event(
            app_state,
            project.id,
            Some(session_record.id),
            "session.launched",
            Some("session"),
            Some(session_record.id),
            "supervisor_runtime",
            &json!({
                "projectId": project.id,
                "worktreeId": input.worktree_id,
                "launchProfileId": profile.id,
                "profileLabel": session.profile_label.clone(),
                "provider": profile.provider,
                "providerSessionId": session_record.provider_session_id.clone(),
                "launchMode": launch_mode,
                "rootPath": launch_root_path,
                "processId": process_id,
                "supervisorPid": supervisor_runtime.pid,
                "startedAt": session.started_at.clone(),
                "hasStartupPrompt": !session_record.startup_prompt.is_empty(),
                "requestedBy": source,
            }),
        );

        if vault_env_binding_count > 0 {
            try_append_session_event(
                app_state,
                project.id,
                Some(session_record.id),
                "session.vault_env_injected",
                Some("session"),
                Some(session_record.id),
                "supervisor_runtime",
                &json!({
                    "projectId": project.id,
                    "worktreeId": input.worktree_id,
                    "launchProfileId": profile.id,
                    "profileLabel": session.profile_label.clone(),
                    "provider": profile.provider,
                    "sessionId": session_record.id,
                    "envVars": vault_env_var_names,
                    "vaultEntries": vault_env_entry_names,
                    "secretCount": vault_env_binding_count,
                    "correlationId": format!("session-launch:{}", session_record.id),
                }),
            );
        }

        if vault_file_binding_count > 0 {
            try_append_session_event(
                app_state,
                project.id,
                Some(session_record.id),
                "session.vault_file_injected",
                Some("session"),
                Some(session_record.id),
                "supervisor_runtime",
                &json!({
                    "projectId": project.id,
                    "worktreeId": input.worktree_id,
                    "launchProfileId": profile.id,
                    "profileLabel": session.profile_label.clone(),
                    "provider": profile.provider,
                    "sessionId": session_record.id,
                    "envVars": vault_file_env_var_names,
                    "vaultEntries": vault_file_entry_names,
                    "secretCount": vault_file_binding_count,
                    "correlationId": format!("session-launch:{}", session_record.id),
                }),
            );
        }

        log::info!(
            "session launched — session_id={} project_id={} worktree_id={:?} profile={} root={} pid={:?} requested_by={}",
            session_record.id,
            session.project_id,
            session.worktree_id,
            session.profile_label,
            session.root_path,
            process_id,
            source
        );

        session_runtime_watch::spawn_output_thread(
            Arc::clone(&session),
            reader,
            resolved_launch_env.into_redaction_rules(),
        );
        session_runtime_watch::spawn_exit_watch_thread(Arc::clone(&session), app_state.clone());

        Ok(session.snapshot())
    }

    pub fn write_input(&self, input: SessionInput) -> AppResult<()> {
        let session = self.get_running_session(&ProjectSessionTarget {
            project_id: input.project_id,
            worktree_id: input.worktree_id,
        })?;

        let clean = strip_ansi_escapes(&input.data);
        let trimmed = clean.trim();
        if !trimmed.is_empty() && trimmed.len() > 1 {
            if let Ok(mut activity) = session.last_activity.lock() {
                *activity = format!("user input: {}", truncate_for_log(trimmed, 300));
            }
        }

        let mut writer = session
            .writer
            .lock()
            .map_err(|_| "failed to access session writer".to_string())?;

        writer.write_all(input.data.as_bytes()).map_err(|error| {
            AppError::supervisor(format!("failed to write to session: {error}"))
        })?;
        writer.flush().map_err(|error| {
            AppError::supervisor(format!("failed to flush session input: {error}"))
        })
    }

    pub fn resize(&self, input: ResizeSessionInput) -> AppResult<()> {
        let session = self.get_running_session(&ProjectSessionTarget {
            project_id: input.project_id,
            worktree_id: input.worktree_id,
        })?;
        let master = session
            .master
            .lock()
            .map_err(|_| "failed to access pty for resize".to_string())?;

        master
            .resize(PtySize {
                rows: input.rows.max(10),
                cols: input.cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| AppError::supervisor(format!("failed to resize session: {error}")))
    }

    pub fn terminate(
        &self,
        target: ProjectSessionTarget,
        app_state: &AppState,
        source: &str,
    ) -> AppResult<()> {
        let session = self.get_running_session(&target)?;
        log::info!(
            "session terminate requested — session_id={} project_id={} worktree_id={:?} root={} requested_by={}",
            session.session_record_id,
            session.project_id,
            session.worktree_id,
            session.root_path,
            source
        );
        let mut killer = session
            .killer
            .lock()
            .map_err(|_| "failed to access session killer".to_string())?;

        try_append_session_event(
            app_state,
            session.project_id,
            Some(session.session_record_id),
            "session.terminate_requested",
            Some("session"),
            Some(session.session_record_id),
            source,
            &json!({
                "projectId": session.project_id,
                "worktreeId": session.worktree_id,
                "launchProfileId": session.launch_profile_id,
                "profileLabel": session.profile_label.clone(),
                "rootPath": session.root_path.clone(),
                "startedAt": session.started_at.clone(),
            }),
        );

        killer
            .kill()
            .or_else(|error| {
                #[cfg(windows)]
                if try_taskkill(session.process_id()).is_ok() {
                    return Ok(());
                }

                if session
                    .try_update_exit_from_child(app_state)
                    .unwrap_or(false)
                {
                    return Ok(());
                }

                try_append_session_event(
                    app_state,
                    session.project_id,
                    Some(session.session_record_id),
                    "session.terminate_failed",
                    Some("session"),
                    Some(session.session_record_id),
                    "supervisor_runtime",
                    &json!({
                        "projectId": session.project_id,
                        "worktreeId": session.worktree_id,
                        "sessionRecordId": session.session_record_id,
                        "rootPath": session.root_path.clone(),
                        "error": error.to_string(),
                        "requestedBy": source,
                    }),
                );

                log::error!(
                    "session terminate failed — session_id={} project_id={} worktree_id={:?} requested_by={} error={}",
                    session.session_record_id,
                    session.project_id,
                    session.worktree_id,
                    source,
                    error
                );
                Err(error)
            })
            .map_err(|error| AppError::supervisor(format!("failed to terminate session: {error}")))?;

        let exit_state = session.current_exit_state().unwrap_or(ExitState {
            exit_code: 127,
            success: false,
            error: None,
        });
        session_runtime_watch::force_record_session_exit(
            &session,
            app_state,
            exit_state.exit_code,
            false,
            "session.terminated",
            Some("terminated"),
            Some("terminated by supervisor"),
        );

        log::info!(
            "session terminated by supervisor — session_id={} project_id={} worktree_id={:?} requested_by={}",
            session.session_record_id,
            session.project_id,
            session.worktree_id,
            source
        );

        Ok(())
    }

    fn get_session(&self, target_key: &SessionTargetKey) -> AppResult<Option<Arc<HostedSession>>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to access session registry".to_string())?;

        Ok(sessions.get(target_key).cloned())
    }

    pub(super) fn acquire_launch_reservation(
        &self,
        target_key: &SessionTargetKey,
    ) -> AppResult<LaunchReservation> {
        let wait_started = Instant::now();

        loop {
            if let Some(existing) = self.get_session(target_key)? {
                if existing.is_running() {
                    return Ok(LaunchReservation::Existing(existing));
                }
            }

            {
                let mut launching = self
                    .launching
                    .lock()
                    .map_err(|_| "failed to access session launch reservations".to_string())?;

                if !launching.contains(target_key) {
                    launching.insert(target_key.clone());
                    return Ok(LaunchReservation::Reserved(SessionLaunchGuard {
                        launching: Arc::clone(&self.launching),
                        target_key: target_key.clone(),
                    }));
                }
            }

            if wait_started.elapsed() >= SESSION_LAUNCH_WAIT_TIMEOUT {
                return Err(AppError::supervisor(
                    "session launch is already in progress for this target",
                ));
            }

            std::thread::sleep(SESSION_LAUNCH_WAIT_INTERVAL);
        }
    }

    fn get_running_session(&self, target: &ProjectSessionTarget) -> AppResult<Arc<HostedSession>> {
        let session = self
            .get_session(&SessionTargetKey::from_target(target))?
            .ok_or_else(|| {
                AppError::not_found(build_missing_session_message(target.worktree_id))
            })?;

        if session.is_running() {
            Ok(session)
        } else {
            Err(AppError::not_found(build_missing_session_message(
                target.worktree_id,
            )))
        }
    }
}

fn build_missing_session_message(worktree_id: Option<i64>) -> String {
    match worktree_id {
        Some(worktree_id) => format!("no live session for worktree #{worktree_id}"),
        None => "no live session for that project".to_string(),
    }
}
