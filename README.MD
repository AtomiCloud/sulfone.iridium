# CyanPrint (sulfone-iridium)

## What is CyanPrint?


CyanPrint is a templating engine that allows you to create templates for your projects. It is a command line tool that allows you to create templates and use them to create projects.

## Caching

CyanPrint keeps a content-addressed cache of each template **node**'s execution
output. When a node is executed again with identical inputs — the same published
template version, the same effective answers and deterministic states, and the
same pinned plugins/processors — its output is reused instead of re-running
Docker. This speeds up repeated `create`/`update` runs, `update`'s old-baseline
re-execution, and reuse across templates that share a sub-template. A cache hit
is byte-identical to a non-cached run.

### Where the cache lives

By default the cache is stored under the OS-standard cache directory (resolved
via the `directories` crate), in a `cyanprint` subdirectory:

- **Linux:** `$XDG_CACHE_HOME/cyanprint`, or `~/.cache/cyanprint` if unset
- **macOS:** `~/Library/Caches/cyanprint`
- **Windows:** `%LOCALAPPDATA%\cyanprint`

The directory and its entries are created with owner-only permissions
(`0700`/`0600`). The cache is completely disposable: deleting the directory is
always safe. There is **no automatic eviction** — use `cyanprint cache clear`
(or delete the directory) to reclaim space.

### Controlling the cache

| Flag / variable        | Effect                                                            |
| ---------------------- | ----------------------------------------------------------------- |
| `--no-output-cache`    | Disable the cache for this run (always execute, never store).     |
| `--cache-dir <PATH>`   | Use a different cache directory.                                  |
| `CYANPRINT_NO_CACHE`   | Set to a truthy value (`1`/`true`/`yes`/`on`) to disable caching. |
| `CYANPRINT_CACHE`      | Override the cache directory.                                     |

Both flags are global, so they work on any command (e.g.
`cyanprint --no-output-cache update` or `cyanprint create user/template --cache-dir /tmp/cc`).
(`--no-output-cache` is deliberately distinct from the `build`/`push` `--no-cache`,
which controls the Docker buildx cache.)

### The `cyanprint cache` command

```text
cyanprint cache path    # print the resolved cache directory
cyanprint cache size    # print the total cache size (human-readable)
cyanprint cache clear   # remove all cached entries
```

### Determinism assumption (and the escape hatch)

The cache assumes template execution is **deterministic** for a given published
version + input set. Published, immutable template versions satisfy this, so a
re-run reuses them; local/dev-mode templates (the local root of `cyanprint try`,
and anything without a published version) are **never cached**, so a changed
local source is never served stale. Note that `cyanprint try group` resolves its
dependencies from the registry — those published sub-templates *are* cached, so
re-running a group try reuses unchanged dependencies. If you ever suspect a
template is non-deterministic and want to bypass the cache, run with
`--no-output-cache` (or `CYANPRINT_NO_CACHE=1`).

## Installation

### Brew
