# GPT-6 Model Instructions

These instructions apply when GPT-6 (including GPT-6 Astra) works on the LoreLens repository.

## Instruction priority

- Follow the user's explicit request first.
- Follow `AGENTS.md` and these instructions unless they conflict with the user's request.
- If a conflict affects the result materially, identify it clearly and use the higher-priority instruction.

## Work style

- Infer the user's intended scope from the request and conversation, then carry authorized work through to completion.
- Inspect the relevant files and current working-tree status before editing.
- Preserve unrelated user changes. Make the smallest coherent change that solves the task.
- Ask a focused question only when the answer would materially change the result or when required authorization is missing.
- State assumptions when they affect the implementation.

## LoreLens project rules

- Keep the Rust 2024 GPUI application organized according to the existing module ownership.
- Keep Lore CLI invocation, filesystem operations, and ignore handling in `lorelens/src/backend/`.
- Preserve local-first browsing and preview behavior when the Lore CLI is unavailable.
- Invoke external commands with direct argument lists; never route them through a shell.
- Keep English UI text in `i18n/en-US.json` and keep the Korean and Simplified Chinese catalogs aligned.
- Preserve interpolation placeholder names across localization catalogs.
- Do not log credentials, authentication tokens, or other secrets.

## Implementation and verification

- Use `apply_patch` for local file edits.
- For Rust changes, run the checks relevant to the modified code, starting with formatting, Clippy, and targeted tests when practical.
- Run broader checks only when the change or a failure justifies them.
- Report what changed and which checks passed or failed.

## Communication

- Lead with the outcome.
- Use concise paragraphs and plain language. Use lists only when they improve scanning.
- Match the response detail to the user's background and the task's complexity.
- Do not claim a check passed unless it was actually run.

## Safety

- Treat deletion, discard, revert, obliterate, publishing, and external writes as consequential actions.
- Retain existing validation and confirmation behavior for destructive flows.
- Before a destructive action, verify its exact target and confirm that it is within the requested scope.

Reference: [OpenAI model guidance](https://developers.openai.com/api/docs/guides/latest-model)
