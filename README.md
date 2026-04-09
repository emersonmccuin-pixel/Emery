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

## Structure

- `src/`: React frontend
- `src-tauri/`: Rust backend and Tauri configuration
- `scripts/`: local helper scripts for development and release workflows

## Next Step

Add Tauri commands in `src-tauri/src/lib.rs` and call them from React with `invoke(...)`.
