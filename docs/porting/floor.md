# Porting table: floor check (TS `check`, `check-spacing`, `spacing-floor`)

Source: `@nyuchi/mzizi-cli` 0.13.0, `src/{check,check-spacing,spacing-floor}.ts`.
Rust: `src/floor/{mod,scan,tokens}.rs`. Tests: `tests/floor_scan.rs`, `tests/floor_check.rs`.
Command: `mz floor` (alias `check`, which the TS bins keep). Run `cargo test --test floor_scan --test floor_check`.

Beyond the cases below, the port was diffed against the TypeScript on real source trees
(`mzizi-registry`, `packages-npm`, `mzizi`, `agent-tools`; 1,858 files, about 1,500 violations):
`--json` and the human report were byte-identical.

## check-spacing.test.ts -> tests/floor_scan.rs

| vitest case                                                                                                | Rust test                                                                          |
| ---------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| scanText: Tailwind utilities / flags p-0 and the zero forms of every padding side                          | `flags_p0_and_the_zero_forms_of_every_padding_side`                                |
| ... holds a line with no layer to 14px padding, and accepts 14px                                           | `holds_a_line_with_no_layer_to_14px_padding_and_accepts_14px`                      |
| ... holds a card to its default floor of 21px, and a control to 17px                                       | `holds_a_card_to_21px_and_a_control_to_17px`                                       |
| ... holds a dense line to the dense floor, and a default line to the default floor                         | `holds_a_dense_line_to_the_dense_floor_and_a_default_line_to_the_default_floor`    |
| ... places a line by its layer token before its element                                                    | `places_a_line_by_its_layer_token_before_its_element`                              |
| ... holds a control's default gap to 7px, and leaves unlayered gaps at the 3px hard floor                  | `holds_a_controls_default_gap_and_leaves_unlayered_gaps_at_the_floor`              |
| ... flags a control drawn under the 24px desktop hit floor as an error, and under 45px as a warning        | `flags_a_control_under_the_24px_hit_floor_as_an_error_and_under_45px_as_a_warning` |
| ... warns, never fails, on a value that is not a multiple of 3, a multiple of 7 or a prime                 | `warns_never_fails_on_a_value_that_is_not_a_mzizi_number`                          |
| ... does not flag small vertical padding                                                                   | `does_not_flag_small_vertical_padding`                                             |
| ... flags gaps under the 7px floor of a line with no layer, including space-x/y, and accepts 7px           | `flags_gaps_under_the_7px_floor_of_a_line_with_no_layer_including_space_xy`        |
| ... flags negative margins of every side, not zero or positive ones                                        | `flags_negative_margins_of_every_side_not_zero_or_positive_ones`                   |
| ... flags leading under 1.5 and accepts normal, relaxed, loose and fixed lengths                           | `flags_leading_under_1_5_and_accepts_normal_relaxed_loose_and_fixed_lengths`       |
| ... honours the configured spacing unit                                                                    | `honours_the_configured_spacing_unit`                                              |
| ... does not flag classes that merely look similar                                                         | `does_not_flag_classes_that_merely_look_similar`                                   |
| ... reports line and column                                                                                | `reports_line_and_column`                                                          |
| scanText: CSS and style props / evaluates padding shorthands by side                                       | `evaluates_padding_shorthands_by_side`                                             |
| ... skips values it cannot evaluate                                                                        | `skips_values_it_cannot_evaluate`                                                  |
| ... flags gap, row-gap and column-gap under the 7px floor of a line with no layer                          | `flags_gap_row_gap_and_column_gap_under_the_7px_floor_of_a_line_with_no_layer`     |
| ... flags negative margins in any shorthand slot                                                           | `flags_negative_margins_in_any_shorthand_slot`                                     |
| ... flags unitless and percent line heights under 1.5                                                      | `flags_unitless_and_percent_line_heights_under_1_5`                                |
| ... reads JSX style props, where a bare number is pixels                                                   | `reads_jsx_style_props_where_a_bare_number_is_pixels`                              |
| ... reads @apply lines                                                                                     | `reads_apply_lines`                                                                |
| scanText: comments and exceptions / ignores code in comments                                               | `ignores_code_in_comments`                                                         |
| ... does not read a glob as a comment                                                                      | `does_not_read_a_glob_as_a_comment`                                                |
| ... excuses a finding with a reason on the same or the previous line                                       | `excuses_a_finding_with_a_reason_on_the_same_or_the_previous_line`                 |
| ... rejects an exception with no reason, and keeps the finding                                             | `rejects_an_exception_with_no_reason_and_keeps_the_finding`                        |
| ... excuses a whole file from the first 20 lines only                                                      | `excuses_a_whole_file_from_the_first_20_lines_only`                                |
| ... notes the legacy opt-out without excusing anything                                                     | `notes_the_legacy_opt_out_without_excusing_anything`                               |
| scanText: review fixes / a marker with no reason excuses nothing, even with code after the comment         | `a_marker_with_no_reason_excuses_nothing_even_with_code_after_the_comment`         |
| ... a trailing exception does not carry down to the next code line                                         | `a_trailing_exception_does_not_carry_down_to_the_next_code_line`                   |
| ... a standalone comment on the line above still excuses the next line                                     | `a_standalone_comment_on_the_line_above_still_excuses_the_next_line`               |
| ... a marker in a string or in prose excuses nothing                                                       | `a_marker_in_a_string_or_in_prose_excuses_nothing`                                 |
| ... reads past a protocol-relative URL in CSS when // is not a comment                                     | `reads_past_a_protocol_relative_url_in_css_when_slashes_are_not_a_comment`         |
| ... sees a block comment right after a value                                                               | `sees_a_block_comment_right_after_a_value`                                         |
| ... does not flag line-height: 0                                                                           | `does_not_flag_line_height_0`                                                      |
| scanText: globs are not comments / does not let src/** blank the rest of the file                          | `does_not_let_a_src_glob_blank_the_rest_of_the_file`                               |
| scanText: exemption reasons / accepts the HTML-legal --!> closer and long trailing whitespace              | `accepts_the_html_legal_closer_and_long_trailing_whitespace`                       |
| scanText: label gap and sibling items / holds a label-to-field gap to 7px by default and 5px dense         | `holds_a_label_to_field_gap_to_7px_by_default_and_5px_dense`                       |
| ... holds sibling items in a list, grid or toolbar to the card gap                                         | `holds_sibling_items_to_the_card_gap_14px_dense_7px`                               |
| scanText: the review of 2026-11-19 / holds an unplaced line to 14px padding and a 7px gap, at any density  | `holds_an_unplaced_line_to_14px_padding_and_a_7px_gap_at_any_density`              |
| ... holds a layer with no gap token (chips) to the unplaced 7px gap                                        | `holds_a_layer_with_no_gap_token_chips_to_the_unplaced_7px_gap`                    |
| ... holds the gap between adjacent controls to 9px, dense too                                              | `holds_the_gap_between_adjacent_controls_to_9px_dense_too`                         |
| ... holds the icon gap and the label gap only where the line names their token                             | `holds_the_icon_gap_and_the_label_gap_only_where_the_line_names_their_token`       |
| ... checks a control's hit region at dense density too                                                     | `checks_a_controls_hit_region_at_dense_density_too`                                |
| ... places a line by a class name, not by a word inside another class                                      | `places_a_line_by_a_class_name_not_by_a_word_inside_another_class`                 |
| ... reads the number rule: 0, 1, 2, 3, 5 and 7                                                             | `reads_the_number_rule`                                                            |
| ... parses a 50,000-character token in well under 200ms                                                    | `parses_a_50000_character_token_in_well_under_200ms`                               |
| scanText: shorthand sides... / errors on a horizontal floor breach even when the vertical side warns first | `errors_on_a_horizontal_floor_breach_even_when_the_vertical_side_warns_first`      |
| ... holds a card's padding to the cards floor of 21                                                        | `holds_a_cards_padding_to_the_cards_floor_of_21`                                   |
| ... falls back to a number-rule warning only when no side is under its floor                               | `falls_back_to_a_number_rule_warning_only_when_no_side_is_under_its_floor`         |
| ... attaches a named gap token only to the gap in its own segment                                          | `attaches_a_named_gap_token_only_to_the_gap_in_its_own_segment`                    |
| ... does not place a line from a property path such as styles.badge or theme.page                          | `does_not_place_a_line_from_a_property_path_such_as_styles_badge_or_theme_page`    |
| ... reads the dense token boundary exactly                                                                 | `reads_the_dense_token_boundary_exactly`                                           |
| ... reads the nit values: a control gap under 9px is held at 9                                             | `reads_the_nit_values_a_control_gap_under_9px_is_held_at_9`                        |
| scanText: hostile inputs stay linear (10 generated cases, one `it` each)                                   | `hostile_inputs_stay_linear` (one test looping the same 10 shapes)                 |

The two time-bound cases assert 200ms in release builds and 2000ms in debug builds
(`cfg!(debug_assertions)`): the `regex` crate is far slower unoptimised. Measured in release the
whole `floor_scan` suite finishes in 0.34s.

## check.test.ts -> tests/floor_check.rs

| vitest case                                                                                    | Rust test                                                                                                                                                                                              |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| checkProject / passes a project at the floor                                                   | `passes_a_project_at_the_floor`                                                                                                                                                                        |
| ... reports file, line and column, sorted, with posix paths                                    | `reports_file_line_and_column_sorted_with_posix_paths`                                                                                                                                                 |
| ... keeps the 0.11 --json floor keys as aliases of the layer floors                            | `keeps_the_0_11_json_floor_keys_as_aliases_of_the_layer_floors`                                                                                                                                        |
| ... skips build output, dependencies, symlinks and unrelated files, but reads an explicit file | `skips_build_output_dependencies_symlinks_and_unrelated_files_but_reads_an_explicit_file` (unix)                                                                                                       |
| ... lists exempted findings and the legacy notice, and still passes                            | `lists_exempted_findings_and_the_legacy_notice_and_still_passes`                                                                                                                                       |
| ... follows a symlink the user names, and fails rather than pass having read nothing           | `follows_a_symlink_the_user_names_and_fails_rather_than_pass_having_read_nothing` (unix)                                                                                                               |
| ... --exclude takes relative, ./, .. and absolute paths                                        | `exclude_takes_relative_dot_slash_trailing_slash_and_absolute_paths`                                                                                                                                   |
| ... reads .ts and .js files, but not .d.ts                                                     | `reads_ts_and_js_files_but_not_d_ts`                                                                                                                                                                   |
| ... rejects a path that does not exist                                                         | `rejects_a_path_that_does_not_exist`                                                                                                                                                                   |
| mzizi check (CLI) / exits clean (resolves) when nothing is below the floor                     | `cli_exits_clean_when_nothing_is_below_the_floor`                                                                                                                                                      |
| ... throws CheckFailed after printing when something is below the floor                        | `cli_fails_after_printing_when_something_is_below_the_floor`                                                                                                                                           |
| ... --json prints one parseable report and still fails                                         | `cli_json_prints_one_parseable_report_and_still_fails`                                                                                                                                                 |
| ... takes paths, --exclude, --spacing-unit and --rem                                           | `cli_takes_paths_exclude_spacing_unit_and_rem` (adds a `--rem` assertion the vitest case lacked)                                                                                                       |
| ... accepts --exclude=dir and prints help for -h                                               | `cli_accepts_exclude_equals_dir_and_prints_help_for_h`                                                                                                                                                 |
| ... rejects bad usage with a CliError                                                          | `cli_rejects_bad_usage_with_a_message`                                                                                                                                                                 |
| ... is documented in --help                                                                    | `cli_is_documented_in_help`                                                                                                                                                                            |
| the floor the CLI enforces / is the set of layer tokens the mzizi-design skill teaches         | `the_floor_is_the_set_of_layer_tokens_the_mzizi_design_skill_teaches` (reads the vendored `tests/fixtures/mzizi-design-SKILL.md`; the TS read `../../mzizi-skills/...` and skipped itself when absent) |

New (no vitest equivalent; JavaScript gave these for free): `json_matches_json_stringify_layout_and_key_order`,
`json_pretty_and_js_num_follow_javascript`.

Not ported as a test: nothing. The vitest `cli-help-parity`, `cli-bin-symlink` and `cli-version`
suites are the CLI shell's, not this module's.

## Differences from the TypeScript (all deliberate, none in the checked behaviour)

- `leading-<name>` looked the name up in a plain JS object, so `leading-constructor` or
  `leading-toString` reached `leadingHit` with a function and produced a NaN finding. The port
  only knows the six Tailwind names; those inputs are ignored.
- `\s` and `trim()` follow Rust's Unicode White_Space, JS's differ only on U+FEFF.
- A file name sort is by UTF-8 bytes, JS sorts by UTF-16 code units: the same except for names that
  mix astral characters with U+E000..U+FFFF.
- Lookbehind and lookahead regexes were ported by hand (see comments in `scan.rs`).
- A class-attribute value window is 2048 characters, JS used 2048 UTF-16 units (differs only for astral characters).
- I/O error text (`ENOENT: ...`) is Rust's, not Node's.
