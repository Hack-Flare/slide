# Agent Instructions

Use this file as the default guide for AI agents working in the repository.

## Human In The Loop

- Keep the user informed when making substantial changes.
- Do not commit, push, deploy, or change shared infrastructure without explicit approval.
- Do not modify unrelated user changes in the worktree.

## Quick Rules

- Inspect the source before changing behavior.
- No dead code is allowed. Remove it or comment it out. 
  - For example, `#[allow(dead_code)]` is not allowed.
- Keep changes minimal and localized.
- Follow existing code style and conventions.
- Do not rely on stale documentation when the source disagrees.
- Do not divert from the active task without being asked.
- Do not use em dashes in code, comments, or documentation.
  - Use singular hyphens instead.
  - In the case of double em dashes, use triple hyphens instead.
  - If you are editing documentation that already has em dashes, replace them with hyphens.
- No unnecessary crate dependencies, this repository is intended to be minimal and self-contained.

## Repository Layout

- `slide_server`: The Slide server implementation.
- `slide_client`: The Slide client implementation.

Read the relevant crate's source before relying on documentation for implementation details.

## Code Conventions

- Keep logic near the module that owns it.
- Avoid blocking I/O in async code.
- Prefer explicit error handling over panics.
- Add comments only when they clarify non-obvious behavior.
- No multi-line comments. Use single-line comments instead.

## Finally

Thanks for your contributions :3 