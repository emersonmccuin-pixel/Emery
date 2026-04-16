use super::{
    agent_message_store, agent_signal_store, session_store, AgentMessageRecord, AgentSignalRecord,
    AppState, AppendSessionEventInput, CreateSessionRecordInput, EmitAgentSignalInput,
    FinishSessionRecordInput, ListAgentMessagesFilter, RespondToAgentSignalInput,
    SendAgentMessageInput, SessionEventRecord, SessionRecord, UpdateSessionRuntimeMetadataInput,
};
use crate::error::{AppError, AppResult};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

impl AppState {
    pub fn current_agent_message_sequence(&self) -> u64 {
        self.agent_message_broker.current_sequence()
    }

    pub fn wait_for_agent_message_change(&self, observed_sequence: u64, timeout: Duration) -> bool {
        self.agent_message_broker
            .wait_for_change(observed_sequence, timeout)
    }

    pub fn create_session_record(
        &self,
        input: CreateSessionRecordInput,
    ) -> AppResult<SessionRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        session_store::create_record(&connection, input)
    }

    pub fn update_session_runtime_metadata(
        &self,
        input: UpdateSessionRuntimeMetadataInput,
    ) -> AppResult<SessionRecord> {
        let connection = self.connect()?;
        session_store::update_runtime_metadata(&connection, input)
    }

    pub fn update_session_heartbeat(&self, session_id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());
        session_store::update_heartbeat(&connection, session_id, &now)
    }

    pub fn update_session_provider_session_id(
        &self,
        session_id: i64,
        provider_session_id: Option<&str>,
    ) -> AppResult<SessionRecord> {
        let connection = self.connect()?;
        session_store::update_provider_session_id(&connection, session_id, provider_session_id)
    }

    pub fn finish_session_record(
        &self,
        input: FinishSessionRecordInput,
    ) -> AppResult<SessionRecord> {
        let connection = self.connect()?;
        session_store::finish_record(&connection, input)
    }

    pub fn append_session_event(
        &self,
        input: AppendSessionEventInput,
    ) -> AppResult<SessionEventRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        session_store::append_event(&connection, input)
    }

    pub fn emit_agent_signal(&self, input: EmitAgentSignalInput) -> AppResult<AgentSignalRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        agent_signal_store::emit(&connection, input)
    }

    pub fn list_agent_signals(
        &self,
        project_id: i64,
        worktree_id: Option<i64>,
        status: Option<&str>,
    ) -> AppResult<Vec<AgentSignalRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        agent_signal_store::list(&connection, project_id, worktree_id, status)
    }

    pub fn get_agent_signal(&self, id: i64, project_id: i64) -> AppResult<AgentSignalRecord> {
        let connection = self.connect()?;
        let signal = agent_signal_store::get(&connection, id)?;
        if signal.project_id != project_id {
            return Err(AppError::not_found(format!(
                "agent signal #{id} not found in project #{project_id}"
            )));
        }
        Ok(signal)
    }

    pub fn respond_to_agent_signal(
        &self,
        input: RespondToAgentSignalInput,
    ) -> AppResult<AgentSignalRecord> {
        let connection = self.connect()?;
        let signal = self.get_agent_signal(input.id, input.project_id)?;

        if signal.status == "responded" {
            return Err(AppError::conflict("signal has already been responded to"));
        }

        agent_signal_store::respond(&connection, input.id, &input.response)
    }

    pub fn acknowledge_agent_signal(
        &self,
        id: i64,
        project_id: i64,
    ) -> AppResult<AgentSignalRecord> {
        let connection = self.connect()?;
        let signal = self.get_agent_signal(id, project_id)?;

        if signal.status != "pending" {
            return Err(AppError::conflict(format!(
                "signal #{id} cannot be acknowledged in status '{}'",
                signal.status
            )));
        }

        agent_signal_store::acknowledge(&connection, id)
    }

    pub fn send_agent_message(
        &self,
        input: SendAgentMessageInput,
    ) -> AppResult<AgentMessageRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;

        let record = agent_message_store::send(&connection, input)?;
        self.agent_message_broker.notify_message();
        Ok(record)
    }

    pub fn list_agent_messages(
        &self,
        project_id: i64,
        filters: ListAgentMessagesFilter,
    ) -> AppResult<Vec<AgentMessageRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        agent_message_store::list(&connection, project_id, filters)
    }

    pub fn get_agent_inbox(
        &self,
        project_id: i64,
        agent_name: &str,
        unread_only: bool,
        from_agent: Option<String>,
        message_type: Option<String>,
        thread_id: Option<String>,
        reply_to_message_id: Option<i64>,
        limit: Option<i64>,
    ) -> AppResult<Vec<AgentMessageRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        agent_message_store::inbox(
            &connection,
            project_id,
            agent_name,
            unread_only,
            from_agent,
            message_type,
            thread_id,
            reply_to_message_id,
            limit,
        )
    }

    pub fn ack_agent_messages(&self, project_id: i64, message_ids: &[i64]) -> AppResult<()> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        agent_message_store::ack(&connection, project_id, message_ids)
    }

    pub fn reconcile_stale_messages(&self, project_id: i64) -> AppResult<i64> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        agent_message_store::reconcile_stale(&connection, project_id)
    }

    pub fn ack_messages_for_work_item(&self, project_id: i64, work_item_id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        agent_message_store::ack_for_work_item(&connection, project_id, work_item_id)
    }

    pub fn list_session_records(&self, project_id: i64) -> AppResult<Vec<SessionRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(session_store::list_records(&connection, project_id, None)?)
    }

    pub fn list_session_records_limited(
        &self,
        project_id: i64,
        limit: usize,
    ) -> AppResult<Vec<SessionRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(session_store::list_records(
            &connection,
            project_id,
            Some(limit),
        )?)
    }

    pub fn list_orphaned_session_records(&self, project_id: i64) -> AppResult<Vec<SessionRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(session_store::list_orphaned_records(
            &connection,
            project_id,
        )?)
    }

    pub fn get_session_record(&self, id: i64) -> AppResult<SessionRecord> {
        let connection = self.connect()?;
        session_store::get_record(&connection, id)
    }

    pub fn list_session_events(
        &self,
        project_id: i64,
        limit: usize,
    ) -> AppResult<Vec<SessionEventRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(session_store::list_events_by_project(
            &connection,
            project_id,
            limit,
        )?)
    }

    pub fn list_session_events_for_session(
        &self,
        session_id: i64,
        limit: usize,
    ) -> AppResult<Vec<SessionEventRecord>> {
        let connection = self.connect()?;
        self.get_session_record(session_id)?;
        Ok(session_store::list_events_by_session(
            &connection,
            session_id,
            limit,
        )?)
    }
}
