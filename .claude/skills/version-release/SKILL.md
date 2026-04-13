---
name: version-release
description: Use when bumping version, creating a release tag, or pushing a version tag. Triggers on "version up", "release", "tag push", "bump version".
---

# Version Release

Workflow for updating the version in Cargo.toml, creating a git tag, and pushing it.

## Workflow

1. **Check current version**: Get current version with `grep '^version' Cargo.toml`
2. **Update version**: Edit the `version` field in Cargo.toml using the Edit tool (according to patch/minor/major)
3. **Sync the Claude Code plugin manifest**: Edit `.claude-plugin/plugin.json` and update its `version` field to the same value. The two MUST stay in lockstep — `tests/plugin_hooks_tests.rs` enforces this. Without bumping plugin.json, users who installed via `/plugin install` will not see the new release because Claude Code uses this field for update detection.
4. **Regenerate Cargo.lock**: Run `cargo check` to update Cargo.lock
5. **Run checks**: Run `cargo fmt --check && cargo clippy && cargo test`
6. **Commit**: Commit `Cargo.toml`, `Cargo.lock`, and `.claude-plugin/plugin.json` (message example: `Bump version to X.Y.Z`)
7. **Create and push tag**: `git tag vX.Y.Z && git push && git push origin vX.Y.Z`

## Quick Reference

| Bump type | Example       |
|-----------|---------------|
| patch     | 0.2.0 → 0.2.1 |
| minor     | 0.2.0 → 0.3.0 |
| major     | 0.2.0 → 1.0.0 |

## Notes

- Tags use the `v` prefix (e.g., `v0.2.0`)
- Always pass CI checks (fmt, clippy, test) before creating a tag
- Commit `Cargo.lock` together with `Cargo.toml`
