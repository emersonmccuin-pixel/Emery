# Project Commander

Rust + Tauri + React desktop app scaffold.

## Environments

### Dev

Use the live-reload desktop environment while building features:

```bash
npm install
npm run tauri:dev
```

This starts Vite on port `1420` and launches the Tauri shell against it.

### Prod

Build the packaged production app:

```bash
npm run prod:build
```

Run the locally built production executable:

```bash
npm run prod:run
```

Windows installers are produced under `src-tauri/target/release/bundle/`.

## Shared Data

Development and production are configured to use the same Tauri app-data root,
based on the app identifier in `src-tauri/tauri.conf.json`.

The scaffold already creates a dedicated database folder inside that root:

```text
<app-data-dir>\db\
```

The app now stores its shared SQLite file at:

```text
<app-data-dir>\db\project-commander.sqlite3
```

If you store work items, documents, and session summaries there, both
`npm run tauri:dev` and the packaged `project-commander.exe` will use the same
data.

## Current Slice

The current MVP slice includes:

- a shared SQLite database owned by the app
- registered projects with root folders
- Claude Code launch profiles stored in the DB
- a PTY-backed terminal slice that can launch Claude Code in the selected project root
- a selectable launch-profile workflow that acts as the MVP account model
- project-scoped work-item CRUD for bugs, tasks, features, and notes
- an agent bridge CLI that lets launched Claude Code sessions list, create, update, and close work items

## Agent Bridge

Launched terminal sessions now inherit:

- `PROJECT_COMMANDER_DB_PATH`
- `PROJECT_COMMANDER_PROJECT_ID`
- `PROJECT_COMMANDER_PROJECT_NAME`
- `PROJECT_COMMANDER_ROOT_PATH`

They also get the companion `project-commander-cli` helper on `PATH`.

Example commands inside a launched Claude Code session:

```powershell
project-commander-cli project current --json
project-commander-cli work-item list --json
project-commander-cli work-item create --type bug --title "Log a bug in Emery" --body "Describe the issue." --json
project-commander-cli work-item close --id 12 --json
```

`npm run tauri:dev` and `npm run tauri:build` now build the companion CLI before starting the app shell so the bridge is available in both dev and local packaged runs.

## Structure

- `src/`: React frontend
- `src-tauri/`: Rust backend and Tauri configuration
- `scripts/`: local helper scripts for development and release workflows

## Next Step

Add project documents and session summaries so Claude Code has more than just work items to read and update.
