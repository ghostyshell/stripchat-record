# Claude Code instructions

The instructions for this repo live in **[AGENTS.md](AGENTS.md)** - the single
source of truth shared by Claude Code, Cursor, and OpenCode. Read it now.

Quick orientation for Claude Code:

- **Before a push** run the `sync-docs` skill to audit `README.md`,
  `CHANGELOG.md`, and `AGENTS.md` against the code.
- **Before exploring code** run `graphify query` (see `.cursor/rules/graphify.mdc`);
  after code changes run `graphify update .`.
- **After substantive changes** run the `code-reviewer` agent (`security-auditor`
  for input/secret/network surface) and add a `CHANGELOG.md` `[Unreleased]` entry.
- Recordings default to **1 GiB split parts** (`--split-size`); keep that accurate
  in the docs.
- No em/en dashes in public-facing copy (`rg '[—–]'`).
