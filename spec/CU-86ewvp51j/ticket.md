# Ticket: CU-86ewvp51j

- **Type**: task
- **Status**: todo
- **URL**: https://app.clickup.com/t/86ewvp51j
- **Parent**: CU-86et8z88g

## Description

Ticket: 86et8z88g-iridium-2
Blocks: iridium-3 (Iridium: Test Command)
Blocked By: boron-1 (Boron: Executor Try Endpoint)

Overview

Implement cyanprint try command for interactive local testing of CyanPrint templates.

Scope

1. Config Parsing: Dev Section

Parse dev section from cyan.yaml (template_url, blob_path)

2. Command: cyanprint try <template_path> <output_path>

Options:

--dev - Use dev mode (external template server, local blob path)
--cleanup - Cleanup containers after execution

3. Normal Mode Flow

Pre-checks (Docker, cyan.yaml, build section)
Generate IDs (local-{uuid}, session-{uuid})
Resolve & pin dependencies from registry
Build images (bollard SDK)
Allocate port (5550..5600)
Start template container
Interactive Q&A loop
Capture final state
Call Boron /executor/try + /executor/{sessionId}
Cleanup (if --cleanup)

4. Dev Mode Flow (--dev)

Skip build, use dev.template_url for Q&A
Use dev.blob_path for Boron

5. Port Allocation

Find available port dynamically (5550..5600)

6. Dependency Pinning

Query registry for each processor/plugin and pin version

Acceptance Criteria

Normal mode: build, Q&A, generate files
Dev mode: no build, Q&A to external, generate files
Cleanup flag works
Error handling for Docker/config issues

Files to Create/Modify

src/commands/try.rs - Try command
src/config/cyan.rs - Parse dev section
src/docker/mod.rs - Docker operations
src/boron/client.rs - Boron API client
src/qa/loop.rs - Q&A loop handler
src/registry/resolver.rs - Dependency resolution

Spec: 02-try.md

## Comments

(No comments)

---

# Parent: CU-86et8z88g (task)

- **Title**: [Ir, B] Allow for local-testing
- **Status**: in progress
- **URL**: https://app.clickup.com/t/86et8z88g

## Description

Local Testing Strategy for CyanPrint

Purpose

Enable local development and testing of CyanPrint templates, processors, and plugins without requiring full production deployment. This allows developers to iterate quickly on templates and validate changes before publishing.
Problem Statement

Currently, testing a CyanPrint template requires:

Building images
Pushing to registry
Deploying to production infrastructure

This is slow, requires network access, and creates friction in the development loop.
Solution

A coordinated effort between Iridium (CLI) and Boron (Executor) to support local testing:

Iridium handles: Config parsing, dependency resolution, image building, interactive Q&A, test orchestration
Boron handles: Volume management, container execution, resolver support

Key Design Decisions

[table-embed:1:1 Decision| 1:2 Choice| 1:3 Rationale| 2:1 Q&A vs Execution| 2:2 Q&A is stateless (Iridium), Execution is stateful (Boron)| 2:3 Clean separation; Boron never handles Q&A| 3:1 Template ID for try/test| 3:2 Synthetic ID: local-{uuid} | 3:3 No registry lookup needed; works with existing naming| 4:1 One repo = One type| 4:2 Template OR Processor OR Plugin| 4:3 Simplifies testing| 5:1 Dependency pinning| 5:2 Pin at try/test time| 5:3 Ensures consistency even if new versions released mid-execution| 6:1 Build tooling| 6:2 build uses buildx CLI; try uses bollard SDK| 6:3 Different needs (multi-platform vs programmatic control)|]

Commands Delivered

# Build images (CI/CD)

cyanprint build v1.0.0

# Local testing (interactive)

cyanprint try . ./output
cyanprint try . ./output --dev # Dev mode: external template server

# Automated testing

cyanprint test
cyanprint test --update-snapshots
cyanprint test --parallel 4

# Publish

cyanprint push --build v1.0.0

Architecture

┌─────────────────────────────────────────────────────────────────────────┐
│ LOCAL TESTING ARCHITECTURE │
├─────────────────────────────────────────────────────────────────────────┤
│ │
│ IRIDIUM (CLI) │
│ ┌─────────────────────────────────────────────────────────────────┐ │
│ │ - Pre-flight checks (Docker, config) │ │
│ │ - Dependency resolution & pinning from registry │ │
│ │ - Build images (bollard SDK for try, buildx CLI for build) │ │
│ │ - Q&A (stateless, external to Boron) │ │
│ │ - Call Boron /executor/try │ │
│ │ - Validation & snapshot comparison (test) │ │
│ └─────────────────────────────────────────────────────────────────┘ │
│ │ │
│ ▼ │
│ BORON │
│ ┌─────────────────────────────────────────────────────────────────┐ │
│ │ - Blob setup (image unzip OR path copy for --dev) │ │
│ │ - Session volume management │ │
│ │ - Dependency warming (pull if missing) │ │
│ │ - Processor execution (parallel) │ │
│ │ - Merger execution │ │
│ │ - Plugin execution (sequential) │ │
│ │ - Resolver warming + proxy │ │
│ │ - Output to bind mount │ │
│ └─────────────────────────────────────────────────────────────────┘ │
│ │
└─────────────────────────────────────────────────────────────────────────┘

Subtasks (4 tickets)

[table-embed:1:1 #| 1:2 Ticket| 1:3 Blocks| 1:4 Blocked By| 2:1 1| 2:2 [B] Executor Try Endpoint| 2:3 iridium-2| 2:4 None| 3:1 2| 3:2 [Ir] Build + Push Commands| 3:3 None| 3:4 None| 4:1 3| 4:2 [Ir] Try Command| 4:3 iridium-3| 4:4 boron-1| 5:1 4| 5:2 [Ir] Test Command| 5:3 None| 5:4 iridium-2|]

Critical Path

boron-1 (Executor Try) → iridium-2 (Try) → iridium-3 (Test)

iridium-1 (Build+Push) is parallel work - can be done anytime.

Execution Strategy

Phase 1 (Parallel):

boron-1: Executor Try Endpoint (Boron)
iridium-1: Build + Push Commands (Iridium)

Phase 2:

iridium-2: Try Command (after boron-1)

Phase 3:

iridium-3: Test Command (after iridium-2)

Naming Conventions

[table-embed:1:1 Resource| 1:2 Pattern| 1:3 Example| 2:1 Blob volume| 2:2 cyan-{LOCAL_TEMPLATE_ID} | 2:3 cyan-localabc123 | 3:1 Session volume| 3:2 cyan-{LOCAL_TEMPLATE_ID}-{SESSION_ID} | 3:3 cyan-localabc123-sessionxyz789 | 4:1 Processor container| 4:2 cyan-processor-{PROC_ID}-{SESSION_ID} | 4:3 cyan-processor-procid-sessionxyz789 | 5:1 Plugin container| 5:2 cyan-plugin-{PLUGIN_ID}-{SESSION_ID} | 5:3 cyan-plugin-pluginid-sessionxyz789 | 6:1 Resolver container| 6:2 cyan-resolver-{RESOLVER_ID} | 6:3 cyan-resolver-resolverid |]

Port Assignments

[table-embed:1:1 Artifact| 1:2 Port| 2:1 Template| 2:2 5550| 3:1 Processor| 3:2 5551| 4:1 Plugin| 4:2 5552| 5:1 Resolver| 5:2 5553| 6:1 Merger| 6:2 9000|]

Specs

Detailed specs are in the repo: 86et8z88g/

boron/01-executor-try.md - Boron endpoint details
iridium/01-build-push.md - Build/Push commands
iridium/02-try.md - Try command details
iridium/03-test.md - Test command details
