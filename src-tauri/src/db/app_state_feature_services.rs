use super::{
    app_settings_store, vault, workflow, AdoptCatalogEntryInput, AppSettings, AppState,
    CatalogAdoptionTarget, DeleteVaultEntryInput, FailWorkflowRunInput,
    MarkWorkflowStageDispatchedInput, ProjectWorkflowCatalog, ProjectWorkflowRunSnapshot,
    RecordWorkflowStageResultInput, ResolvedVaultBinding, StartWorkflowRunInput,
    UpsertVaultEntryInput, VaultAccessBindingRequest, VaultSnapshot, WorkflowLibrarySnapshot,
    WorkflowRunRecord,
};
use crate::error::AppResult;
use std::path::Path;

impl AppState {
    pub fn list_workflow_library(&self) -> AppResult<WorkflowLibrarySnapshot> {
        let connection = self.connect()?;
        workflow::sync_library_catalog(&connection, Path::new(&self.storage.app_data_dir))?;
        Ok(workflow::load_library_snapshot(
            &connection,
            Path::new(&self.storage.app_data_dir),
        )?)
    }

    pub fn list_project_workflow_catalog(
        &self,
        project_id: i64,
    ) -> AppResult<ProjectWorkflowCatalog> {
        let connection = self.connect()?;
        workflow::sync_library_catalog(&connection, Path::new(&self.storage.app_data_dir))?;
        Ok(workflow::load_project_catalog(&connection, project_id)?)
    }

    pub fn adopt_catalog_entry(
        &self,
        input: AdoptCatalogEntryInput,
    ) -> AppResult<ProjectWorkflowCatalog> {
        let connection = self.connect()?;
        workflow::sync_library_catalog(&connection, Path::new(&self.storage.app_data_dir))?;
        workflow::adopt_catalog_entry(&connection, &input)?;
        Ok(workflow::load_project_catalog(
            &connection,
            input.project_id,
        )?)
    }

    pub fn upgrade_catalog_adoption(
        &self,
        input: CatalogAdoptionTarget,
    ) -> AppResult<ProjectWorkflowCatalog> {
        let connection = self.connect()?;
        workflow::sync_library_catalog(&connection, Path::new(&self.storage.app_data_dir))?;
        workflow::upgrade_catalog_adoption(&connection, &input)?;
        Ok(workflow::load_project_catalog(
            &connection,
            input.project_id,
        )?)
    }

    pub fn detach_catalog_adoption(
        &self,
        input: CatalogAdoptionTarget,
    ) -> AppResult<ProjectWorkflowCatalog> {
        let connection = self.connect()?;
        workflow::sync_library_catalog(&connection, Path::new(&self.storage.app_data_dir))?;
        workflow::detach_catalog_adoption(&connection, &input)?;
        Ok(workflow::load_project_catalog(
            &connection,
            input.project_id,
        )?)
    }

    pub fn list_project_workflow_runs(
        &self,
        project_id: i64,
    ) -> AppResult<ProjectWorkflowRunSnapshot> {
        let connection = self.connect()?;
        Ok(workflow::load_project_run_snapshot(
            &connection,
            project_id,
        )?)
    }

    pub fn start_workflow_run(&self, input: StartWorkflowRunInput) -> AppResult<WorkflowRunRecord> {
        let connection = self.connect()?;
        workflow::sync_library_catalog(&connection, Path::new(&self.storage.app_data_dir))?;
        Ok(workflow::start_workflow_run(&connection, &input)?)
    }

    pub fn mark_workflow_stage_dispatched(
        &self,
        input: MarkWorkflowStageDispatchedInput,
    ) -> AppResult<WorkflowRunRecord> {
        let connection = self.connect()?;
        Ok(workflow::mark_workflow_stage_dispatched(
            &connection,
            &input,
        )?)
    }

    pub fn record_workflow_stage_result(
        &self,
        input: RecordWorkflowStageResultInput,
    ) -> AppResult<WorkflowRunRecord> {
        let connection = self.connect()?;
        Ok(workflow::record_workflow_stage_result(&connection, &input)?)
    }

    pub fn fail_workflow_run(&self, input: FailWorkflowRunInput) -> AppResult<WorkflowRunRecord> {
        let connection = self.connect()?;
        Ok(workflow::fail_workflow_run(&connection, &input)?)
    }

    pub fn get_app_settings(&self) -> AppResult<AppSettings> {
        let connection = self.connect()?;
        Ok(app_settings_store::load_snapshot(&connection)?)
    }

    pub fn list_vault_entries(&self) -> AppResult<VaultSnapshot> {
        let connection = self.connect()?;
        Ok(vault::load_snapshot(
            &connection,
            Path::new(&self.storage.app_data_dir),
        )?)
    }

    pub fn upsert_vault_entry(&self, input: UpsertVaultEntryInput) -> AppResult<VaultSnapshot> {
        let connection = self.connect()?;
        vault::upsert_entry(&connection, Path::new(&self.storage.app_data_dir), input)?;
        Ok(vault::load_snapshot(
            &connection,
            Path::new(&self.storage.app_data_dir),
        )?)
    }

    pub fn delete_vault_entry(&self, input: DeleteVaultEntryInput) -> AppResult<VaultSnapshot> {
        let connection = self.connect()?;
        vault::delete_entry(&connection, Path::new(&self.storage.app_data_dir), &input)?;
        Ok(vault::load_snapshot(
            &connection,
            Path::new(&self.storage.app_data_dir),
        )?)
    }

    pub fn resolve_vault_access_bindings(
        &self,
        bindings: Vec<VaultAccessBindingRequest>,
        source: &str,
    ) -> AppResult<Vec<ResolvedVaultBinding>> {
        let connection = self.connect()?;
        let app_data_dir = Path::new(&self.storage.app_data_dir);
        bindings
            .into_iter()
            .map(|binding| {
                vault::resolve_access_binding(&connection, app_data_dir, binding, source)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn record_vault_access_bindings<'a, I>(
        &self,
        bindings: I,
        action: &str,
        consumer_prefix: &str,
        correlation_id: &str,
        session_id: Option<i64>,
    ) -> AppResult<()>
    where
        I: IntoIterator<Item = &'a ResolvedVaultBinding>,
    {
        let connection = self.connect()?;
        vault::record_access_bindings(
            &connection,
            bindings,
            action,
            consumer_prefix,
            correlation_id,
            session_id,
        )?;
        Ok(())
    }
}
