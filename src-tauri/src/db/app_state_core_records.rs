use super::{
    app_settings_store, document_store, launch_profile_store, project_store, work_item_store,
    worktree_store, AppSettings, AppState, CreateDocumentInput, CreateLaunchProfileInput,
    CreateProjectInput, DocumentRecord, LaunchProfileRecord, ProjectRecord, ReparentRequest,
    UpdateAppSettingsInput, UpdateDocumentInput, UpdateLaunchProfileInput, UpdateProjectInput,
    UpdateWorkItemInput, UpsertWorktreeRecordInput, WorkItemRecord, WorktreeRecord,
};
use crate::error::AppResult;
use std::path::Path;

impl AppState {
    pub fn update_app_settings(&self, input: UpdateAppSettingsInput) -> AppResult<AppSettings> {
        let connection = self.connect()?;
        app_settings_store::update_snapshot(&connection, input)
    }

    pub fn set_clean_shutdown(&self, clean: bool) -> AppResult<()> {
        let connection = self.connect()?;
        app_settings_store::set_clean_shutdown(&connection, clean).map_err(Into::into)
    }

    pub fn get_clean_shutdown_setting(&self) -> AppResult<Option<String>> {
        let connection = self.connect()?;
        Ok(app_settings_store::load_clean_shutdown_setting(
            &connection,
        )?)
    }

    pub fn list_in_progress_work_items(&self) -> AppResult<Vec<WorkItemRecord>> {
        let connection = self.connect()?;
        Ok(work_item_store::list_in_progress_records(&connection)?)
    }

    pub fn list_projects(&self) -> AppResult<Vec<ProjectRecord>> {
        let connection = self.connect()?;
        Ok(project_store::load_records(&connection)?)
    }

    pub fn create_project(&self, input: CreateProjectInput) -> AppResult<ProjectRecord> {
        let connection = self.connect()?;
        project_store::ensure_registration(
            &connection,
            &input.name,
            &input.root_path,
            input.work_item_prefix.as_deref(),
        )
    }

    pub fn update_project(&self, input: UpdateProjectInput) -> AppResult<ProjectRecord> {
        let connection = self.connect()?;
        project_store::update_record(&connection, input)
    }

    pub fn create_launch_profile(
        &self,
        input: CreateLaunchProfileInput,
    ) -> AppResult<LaunchProfileRecord> {
        let connection = self.connect()?;
        launch_profile_store::create_record(
            &connection,
            &input.label,
            &input.provider,
            &input.executable,
            &input.args,
            &input.env_json,
        )
    }

    pub fn update_launch_profile(
        &self,
        input: UpdateLaunchProfileInput,
    ) -> AppResult<LaunchProfileRecord> {
        let connection = self.connect()?;
        launch_profile_store::update_record(
            &connection,
            input.id,
            &input.label,
            &input.provider,
            &input.executable,
            &input.args,
            &input.env_json,
        )
    }

    pub fn delete_launch_profile(&self, id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        launch_profile_store::delete_record(&connection, id)
    }

    pub fn get_project(&self, id: i64) -> AppResult<ProjectRecord> {
        let connection = self.connect()?;
        Ok(project_store::load_record_by_id(&connection, id)?)
    }

    pub fn get_launch_profile(&self, id: i64) -> AppResult<LaunchProfileRecord> {
        let connection = self.connect()?;
        Ok(launch_profile_store::load_record_by_id(&connection, id)?)
    }

    pub fn find_project_by_path(&self, path: &Path) -> AppResult<Option<ProjectRecord>> {
        let connection = self.connect()?;
        Ok(project_store::find_record_by_path(&connection, path)?)
    }

    pub fn list_work_items(&self, project_id: i64) -> AppResult<Vec<WorkItemRecord>> {
        let connection = self.connect()?;
        Ok(work_item_store::list_records_by_project(
            &connection,
            project_id,
        )?)
    }

    pub fn get_work_item(&self, id: i64) -> AppResult<WorkItemRecord> {
        let connection = self.connect()?;
        Ok(work_item_store::load_record_by_id(&connection, id)?)
    }

    pub fn get_work_item_by_call_sign(&self, call_sign: &str) -> AppResult<WorkItemRecord> {
        let connection = self.connect()?;
        Ok(work_item_store::load_record_by_call_sign(
            &connection,
            call_sign,
        )?)
    }

    pub fn create_work_item(&self, input: super::CreateWorkItemInput) -> AppResult<WorkItemRecord> {
        let connection = self.connect()?;
        let record = work_item_store::create_record(&connection, input)?;
        self.notify_embeddings_dirty(record.id);
        Ok(record)
    }

    pub fn update_work_item(&self, input: UpdateWorkItemInput) -> AppResult<WorkItemRecord> {
        let connection = self.connect()?;
        let record = work_item_store::update_record(&connection, input)?;
        self.notify_embeddings_dirty(record.id);
        Ok(record)
    }

    pub fn reparent_work_item(
        &self,
        id: i64,
        request: ReparentRequest,
    ) -> AppResult<WorkItemRecord> {
        let connection = self.connect()?;
        work_item_store::reparent_record(&connection, id, request)
    }

    pub fn delete_work_item(&self, id: i64) -> AppResult<()> {
        let mut connection = self.connect()?;
        work_item_store::delete_record(&mut connection, id)
    }

    pub fn list_documents(&self, project_id: i64) -> AppResult<Vec<DocumentRecord>> {
        let connection = self.connect()?;
        Ok(document_store::list_records_by_project(
            &connection,
            project_id,
        )?)
    }

    pub fn create_document(&self, input: CreateDocumentInput) -> AppResult<DocumentRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        document_store::create_record(&connection, input)
    }

    pub fn update_document(&self, input: UpdateDocumentInput) -> AppResult<DocumentRecord> {
        let connection = self.connect()?;
        document_store::update_record(&connection, input)
    }

    pub fn delete_document(&self, id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        document_store::delete_record(&connection, id)
    }

    pub fn upsert_worktree_record(
        &self,
        input: UpsertWorktreeRecordInput,
    ) -> AppResult<WorktreeRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        worktree_store::upsert_record(&connection, input)
    }

    pub fn list_worktrees(&self, project_id: i64) -> AppResult<Vec<WorktreeRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(worktree_store::list_records_by_project(
            &connection,
            project_id,
        )?)
    }

    pub fn get_worktree(&self, id: i64) -> AppResult<WorktreeRecord> {
        let connection = self.connect()?;
        Ok(worktree_store::get_record(&connection, id)?)
    }

    pub fn get_worktree_for_project_and_work_item(
        &self,
        project_id: i64,
        work_item_id: i64,
    ) -> AppResult<Option<WorktreeRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(worktree_store::get_record_for_project_and_work_item(
            &connection,
            project_id,
            work_item_id,
        )?)
    }

    pub fn set_worktree_pinned(&self, id: i64, pinned: bool) -> AppResult<WorktreeRecord> {
        let connection = self.connect()?;
        worktree_store::set_pinned(&connection, id, pinned)
    }

    pub fn delete_worktree(&self, id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        worktree_store::delete_record(&connection, id)
    }

    pub fn clear_worktrees(&self, project_id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        worktree_store::clear_records(&connection, project_id)
    }
}
