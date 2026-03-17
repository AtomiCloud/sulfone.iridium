# Plan 1: Update default coordinator endpoint for test and try commands

## Goal

Change the `default_value` on the `coordinator_endpoint` clap arg from `http://coord.cyanprint.dev:9000` to `http://localhost:9000` for all test and try commands.

## File to modify

`cyanprint/src/commands.rs`

## Approach

Find all `coordinator_endpoint` field definitions with `default_value = "http://coord.cyanprint.dev:9000"` that belong to try or test command structs, and replace the default value string. There are 7 occurrences:

- Try commands (2): `TryTemplateArgs`, `TryGroupArgs`
- Test commands (5): `TestTemplateArgs`, `TestProcessorArgs`, `TestPluginArgs`, `TestResolverArgs`, `TestInitArgs`

Do NOT touch `CreateArgs` or `UpdateArgs`.

## Implementation Checklist

- [ ] Change `default_value` to `http://localhost:9000` for Try Template command (~line 152)
- [ ] Change `default_value` to `http://localhost:9000` for Try Group command (~line 168)
- [ ] Change `default_value` to `http://localhost:9000` for Test Template command (~line 214)
- [ ] Change `default_value` to `http://localhost:9000` for Test Processor command (~line 258)
- [ ] Change `default_value` to `http://localhost:9000` for Test Plugin command (~line 299)
- [ ] Change `default_value` to `http://localhost:9000` for Test Resolver command (~line 344)
- [ ] Change `default_value` to `http://localhost:9000` for Test Init command (~line 397)
- [ ] Verify Create and Update commands are NOT changed
- [ ] Verify the project compiles (`cargo check`)
