# Backup + Restore (Cloudflare R2)

## Scope

- **Full backup**: hot SQLite snapshot (`db.sqlite`) + vault stronghold
  (`vault/project-commander-vault.hold`), optionally vault key
  (`vault/project-commander-vault.key`) if `include_vault_key` is true.
- **Diagnostics backup**: rolling copies of `logs/`, `crash-reports/`,
  `session-output/` trimmed to `diagnostics_retention_days`.

## Security tradeoff — vault key

Default: `include_vault_key = true`. Makes the backup self-restoring but
means R2 now stores the material that decrypts vault secrets. Disable if you
prefer to re-deposit secrets manually after a restore.

## Restore flow

1. **Prepare** (UI → `prepare_restore_from_r2(object_key)`):
   - GET object from R2
   - Write zip to `<app-data>/restore-staging/<uuid>/backup.zip`
   - Extract to `<app-data>/restore-staging/<uuid>/extracted/`
   - Validate that `db.sqlite` is present
   - Persist `restore-prepare.json` inside the token staging directory so the
     prepared restore survives a desktop-app restart before commit
   - Returns a `RestoreToken` (uuid, expires in 5 minutes, included files)
2. **Confirm** (UI warning modal → `commit_restore(token)`):
   - Reloads the prepared token from memory or from the persisted
     `restore-prepare.json` on disk
   - Writes `<app-data>/restore-pending.json` with
     `{ staging_path, source_object_key, prepared_at, token_id }`
   - Deletes the per-token `restore-prepare.json` after the pending marker is
     written so the token remains single-use
3. **Restart** — user must quit and relaunch the app
4. **Boot-time swap** (`backup::apply_pending_restore_if_any`, called BEFORE
   the main DB connection is opened in `AppState::new`):
   - Swap the staged database into `db/project-commander.sqlite3`, which is
     the same SQLite file the normal runtime opens after startup
   - Move current DB target (+ vault files, if present in staging) aside as
     `.bak`
   - `fs::rename` staged files into place (atomic on same volume)
   - Delete `.bak` + marker + staging dir
   - On any error mid-swap, **the marker is left in place** so the failure
     is visible on next boot

## Troubleshooting

- **Marker left behind** (`restore-pending.json` exists but swap failed): fix
  the underlying error (missing staged file, permission, etc.) then relaunch
  the app. Delete the marker by hand only after confirming the contents of
  the staging directory.
- **Aborted restore** (token expired before commit): staging directory
  remains under `restore-staging/<token>/`. `BackupService::clean_expired_tokens`
  now clears expired staged restore directories from both the in-memory token
  map and the on-disk staging root on the next `prepare_restore` call. Safe to
  delete by hand.
- **Restart between prepare and commit**: the prepared restore is still
  commit-capable after relaunch as long as the token has not expired and the
  staged directory is still present.
- **Corrupt staging**: if `db.sqlite` is missing from the staging dir, the
  boot-time swap refuses to run and surfaces the error on next launch.
