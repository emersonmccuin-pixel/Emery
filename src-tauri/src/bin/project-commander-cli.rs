use clap::{Args, Parser, Subcommand};
use project_commander_lib::db::{
    AppState, CreateWorkItemInput, ProjectRecord, UpdateWorkItemInput, WorkItemRecord,
};
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "project-commander-cli",
    about = "Manage Project Commander work items from a rooted terminal session.",
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
    WorkItem(WorkItemCommand),
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

fn main() {
    let cli = Cli::parse();

    if let Err(error) = run(cli) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let db_path = cli.db_path.ok_or_else(|| {
        "database path not provided. Pass --db-path or launch the session from Project Commander."
            .to_string()
    })?;
    let state = AppState::from_database_path(db_path)?;

    match cli.command {
        Command::Project(command) => handle_project_command(&state, command),
        Command::WorkItem(command) => handle_work_item_command(&state, command),
    }
}

fn handle_project_command(state: &AppState, command: ProjectCommand) -> Result<(), String> {
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

fn handle_work_item_command(state: &AppState, command: WorkItemCommand) -> Result<(), String> {
    match command.command {
        WorkItemSubcommand::List(args) => list_work_items(state, args),
        WorkItemSubcommand::Create(args) => create_work_item(state, args),
        WorkItemSubcommand::Update(args) => update_work_item(state, args),
        WorkItemSubcommand::Close(args) => close_work_item(state, args),
    }
}

fn list_work_items(state: &AppState, args: ListWorkItemsArgs) -> Result<(), String> {
    let project = resolve_project(state, args.project)?;
    let mut items = state.list_work_items(project.id)?;

    if let Some(status) = args.status {
        items.retain(|item| item.status == status);
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
                "#{} [{} / {}] {}",
                item.id, item.status, item.item_type, item.title
            );

            if !item.body.trim().is_empty() {
                println!("  {}", one_line(&item.body));
            }
        }

        Ok(())
    }
}

fn create_work_item(state: &AppState, args: CreateWorkItemArgs) -> Result<(), String> {
    let project = resolve_project(state, args.project)?;
    let item = state.create_work_item(CreateWorkItemInput {
        project_id: project.id,
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

fn update_work_item(state: &AppState, args: UpdateWorkItemArgs) -> Result<(), String> {
    if args.title.is_none()
        && args.body.is_none()
        && args.item_type.is_none()
        && args.status.is_none()
    {
        return Err(
            "no changes provided. Pass at least one of --title, --body, --type, or --status."
                .to_string(),
        );
    }

    let project = resolve_project(state, args.project)?;
    let existing = state.get_work_item(args.id)?;
    ensure_work_item_project(&existing, &project)?;
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

fn close_work_item(state: &AppState, args: CloseWorkItemArgs) -> Result<(), String> {
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

fn resolve_project(
    state: &AppState,
    selection: ProjectSelectionArgs,
) -> Result<ProjectRecord, String> {
    if let Some(project_id) = selection.project_id {
        return state.get_project(project_id);
    }

    if let Ok(current_dir) = env::current_dir() {
        if let Some(project) = state.find_project_by_path(&current_dir)? {
            return Ok(project);
        }
    }

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

    Err(
        "no active project found. Launch the session from Project Commander or pass --project-id."
            .to_string(),
    )
}

fn ensure_work_item_project(item: &WorkItemRecord, project: &ProjectRecord) -> Result<(), String> {
    if item.project_id != project.id {
        return Err(format!(
            "work item #{} belongs to project #{} instead of the active project #{}",
            item.id, item.project_id, project.id
        ));
    }

    Ok(())
}

fn print_json(value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize output: {error}"))?;
    println!("{json}");
    Ok(())
}

fn print_work_item_result(action: &str, item: &WorkItemRecord) {
    println!(
        "{action} work item #{} [{} / {}] {}",
        item.id, item.status, item.item_type, item.title
    );

    if !item.body.trim().is_empty() {
        println!("  {}", one_line(&item.body));
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
