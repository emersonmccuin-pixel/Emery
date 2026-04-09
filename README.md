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

The app UI displays the exact resolved storage path at runtime. If you store
SQLite files or other durable state there, both `npm run tauri:dev` and the
packaged `project-commander.exe` will use the same data.

## Structure

- `src/`: React frontend
- `src-tauri/`: Rust backend and Tauri configuration
- `scripts/`: local helper scripts for development and release workflows

## Next Step

Add Tauri commands in `src-tauri/src/lib.rs` and call them from React with `invoke(...)`.
