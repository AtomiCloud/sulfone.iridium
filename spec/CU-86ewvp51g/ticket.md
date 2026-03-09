# Ticket: CU-86ewvp51g

- **Type**: Task
- **Status**: todo
- **URL**: https://app.clickup.com/t/86ewvp51g
- **Parent**: 86et8z88g

## Description

Ticket: 86et8z88g-iridium-1
Blocks: None
Blocked By: None

Overview

Implement cyanprint build and cyanprint push commands for building Docker images and publishing to CyanPrint registry.

Scope

1. Config Parsing: Build Section

Parse build section from cyan.yaml (registry, platforms, images)

2. Command: cyanprint build <tag>

Build Docker images using docker buildx CLI.

Options:

--platform <platforms> - Target platforms
--image <name> - Build specific image only
--builder <name> - Buildx builder to use
--no-cache - Don't use cache
--dry-run - Show commands without executing

3. Command: cyanprint push

Mode 1: Build and push: cyanprint push --build <tag>
Mode 2: Push existing: cyanprint push --template <ref> --blob <ref>

4. Authentication

Support CYAN_TOKEN environment variable or --token flag

Acceptance Criteria

Dry run shows commands
Build succeeds and pushes to registry
Build specific image works
Push with build works
Push existing images works
Error handling for Docker/auth issues

Files to Create/Modify

src/commands/build.rs - Build command
src/commands/push.rs - Push command
src/config/cyan.rs - Parse build section
src/registry/client.rs - Registry API client
src/docker/buildx.rs - Buildx CLI wrapper

Spec: 01-build-push.md

---

# Parent: 86et8z88g (Feature)

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

- Q&A is stateless (Iridium), Execution is stateful (Boron)
- Template ID for try/test: Synthetic ID: local-{uuid}
- One repo = One type: Template OR Processor OR Plugin
- Dependency pinning: Pin at try/test time
- Build tooling: build uses buildx CLI; try uses bollard SDK

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

IRIDIUM (CLI)

- Pre-flight checks (Docker, config)
- Dependency resolution & pinning from registry
- Build images (bollard SDK for try, buildx CLI for build)
- Q&A (stateless, external to Boron)
- Call Boron /executor/try
- Validation & snapshot comparison (test)

BORON

- Blob setup (image unzip OR path copy for --dev)
- Session volume management
- Dependency warming (pull if missing)
- Processor execution (parallel)
- Merger execution
- Plugin execution (sequential)
- Resolver warming + proxy
- Output to bind mount

Subtasks (4 tickets)

1. [B] Executor Try Endpoint - Blocks: iridium-2, Blocked By: None
2. [Ir] Build + Push Commands - Blocks: None, Blocked By: None
3. [Ir] Try Command - Blocks: iridium-3, Blocked By: boron-1
4. [Ir] Test Command - Blocks: None, Blocked By: iridium-2

Critical Path

boron-1 (Executor Try) -> iridium-2 (Try) -> iridium-3 (Test)

iridium-1 (Build+Push) is parallel work - can be done anytime.

Execution Strategy

Phase 1 (Parallel):

- boron-1: Executor Try Endpoint (Boron)
- iridium-1: Build + Push Commands (Iridium)

Phase 2:

- iridium-2: Try Command (after boron-1)

Phase 3:

- iridium-3: Test Command (after iridium-2)

Naming Conventions

- Blob volume: cyan-{LOCAL_TEMPLATE_ID} (e.g., cyan-localabc123)
- Session volume: cyan-{LOCAL_TEMPLATE_ID}-{SESSION_ID} (e.g., cyan-localabc123-sessionxyz789)
- Processor container: cyan-processor-{PROC_ID}-{SESSION_ID}
- Plugin container: cyan-plugin-{PLUGIN_ID}-{SESSION_ID}
- Resolver container: cyan-resolver-{RESOLVER_ID}

Port Assignments

- Template: 5550
- Processor: 5551
- Plugin: 5552
- Resolver: 5553
- Merger: 9000

Specs

Detailed specs are in the repo: 86et8z88g/

- boron/01-executor-try.md - Boron endpoint details
- iridium/01-build-push.md - Build/Push commands
- iridium/02-try.md - Try command details
- iridium/03-test.md - Test command details
