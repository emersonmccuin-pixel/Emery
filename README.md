# EURI

EURI is a supervisor-backed rebuild of Forge.

WCP is the source of truth for architecture and planning. This repository mirrors concrete implementation-facing artifacts and scaffolding produced during the rebuild.

## Current Repo Docs

The main mirrored implementation-facing docs in this repo are:

- `docs/euri-backend-salvage-audit.md`
- `docs/euri-supervisor-rpc-contract.md`
- `docs/euri-domain-schema-plan.md`

Current progression:

- `EURI-2` complete: backend salvage audit
- `EURI-3` complete: durable supervisor and local RPC contract
- `EURI-4` complete: domain model and storage schema plan

Next planned item:

- `EURI-5`: define the Tauri client architecture on top of the supervisor-backed core
