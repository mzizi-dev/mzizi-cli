# Porting `add`: vitest case to Rust test

Source: `@nyuchi/mzizi-cli` 0.13.0 (`agent-tools/mzizi-cli`). Rust: `src/add/{mod,target,registry,install}.rs`.
Unit tests sit beside the code (`cargo test --lib add::`); integration and golden tests are in
`tests/add_golden.rs`; the end-to-end diff against the TypeScript CLI is `scripts/parity-add.sh`.

## test/registry.test.ts (10 cases)

| vitest case                                                  | Rust test                                                                               |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------- |
| fetchItem: returns the document at the canonical /v1 address | `registry::tests::fetch_item_returns_the_document_at_the_canonical_v1_address`          |
| fetchItem: propagates a 404                                  | `registry::tests::fetch_item_propagates_a_404`                                          |
| fetchItem: rejects a 200 that is not a registry document     | `registry::tests::fetch_item_rejects_a_200_that_is_not_a_registry_document`             |
| resolveClosure: dependencies before dependents               | `registry::tests::returns_dependencies_before_the_items_that_need_them`                 |
| resolveClosure: diamond fetched once                         | `registry::tests::fetches_a_diamond_dependency_exactly_once`                            |
| resolveClosure: terminates on a cycle                        | `registry::tests::terminates_on_a_dependency_cycle`                                     |
| resolveClosure: refuses a bare dependency name               | `registry::tests::refuses_a_bare_dependency_name_and_says_what_it_would_have_installed` |
| resolveClosure: refuses a non-https URL                      | `registry::tests::refuses_a_non_https_dependency_url`                                   |
| resolveClosure: names the requiring component on failure     | `registry::tests::names_the_component_that_pulled_in_a_dependency_that_failed`          |
| resolveClosure: stops when the closure explodes              | `registry::tests::stops_when_the_closure_explodes`                                      |

## test/target.test.ts (26 cases)

| vitest case                                          | Rust test (in `target::tests`)                           |
| ---------------------------------------------------- | -------------------------------------------------------- |
| detectTarget: mzizi from Cargo.toml                  | `infers_mzizi_from_cargo_toml_with_no_package_json`      |
| detectTarget: tsx from components.json               | `infers_tsx_from_components_json`                        |
| detectTarget: tsx from a react dependency            | `infers_tsx_from_a_react_dependency`                     |
| detectTarget: refuses Cargo.toml beside package.json | `refuses_when_cargo_toml_sits_beside_package_json`       |
| detectTarget: Astro project                          | `targets_astro_for_an_astro_project`                     |
| detectTarget: Astro even with React                  | `targets_astro_even_when_react_is_also_present`          |
| detectTarget: bare directory                         | `gives_up_with_an_actionable_reason_on_a_bare_directory` |
| registryItemUrl: routes                              | `routes_tsx_to_ui_mzizi_to_rs_and_astro_to_astro`        |
| registryItemUrl: encodes the name                    | `encodes_the_name_so_it_cannot_escape_the_route`         |
| registryItemUrl: rejects unknown targets (isTarget)  | `rejects_anything_that_is_not_a_known_target`            |
| types: order                                         | `lists_every_served_type_in_that_order`                  |
| types: old and long spellings, not dioxus            | `accepts_old_and_long_spellings_and_not_dioxus`          |
| types: native types from /ui                         | `serves_native_types_from_the_ui_document`               |
| declaredTypes: tsx                                   | `declared_is_tsx_for_tsx_and_ts`                         |
| declaredTypes: swift, kotlin                         | `declared_is_swift_for_swift_and_kotlin_for_kt`          |
| declaredTypes: empty                                 | `declared_is_empty_for_no_files`                         |
| detectTarget: swift                                  | `infers_swift_from_package_swift`                        |
| detectTarget: kotlin                                 | `infers_kotlin_from_a_gradle_file`                       |
| chooseTarget: --type when available                  | `uses_type_when_the_component_has_it`                    |
| chooseTarget: refuses a type the component lacks     | `refuses_a_requested_type_the_component_lacks`           |
| chooseTarget: project's type                         | `takes_the_projects_type_when_the_component_has_it`      |
| chooseTarget: the only type                          | `takes_the_only_type_a_component_has`                    |
| chooseTarget: several types, no signal               | `refuses_several_types_with_no_signal`                   |
| chooseTarget: no type at all                         | `refuses_a_component_with_no_type_at_all`                |
| chooseTarget: Astro first-class                      | `treats_astro_as_a_first_class_type`                     |

## test/install.test.ts (20 cases)

| vitest case                                       | Rust test (in `install::tests`)                                     |
| ------------------------------------------------- | ------------------------------------------------------------------- |
| path safety: escapes the root                     | `refuses_a_file_whose_target_escapes_the_project_root`              |
| path safety: absolute path refused, not relocated | `refuses_an_absolute_target_path_rather_than_relocating_it`         |
| path safety: the two root prefixes                | `accepts_the_two_prefixes_that_mean_the_project_root`               |
| path safety: same destination twice               | `refuses_when_two_components_claim_the_same_destination`            |
| targets: npm deps deduplicated and sorted         | `collects_npm_dependencies_deduplicated_and_sorted`                 |
| targets: no .rs copy under mzizi                  | `does_not_copy_rs_component_sources_rust_is_a_crate_dependency`     |
| targets: token artifacts copied                   | `does_copy_the_rust_token_artifacts`                                |
| targets: retired dioxus tolerated                 | `tolerates_a_document_that_still_names_the_retired_dioxus_target`   |
| targets: swift and kotlin files                   | `installs_the_swift_and_kotlin_files_an_item_declares`              |
| targets: tsx-only under swift                     | `refuses_a_tsx_only_item_under_type_swift`                          |
| targets: .rs copied under tsx                     | `still_copies_rs_sources_when_the_target_is_tsx`                    |
| targets: no content warns                         | `warns_rather_than_silently_dropping_a_file_with_no_content`        |
| applyPlan: writes and mkdirs                      | `writes_planned_files_and_creates_missing_directories`              |
| applyPlan: no overwrite by default                | `does_not_overwrite_an_existing_file_unless_asked`                  |
| applyPlan: overwrite                              | `overwrites_when_overwrite_was_passed`                              |
| renderPlan: names the requirer                    | `render_names_the_component_that_pulled_in_each_dependency`         |
| Astro: writes .astro, decodes base64              | `writes_astro_files_and_decodes_base64_assets_byte_for_byte`        |
| Astro: refuses stepping off the target            | `refuses_a_closure_that_steps_off_the_astro_target_naming_the_item` |
| labels: skip (exists)                             | `says_skip_exists_not_overwrite_when_overwrite_is_not_passed`       |
| labels: overwrite                                 | `says_overwrite_only_when_overwrite_will_replace_the_file`          |

## test/cli-plan-add-output.test.ts, the `add` cases (5 of 9; the 4 `plan` cases belong to the fundi port)

| vitest case                                       | Rust test (`tests/add_golden.rs`)                                             |
| ------------------------------------------------- | ----------------------------------------------------------------------------- |
| add --dry-run on the mzizi type (#282)            | `dry_run_on_the_mzizi_type_says_nothing_would_be_written_and_shows_cargo_add` |
| refuses --type dioxus without fetching            | `refuses_type_dioxus_listing_the_types_without_fetching`                      |
| refuses several types, no signal                  | `refuses_a_component_with_several_types_and_no_signal_listing_them`           |
| installs the only type a native item has          | `installs_the_only_type_a_native_item_has_with_no_flag`                       |
| names the types when --type asks for one it lacks | `names_the_components_types_when_type_asks_for_one_it_lacks`                  |

## test/registry-url.test.ts (5 cases): not ported here

These test `registryComponentUrl`, `REGISTRY_HTTP_BASE` and `registryInstallArgv` in `tools.ts`, which is the
fundi agent's `install-component` tool, not `add`. They belong to the fundi port. The URL-encoding case is covered
for `add`'s own URL builder by `target::tests::encodes_the_name_so_it_cannot_escape_the_route`.

## New tests (no vitest equivalent)

- `registry::tests::available_types_*` and `crate_exists_*`: `availableTypes` and `crateExists` had no unit tests.
- `install::tests::base64_decoding_is_as_lenient_as_nodes`.
- `tests/add_golden.rs`: golden runs on recorded registry responses (`button` at ui/rs/astro, `ui-variants`, `input`,
  `address-input`, `mzizi-tokens-swift`, crates.io `mzizi-ui`): dry run, write with closure, Rust type, Astro, Swift,
  skip/overwrite, unpublished crate, unreachable crates.io, missing `--type` value, no names, and the real HTTP
  transport against a loopback server.

## Parity with the TypeScript CLI

`scripts/parity-add.sh` runs both CLIs on 14 scenarios against the same fixtures and diffs stdout, stderr, exit
code and every file written. Result at the time of writing: 14/14 identical.

## Intended differences

- The user-facing name is `mz add`, not `mzizi add` (the plan header and the usage error). The script normalises
  exactly this and nothing else.
- Test hook: `MZ_REGISTRY_BASE` and `MZ_CRATES_BASE` redirect the transport (new; the TS client had no override).
- Transport errors (DNS, TLS, timeout) read as reqwest's text, not Node's `fetch failed`.
- `availableTypes` probes on three threads (TS used `Promise.all`).
- Not ported: JavaScript quirks that were bugs, such as `parseTarget("constructor")` resolving through the object
  prototype, and `package.json` whose `dependencies` is not an object being spread character by character.
