use clap::{Args, Parser, Subcommand};
use project_commander_lib::db::{
    AppState, CreateDocumentInput, CreateWorkItemInput, DocumentRecord, ProjectRecord,
    ReparentRequest, UpdateDocumentInput, UpdateWorkItemInput, WorkItemRecord,
};
use project_commander_lib::error::{AppError, AppResult};
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "project-commander-cli",
    about = "Inspect and update Project Commander project context from a rooted terminal session.",
    after_help = "When launched from Project Commander, the active project and DB path are injected automatically."
)]
struct Cli {
    #[arg(long, env = "PROJECT_COMMANDER_DB_PATH")]
    db_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Project(ProjectCommand),
    Session(SessionCommand),
    WorkItem(WorkItemCommand),
    Document(DocumentCommand),
}

#[derive(Args)]
struct ProjectCommand {
    #[command(subcommand)]
    command: ProjectSubcommand,
}

#[derive(Subcommand)]
enum ProjectSubcommand {
    Current(ProjectCurrentArgs),
}

#[derive(Args)]
struct ProjectCurrentArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct SessionCommand {
    #[command(subcommand)]
    command: SessionSubcommand,
}

#[derive(Subcommand)]
enum SessionSubcommand {
    Brief(SessionBriefArgs),
}

#[derive(Args)]
struct SessionBriefArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct WorkItemCommand {
    #[command(subcommand)]
    command: WorkItemSubcommand,
}

#[derive(Subcommand)]
enum WorkItemSubcommand {
    List(ListWorkItemsArgs),
    Create(CreateWorkItemArgs),
    Update(UpdateWorkItemArgs),
    Close(CloseWorkItemArgs),
}

#[derive(Args)]
struct DocumentCommand {
    #[command(subcommand)]
    command: DocumentSubcommand,
}

#[derive(Subcommand)]
enum DocumentSubcommand {
    List(ListDocumentsArgs),
    Create(CreateDocumentArgs),
    Update(UpdateDocumentArgs),
    Delete(DeleteDocumentArgs),
}

#[derive(Args, Clone, Copy)]
struct ProjectSelectionArgs {
    #[arg(long)]
    project_id: Option<i64>,
}

#[derive(Args)]
struct ListWorkItemsArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long, value_parser = ["backlog", "in_progress", "blocked", "done"])]
    status: Option<String>,
    #[arg(long = "type", value_parser = ["bug", "task", "feature", "note"])]
    item_type: Option<String>,
    #[arg(long)]
    parent_only: bool,
    #[arg(long)]
    open_only: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct CreateWorkItemArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long)]
    title: String,
    #[arg(long, default_value = "")]
    body: String,
    #[arg(long = "type", default_value = "task", value_parser = ["bug", "task", "feature", "note"])]
    item_type: String,
    #[arg(long, default_value = "backlog", value_parser = ["backlog", "in_progress", "blocked", "done"])]
    status: String,
    #[arg(long)]
    parent_work_item_id: Option<i64>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct UpdateWorkItemArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long)]
    id: i64,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    body: Option<String>,
    #[arg(long = "type", value_parser = ["bug", "task", "feature", "note"])]
    item_type: Option<String>,
    #[arg(long, value_parser = ["backlog", "in_progress", "blocked", "done"])]
    status: Option<String>,
    #[arg(long, conflicts_with = "clear_parent")]
    parent_work_item_id: Option<i64>,
    #[arg(long)]
    clear_parent: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct CloseWorkItemArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long)]
    id: i64,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ListDocumentsArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct CreateDocumentArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long)]
    title: String,
    #[arg(long, default_value = "")]
    body: String,
    #[arg(long)]
    work_item_id: Option<i64>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct UpdateDocumentArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long)]
    id: i64,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    body: Option<String>,
    #[arg(long, conflicts_with = "clear_work_item")]
    work_item_id: Option<i64>,
    #[arg(long)]
    clear_work_item: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct DeleteDocumentArgs {
    #[command(flatten)]
    project: ProjectSelectionArgs,
    #[arg(long)]
    id: i64,
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionBriefOutput {
    project: ProjectRecord,
    work_items: Vec<WorkItemRecord>,
    documents: Vec<DocumentRecord>,
}

fn main() {
    let cli = Cli::parse();

    if let Err(error) = run(cli) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> AppResult<()> {
    let db_path = cli.db_path.ok_or_else(|| {
        AppError::invalid_input(
            "database path not provided. Pass --db-path or launch the session from Project Commander.",
        )
    })?;
    let state = AppState::from_database_path(db_path)?;

    match cli.command {
        Command::Project(command) => handle_project_command(&state, command),
        Command::Session(command) => handle_session_command(&state, command),
        Command::WorkItem(command) => handle_work_item_command(&state, command),
        Command::Document(command) => handle_document_command(&state, command),
    }
}

fn handle_project_command(state: &AppState, command: ProjectCommand) -> AppResult<()> {
    match command.command {
        ProjectSubcommand::Current(args) => {
            let project = resolve_project(state, args.project)?;

            if args.json {
                print_json(&project)
            } else {
                println!("Current project: {} (#{})", project.name, project.id);
                println!("Root: {}", project.root_path);
                println!(
                    "Counts: {} work items, {} docs, {} session summaries",
                    project.work_item_count, project.document_count, project.session_count
                );
                Ok(())
            }
        }
    }
}

fn handle_session_command(state: &AppState, command: SessionCommand) -> AppResult<()> {
    match command.command {
        SessionSubcommand::Brief(args) => session_brief(state, args),
    }
}

fn handle_work_item_command(state: &AppState, command: WorkItemCommand) -> AppResult<()> {
    match command.command {
        WorkItemSubcommand::List(args) => list_work_items(state, args),
        WorkItemSubcommand::Create(args) => create_work_item(state, args),
        WorkItemSubcommand::Update(args) => update_work_item(state, args),
        WorkItemSubcommand::Close(args) => close_work_item(state, args),
    }
}

fn handle_document_command(state: &AppState, command: DocumentCommand) -> AppResult<()> {
    match command.command {
        DocumentSubcommand::List(args) => list_documents(state, args),
        DocumentSubcommand::Create(args) => create_document(state, args),
        DocumentSubcommand::Update(args) => update_document(state, args),
        DocumentSubcommand::Delete(args) => delete_document(state, args),
    }
}

fn session_brief(state: &AppState, args: SessionBriefArgs) -> AppResult<()> {
    let project = resolve_project(state, args.project)?;
    let work_items = state.list_work_items(project.id)?;
    let documents = state.list_documents(project.id)?;
    let brief = SessionBriefOutput {
        project,
        work_items,
        documents,
    };

    if args.json {
        print_json(&brief)
    } else {
        println!(
            "Session brief for {} (#{})",
            brief.project.name, brief.project.id
        );
        println!("Root: {}", brief.project.root_path);
        println!("Work items: {}", brief.work_items.len());
        println!("Documents: {}", brief.documents.len());
        println!("Recommended first command: project-commander-cli session brief --json");
        Ok(())
    }
}

fn list_work_items(state: &AppState, args: ListWorkItemsArgs) -> AppResult<()> {
    let project = resolve_project(state, args.project)?;
    let mut items = state.list_work_items(project.id)?;

    if let Some(status) = args.status {
        items.retain(|item| item.status == status);
    }

    if let Some(item_type) = args.item_type {
        items.retain(|item| item.item_type == item_type);
    }

    if args.open_only {
        items.retain(|item| item.status != "done");
    }

    if args.parent_only {
        items.retain(|item| item.parent_work_item_id.is_none());
    }

    if args.json {
        print_json(&items)
    } else if items.is_empty() {
        println!("No work items for {}.", project.name);
        Ok(())
    } else {
        println!("Work items for {} (#{})", project.name, project.id);

        for item in items {
            println!(
                "{} [{} / {}] {}",
                item.call_sign, item.status, item.item_type, item.title
            );

            if !item.body.trim().is_empty() {
                println!("  {}", one_line(&item.body));
            }
        }

        Ok(())
    }
}

fn list_documents(state: &AppState, args: ListDocumentsArgs) -> AppResult<()> {
    let project = resolve_project(state, args.project)?;
    let documents = state.list_documents(project.id)?;
    let work_item_titles = work_item_title_map(&state.list_work_items(project.id)?);

    if args.json {
        print_json(&documents)
    } else if documents.is_empty() {
        println!("No documents for {}.", project.name);
        Ok(())
    } else {
        println!("Documents for {} (#{})", project.name, project.id);

        for document in documents {
            let linked_label = document
                .work_item_id
                .map(|work_item_id| {
                    work_item_titles
                        .get(&work_item_id)
                        .map(|title| format!("linked to #{} {}", work_item_id, title))
                        .unwrap_or_else(|| format!("linked to work item #{}", work_item_id))
                })
                .unwrap_or_else(|| "project-level".to_string());

            println!("#{} [{}] {}", document.id, linked_label, document.title);

            if !document.body.trim().is_empty() {
                println!("  {}", one_line(&document.body));
            }
        }

        Ok(())
    }
}

fn create_work_item(state: &AppState, args: CreateWorkItemArgs) -> AppResult<()> {
    let project = resolve_project(state, args.project)?;
    let item = state.create_work_item(CreateWorkItemInput {
        project_id: project.id,
        parent_work_item_id: args.parent_work_item_id,
        title: args.title,
        body: args.body,
        item_type: args.item_type,
        status: args.status,
    })?;

    if args.json {
        print_json(&item)
    } else {
        print_work_item_result("Created", &item);
        Ok(())
    }
}

fn create_document(state: &AppState, args: CreateDocumentArgs) -> AppResult<()> {
    let project = resolve_project(state, args.project)?;
    let document = state.create_document(CreateDocumentInput {
        project_id: project.id,
        work_item_id: args.work_item_id,
        title: args.title,
        body: args.body,
    })?;

    if args.json {
        print_json(&document)
    } else {
        print_document_result("Created", &document);
        Ok(())
    }
}

fn update_work_item(state: &AppState, args: UpdateWorkItemArgs) -> AppResult<()> {
    if args.title.is_none()
        && args.body.is_none()
        && args.item_type.is_none()
        && args.status.is_none()
        && args.parent_work_item_id.is_none()
        && !args.clear_parent
    {
        return Err(AppError::invalid_input(
            "no changes provided. Pass at least one of --title, --body, --type, --status, --parent-work-item-id, or --clear-parent.",
        ));
    }

    let project = resolve_project(state, args.project)?;
    let existing = state.get_work_item(args.id)?;
    ensure_work_item_project(&existing, &project)?;

    let reparent_request = if args.clear_parent {
        Some(ReparentRequest::Detach)
    } else {
        args.parent_work_item_id.map(ReparentRequest::SetParent)
    };
    if let Some(request) = reparent_request {
        state.reparent_work_item(existing.id, request)?;
    }

    let item = state.update_work_item(UpdateWorkItemInput {
        id: existing.id,
        title: args.title.unwrap_or(existing.title),
        body: args.body.unwrap_or(existing.body),
        item_type: args.item_type.unwrap_or(existing.item_type),
        status: args.status.unwrap_or(existing.status),
    })?;

    if args.json {
        print_json(&item)
    } else {
        print_work_item_result("Updated", &item);
        Ok(())
    }
}

fn update_document(state: &AppState, args: UpdateDocumentArgs) -> AppResult<()> {
    if args.title.is_none()
        && args.body.is_none()
        && args.work_item_id.is_none()
        && !args.clear_work_item
    {
        return Err(AppError::invalid_input(
            "no changes provided. Pass at least one of --title, --body, --work-item-id, or --clear-work-item.",
        ));
    }

    let project = resolve_project(state, args.project)?;
    let existing = state
        .list_documents(project.id)?
        .into_iter()
        .find(|document| document.id == args.id)
        .ok_or_else(|| {
            AppError::not_found(format!(
                "document #{} does not belong to the active project",
                args.id
            ))
        })?;

    let work_item_id = if args.clear_work_item {
        None
    } else {
        args.work_item_id.or(existing.work_item_id)
    };
    let document = state.update_document(UpdateDocumentInput {
        id: existing.id,
        work_item_id,
        title: args.title.unwrap_or(existing.title),
        body: args.body.unwrap_or(existing.body),
    })?;

    if args.json {
        print_json(&document)
    } else {
        print_document_result("Updated", &document);
        Ok(())
    }
}

fn close_work_item(state: &AppState, args: CloseWorkItemArgs) -> AppResult<()> {
    let project = resolve_project(state, args.project)?;
    let existing = state.get_work_item(args.id)?;
    ensure_work_item_project(&existing, &project)?;
    let item = state.update_work_item(UpdateWorkItemInput {
        id: existing.id,
        title: existing.title,
        body: existing.body,
        item_type: existing.item_type,
        status: "done".to_string(),
    })?;

    if args.json {
        print_json(&item)
    } else {
        print_work_item_result("Closed", &item);
        Ok(())
    }
}

fn delete_document(state: &AppState, args: DeleteDocumentArgs) -> AppResult<()> {
    let project = resolve_project(state, args.project)?;
    let existing = state
        .list_documents(project.id)?
        .into_iter()
        .find(|document| document.id == args.id)
        .ok_or_else(|| {
            AppError::not_found(format!(
                "document #{} does not belong to the active project",
                args.id
            ))
        })?;

    state.delete_document(existing.id)?;

    if args.json {
        print_json(&serde_json::json!({
            "deleted": true,
            "id": existing.id,
        }))
    } else {
        println!("Deleted document #{} {}", existing.id, existing.title);
        Ok(())
    }
}

fn resolve_project(
    state: &AppState,
    selection: ProjectSelectionArgs,
) -> AppResult<ProjectRecord> {
    if let Some(project_id) = selection.project_id {
        return state.get_project(project_id);
    }

    // A launched terminal session should stay bound to its registered project,
    // even if the shell later changes directories.
    if let Ok(project_id) = env::var("PROJECT_COMMANDER_PROJECT_ID") {
        if let Ok(parsed_project_id) = project_id.parse::<i64>() {
            return state.get_project(parsed_project_id);
        }
    }

    if let Some(root_path) = env::var_os("PROJECT_COMMANDER_ROOT_PATH") {
        if let Some(project) = state.find_project_by_path(Path::new(&root_path))? {
            return Ok(project);
        }
    }

    if let Ok(current_dir) = env::current_dir() {
        if let Some(project) = state.find_project_by_path(&current_dir)? {
            return Ok(project);
        }
    }

    Err(AppError::not_found(
        "no active project found. Launch the session from Project Commander or pass --project-id.",
    ))
}

fn ensure_work_item_project(item: &WorkItemRecord, project: &ProjectRecord) -> AppResult<()> {
    if item.project_id != project.id {
        return Err(AppError::invalid_input(format!(
            "work item #{} belongs to project #{} instead of the active project #{}",
            item.id, item.project_id, project.id
        )));
    }

    Ok(())
}

fn work_item_title_map(items: &[WorkItemRecord]) -> std::collections::HashMap<i64, String> {
    items
        .iter()
        .map(|item| (item.id, item.title.clone()))
        .collect()
}

fn print_json(value: &impl Serialize) -> AppResult<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| AppError::internal(format!("failed to serialize output: {error}")))?;
    println!("{json}");
    Ok(())
}

fn print_work_item_result(action: &str, item: &WorkItemRecord) {
    println!(
        "{action} work item {} [{} / {}] {}",
        item.call_sign, item.status, item.item_type, item.title
    );

    if !item.body.trim().is_empty() {
        println!("  {}", one_line(&item.body));
    }
}

fn print_document_result(action: &str, document: &DocumentRecord) {
    println!("{action} document #{} {}", document.id, document.title);

    if let Some(work_item_id) = document.work_item_id {
        println!("  linked to work item #{work_item_id}");
    } else {
        println!("  project-level document");
    }

    if !document.body.trim().is_empty() {
        println!("  {}", one_line(&document.body));
    }
}

fn one_line(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
