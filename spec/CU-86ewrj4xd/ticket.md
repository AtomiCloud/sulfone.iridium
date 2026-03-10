# Ticket: CU-86ewrj4xd

## Metadata

- **ID**: CU-86ewrj4xd
- **Title**: Prompt user to proceed if git is dirty during cyanprint update command
- **Status**: todo
- **Assignee**: Adelphi Liong
- **URL**: https://app.clickup.com/t/86ewrj4xd

## Description

The `cyanprint update` command updates template files in-place. When users have uncommitted changes in their working directory, running this command could lead to data loss or merge conflicts.

Currently, the command does not check for uncommitted git changes before proceeding. This task adds a safety check that:

1. Detects uncommitted git changes
2. Prompts the user to confirm whether to proceed
3. Allows bypassing the check with a `--force` flag
