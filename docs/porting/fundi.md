# Porting the fundi agent (TypeScript to Rust)

Scope: `explore`, `plan`, `chat`, `login`, `logout`, `whoami`, and everything under them.
Source: `@nyuchi/mzizi-cli` 0.13.0 (`agent-tools/mzizi-cli`: explore, context-gather, prompts, plan,
client, tools, auth, cli-token-store, and those cases of cli.ts). Code: `src/fundi/`.
Tests: `tests/fundi_unit.rs`, `tests/fundi_cli.rs` (84 tests, no live network, no real keychain).

## Decisions

- **No SDK crates.** `@anthropic-ai/sdk` becomes a small `reqwest::blocking` client
  (`anthropic.rs`: `POST {ANTHROPIC_BASE_URL}/v1/messages`, two retries on 408/409/429/5xx and
  connection errors). `@modelcontextprotocol/sdk` becomes a minimal Streamable HTTP client
  (`mcp.rs`: `initialize`, `notifications/initialized`, `tools/call`; JSON or SSE replies; the
  `mcp-session-id` header). `jose.decodeJwt` becomes base64url + serde (claims decoded, signature
  never verified, as before). No async runtime, no OpenSSL.
- **Skills are embedded, not fetched.** The TS CLI read the `@nyuchi/mzizi-skills` npm package.
  A Rust binary has no `node_modules`, and `plan` without a key is documented as offline, so the
  binary embeds a snapshot: `skills/<name>/SKILL.md` (12 skills, 324 KB, from skills 0.24.0) plus a
  generated `src/fundi/skills_data.rs` of `include_str!`s. `scripts/refresh-skills.sh [path]`
  regenerates both; nothing is edited by hand. Fetching `api.mzizi.dev/v1/skills` at run time was
  rejected for `plan` (it would add a network dependency to an offline command and let the corpus
  drift from the binary's prompts). The two hard-coded fallback skills are kept for an empty corpus.
  A freshness check against npm/the API belongs in this repo's CI (not built here).
- **Naming (owner decision).** Binary `mz`. `MZ_MCP_URL`, `MZ_MODEL`, `MZ_AUTH_TOKEN`; the old
  `MZIZI_MCP_URL`, `MZIZI_MODEL`, `MZIZI_AUTH_TOKEN` still work and print one stderr line
  (`warning: MZIZI_MODEL is deprecated, use MZ_MODEL`). `WORKOS_ACCESS_TOKEN`, `ANTHROPIC_API_KEY`
  and `ANTHROPIC_BASE_URL` (the SDK's own variable) are unchanged. User-visible text says `mz`.
- **Token store** (`token_store.rs`). Read order: OS keychain (service `mz`, account `cli`, `keyring`
  v3 on macOS and Windows; Linux uses `secret-service` 6 over D-Bus, not the memory-only kernel keyutils), then `$XDG_CONFIG_HOME/mz/auth.json`
  (default `~/.config/mz/auth.json`, 0600, byte-compatible with what the TS CLI wrote), then the
  TS location `.../mzizi/auth.json`. Write: keychain if it works (and any stale file is removed),
  else the file. A token found only at the old path is migrated on first read (to the keychain or
  the new file), one `note:` line goes to stderr, and the old file is removed; if the migration
  cannot write, the old file stays and a one-line warning is printed. `logout` removes all three.
  `MZ_NO_KEYCHAIN=1` forces the file store (headless boxes, CI, tests).
- Stored tokens are used by `whoami` only. As in the TS CLI, `plan`/`chat` read the token from the
  environment, not from the store (`cli.ts` never passed the stored token to `createFundi`). Left
  as-is to keep this a move; worth fixing deliberately.

## Behaviour differences (all small, none in output of the happy paths)

- Error wording that came from Node or the SDKs differs: serde's JSON parse messages in
  `invalid JSON: ...` notes and `Planner JSON parse failed: ...`; Anthropic HTTP errors read
  `<status> <body>`; a failed MCP/HTTP request shows reqwest's message. `ENOENT/EACCES`-style
  messages are imitated for file errors, other I/O errors use Rust's text.
- `jose`'s JWT error texts are reproduced for the cases it distinguishes ("Invalid JWT", the
  five-part JWE message); other jose-specific messages are not.
- A set-but-empty `MZ_MCP_URL`/`MZ_MODEL` counts as unset (TS passed the empty string through).
- The system prompt's `cache_control` and tool schemas are sent as before; retries/timeouts follow
  the SDK's defaults (2 retries, 10 min) but not its backoff jitter.
- `whoami` and `login` print the same text; `login` says "to the OS keychain (service "mz")" when the
  keychain took the token.
- `plan`'s `howToFetch` text says `mz plan` where it said `mzizi plan`.
- `JSON.stringify` and serde_json agree on the printed shapes (2-space indent, key order); an
  integral `exp` prints as an integer.

## Not ported / not verified

- Nothing in the fundi surface is left out. The public SDK entry `createFundi()` (library use) is
  not re-exported as a Rust API contract; `FundiClient` is public in the crate but unstable.
- Not run against the live Anthropic API, `mcp.mzizi.dev` or a real keychain in tests. The OS
  keychain path is exercised only through a fake; `OsKeychain` compiles on macOS here and is
  otherwise untested.
- `explore` of a root where `package.json` is unreadable (permissions) is not tested.

## Test table (vitest case to Rust test)

| vitest case                                                                 | Rust test                                                                                                                                                                                          |
| --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| auth: resolveToken prefers an explicit accessToken over both env vars       | `resolve_token_prefers_explicit_over_both_env_vars`                                                                                                                                                |
| auth: falls back to WORKOS_ACCESS_TOKEN                                     | `resolve_token_falls_back_to_workos_access_token`                                                                                                                                                  |
| auth: falls back to MZIZI_AUTH_TOKEN                                        | `resolve_token_falls_back_to_auth_token_alias` (now `MZ_AUTH_TOKEN`), plus new `..._deprecated_mzizi_auth_token_still_works_and_warns_once`, `..._new_name_wins_over_deprecated_and_does_not_warn` |
| auth: returns undefined when no token                                       | `resolve_token_returns_none_when_no_token_is_available`                                                                                                                                            |
| auth: ignores whitespace-only tokens                                        | `resolve_token_ignores_whitespace_only_tokens`                                                                                                                                                     |
| auth: assertToken returns the resolved token                                | `assert_token_returns_the_resolved_token`                                                                                                                                                          |
| auth: assertToken throws AuthError                                          | `assert_token_errors_when_no_token_is_available`                                                                                                                                                   |
| auth: assertToken includes the label                                        | `assert_token_includes_the_label_in_the_message`                                                                                                                                                   |
| auth: getIdentity decodes a well-formed token                               | `get_identity_decodes_a_well_formed_workos_token`                                                                                                                                                  |
| auth: accepts organizationId camelCase                                      | `get_identity_accepts_organization_id_as_camel_case`                                                                                                                                               |
| auth: defaults permissions to []                                            | `get_identity_defaults_permissions_to_empty`                                                                                                                                                       |
| auth: filters non-string permissions                                        | `get_identity_filters_non_string_permission_entries`                                                                                                                                               |
| auth: expired token                                                         | `get_identity_errors_when_the_token_is_expired`                                                                                                                                                    |
| auth: missing sub                                                           | `get_identity_errors_when_sub_is_missing`                                                                                                                                                          |
| auth: missing exp                                                           | `get_identity_errors_when_exp_is_missing`                                                                                                                                                          |
| auth: malformed token                                                       | `get_identity_errors_when_the_token_is_malformed`                                                                                                                                                  |
| auth: bearerHeader with a token                                             | `bearer_header_returns_authorization_when_a_token_is_provided`                                                                                                                                     |
| auth: bearerHeader without a token                                          | `bearer_header_is_empty_without_a_token`                                                                                                                                                           |
| explore: reads the well-known markers                                       | `explore_reads_the_well_known_markers_and_normalises_them`                                                                                                                                         |
| explore: non-existent root surfaces no throw                                | `explore_surfaces_a_missing_root_as_empty_not_an_error`                                                                                                                                            |
| context-gather: returns snapshot and goal untouched                         | `gather_context_returns_snapshot_and_goal_untouched`                                                                                                                                               |
| context-gather: matches skills sharing a word                               | `gather_context_matches_skills_sharing_a_word_with_the_goal`                                                                                                                                       |
| context-gather: matches nothing for unrelated goal                          | `gather_context_matches_nothing_for_unrelated_words`                                                                                                                                               |
| context-gather: ignores stopwords and short words                           | `gather_context_ignores_stopwords_and_short_words`                                                                                                                                                 |
| describe-skill: frontmatter description, first sentence                     | `describe_skill_reads_the_frontmatter_description_cut_at_first_sentence`                                                                                                                           |
| describe-skill: falls back to first prose line                              | `describe_skill_falls_back_to_first_prose_line_when_description_is_empty`                                                                                                                          |
| describe-skill: fast on long whitespace (ReDoS)                             | `describe_skill_stays_fast_on_a_long_run_of_whitespace` (Rust's regex is linear-time); new `describe_skill_caps_at_160_characters`                                                                 |
| plan-steps: Step type guards narrow                                         | `step_kinds_narrow_correctly` (enum variants replace the guards)                                                                                                                                   |
| plan-steps: typed param access                                              | `step_params_stay_typed_after_narrowing`                                                                                                                                                           |
| plan-steps: fenced plan block                                               | `extract_steps_parses_a_fenced_plan_block`                                                                                                                                                         |
| plan-steps: bare JSON array                                                 | `extract_steps_falls_back_to_a_bare_json_array`                                                                                                                                                    |
| plan-steps: unknown kind                                                    | `extract_steps_rejects_unknown_kinds`                                                                                                                                                              |
| plan-steps: install-component without components                            | `extract_steps_rejects_install_component_without_components`                                                                                                                                       |
| plan-steps: no fenced block and no array                                    | `extract_steps_errors_without_a_fence_or_array`                                                                                                                                                    |
| tools: exposes all six tools                                                | `tool_registry_exposes_all_six_expected_tools`                                                                                                                                                     |
| tools: resolveWithin permits inside                                         | `resolve_within_permits_paths_inside_the_root`                                                                                                                                                     |
| tools: resolveWithin refuses escapes                                        | `resolve_within_refuses_paths_that_escape_the_root`                                                                                                                                                |
| tools: read_file                                                            | `dispatch_read_file_returns_file_contents`                                                                                                                                                         |
| tools: list_dir sorted, `/` on dirs                                         | `dispatch_list_dir_returns_sorted_entries_with_slash_on_dirs`                                                                                                                                      |
| tools: refuses unknown tools                                                | `dispatch_refuses_unknown_tools`                                                                                                                                                                   |
| tools: blocks write_file read-only                                          | `dispatch_blocks_write_file_in_read_only_mode`                                                                                                                                                     |
| tools: blocks run_shell read-only                                           | `dispatch_blocks_run_shell_in_read_only_mode`                                                                                                                                                      |
| tools: refuses paths outside the root                                       | `dispatch_refuses_paths_outside_the_project_root`                                                                                                                                                  |
| tools: fetch_mzizi_skill known                                              | `dispatch_fetch_mzizi_skill_returns_body_for_a_known_skill`                                                                                                                                        |
| tools: fetch_mzizi_skill unknown                                            | `dispatch_fetch_mzizi_skill_errors_on_unknown_name`                                                                                                                                                |
| mcp-tool-name: a tool mzizi-mcp registers, running an operation it declares | `fetch_component_calls_the_registered_tool_and_operation_with_name` (asserted on the wire: the standalone repo cannot read mzizi-mcp's source)                                                     |
| mcp-tool-name: takes `name`, not `p_name`                                   | same test (`arguments.arguments == {name}`, no `p_name`)                                                                                                                                           |
| cli-plan-add-output: plan prints a summary, no bodies                       | `plan_without_a_key_prints_a_summary_not_skill_bodies`                                                                                                                                             |
| cli-plan-add-output: `--full`                                               | `plan_full_prints_matching_skills_in_full`                                                                                                                                                         |
| cli-plan-add-output: `--skill <name>`                                       | `plan_skill_prints_one_skill_without_a_goal`                                                                                                                                                       |
| cli-plan-add-output: unknown `--skill`                                      | `plan_skill_with_an_unknown_name_lists_the_available_ones`                                                                                                                                         |

The `add` half of cli-plan-add-output is the add fork's. New tests with no vitest ancestor: SSE and
HTTP-fallback component fetches, both-fail message, install argv, chat request shape, `MZ_*`/`MZIZI_*`
fallbacks (model, MCP URL, auth token), invalid-token warning, the full planner loop against fixture
servers, iteration cap, real apply and path escape, login/whoami/logout, TS auth-file compatibility,
keychain preference and fallback, legacy-file migration (file and keychain), ISO dates.
