# Ticket: CU-86ewr17ey

- **Title**: [Ir] Fix file deletion during upgrades
- **Type**: Bug
- **Status**: todo
- **URL**: https://app.clickup.com/t/86ewr17ey
- **Assignee**: Adelphi Liong
- **Tags**: bug, dept:engineering, platform:sulfone
- **Parent**: none

## Description

After performing a 3-way merge in the VFS (Virtual File System), deletions are not properly accounted for. Specifically, if a file is deleted in the "current" branch relative to the base, the merge process does not actually remove the file—it simply causes the file to disappear from the merged view, but does not register this as a true deletion. This creates a problem because the current branch may have created additional data or dependencies related to the file that is now missing, leading to inconsistencies or orphaned data.

The root of the issue is that the VFS 3-way-merge operation is generally implemented as a "write" operation (i.e., JavaScript writes), and there is no explicit handling for deletions. Since the system is designed around writing or updating files, delete operations are not naturally represented or propagated during the merge. As a result, files that should be deleted according to the merge logic are simply omitted, rather than being explicitly removed, which can leave behind related artifacts or metadata.

Resolving this is challenging because:

- The merge process lacks a mechanism to track and propagate deletions from the current branch relative to the base.
- Additional data or state associated with the deleted file in the current branch may persist, causing inconsistencies.
- The write-centric design of the VFS merge means that only additions and modifications are handled cleanly, while deletions are ignored or mishandled.

To address this, the merge logic would need to explicitly detect files that have been deleted relative to the base and ensure that these deletions are properly applied, including cleaning up any associated data or references.

## Comments

(no comments)
