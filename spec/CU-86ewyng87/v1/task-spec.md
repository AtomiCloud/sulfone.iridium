# Task Spec: Default coordinator endpoint to localhost for test and try commands

## Ticket

CU-86ewyng87 — [Ir] Default coordinator endpoint to localhost for test and try commands

## Objective

Change the default coordinator endpoint from `http://coord.cyanprint.dev:9000` to `http://localhost:9000` for all test and try commands in the CyanPrint CLI. This makes local development the default experience for these commands, since they are primarily used during development.

## Scope

### In scope

- Change `default_value` on the `coordinator_endpoint` arg for these 7 commands in `cyanprint/src/commands.rs`:
  - **Try commands**: `Try Template`, `Try Group`
  - **Test commands**: `Test Template`, `Test Processor`, `Test Plugin`, `Test Resolver`, `Test Init`

### Out of scope

- `Create` and `Update` commands — these remain defaulted to `http://coord.cyanprint.dev:9000`
- The `-c`/`--coordinator` flag and `CYANPRINT_COORDINATOR` env var — these already work as overrides via clap's `env` attribute and require no code changes
- Any changes to coordinator client logic or documentation

## Acceptance Criteria

1. All 7 test/try commands default to `http://localhost:9000` when no `-c` flag or `CYANPRINT_COORDINATOR` env var is provided
2. The `-c`/`--coordinator` flag continues to override the default
3. The `CYANPRINT_COORDINATOR` env var continues to override the default
4. Create and Update commands remain defaulted to `http://coord.cyanprint.dev:9000`
5. No behavioral change other than the default endpoint value

## Constraints

- Pure string change in clap `default_value` attribute — no logic changes
- Single file: `cyanprint/src/commands.rs`
