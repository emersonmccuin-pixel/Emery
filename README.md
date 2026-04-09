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

## Structure

- `src/`: React frontend
- `src-tauri/`: Rust backend and Tauri configuration
- `scripts/`: local helper scripts for development and release workflows

## Next Step

Add Tauri commands in `src-tauri/src/lib.rs` and call them from React with `invoke(...)`.
