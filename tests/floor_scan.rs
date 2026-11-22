//! Port of `test/check-spacing.test.ts` (vitest) case by case; the mapping is in
//! docs/porting/floor.md.
#![allow(clippy::needless_raw_string_hashes)]

use std::time::Instant;

use mzizi_cli::floor::scan::{Finding, ScanOptions, ScanResult, is_dense_line, scan_text};
use mzizi_cli::floor::tokens::{Layer, is_mzizi_number, next_mzizi_number};

fn scan(t: &str) -> ScanResult {
    scan_text(t, &ScanOptions::default())
}
fn rules(t: &str) -> Vec<String> {
    scan(t)
        .findings
        .iter()
        .map(|f| format!("{}:{}", f.line, f.rule))
        .collect()
}
fn rule_names(t: &str) -> Vec<&'static str> {
    scan(t).findings.iter().map(|f| f.rule).collect()
}
fn first(t: &str) -> Finding {
    scan(t).findings.into_iter().next().expect("a finding")
}
fn v(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// ---- scanText: Tailwind utilities ----

#[test]
fn flags_p0_and_the_zero_forms_of_every_padding_side() {
    assert_eq!(rules(r#"<div class="p-0">"#), v(&["1:padding-zero"]));
    assert_eq!(
        rules(r#"<div class="py-0 pt-0">"#),
        v(&["1:padding-zero", "1:padding-zero"])
    );
    assert_eq!(
        rules(r#"<div class="hover:md:!px-0">"#),
        v(&["1:padding-zero"])
    );
}

#[test]
fn holds_a_line_with_no_layer_to_14px_padding_and_accepts_14px() {
    assert_eq!(
        rules(r#"<div class="px-1 pl-4 p-1">"#),
        v(&["1:padding-below-inset", "1:padding-below-inset"])
    );
    assert!(rules(r#"<div class="px-5 pl-4 p-4 pl-[14px] px-[1rem]">"#).is_empty());
    assert_eq!(
        rules(r#"<div class="px-[4px]">"#),
        v(&["1:padding-below-inset"])
    );
}

#[test]
fn holds_a_card_to_21px_and_a_control_to_17px() {
    let card = scan(r#"<Card className="px-3 p-4">"#);
    let got: Vec<_> = card
        .findings
        .iter()
        .map(|f| (f.rule, f.layer, f.minimum))
        .collect();
    assert_eq!(
        got,
        vec![
            ("padding-below-inset", Some(Layer::Cards), Some(21.0)),
            ("padding-below-inset", Some(Layer::Cards), Some(21.0)),
        ]
    );
    assert!(
        scan(r#"<Card className="p-6 px-[21px]">"#)
            .findings
            .is_empty()
    );
    assert!(
        scan(r#"<button className="px-[17px] py-2">"#)
            .findings
            .is_empty()
    );
    let b = first(r#"<button className="px-3">"#);
    assert_eq!(
        (b.rule, b.layer, b.minimum),
        ("padding-below-inset", Some(Layer::Controls), Some(17.0))
    );
    let d = first(r#"<dialog className="p-5">"#);
    assert_eq!(
        (d.rule, d.layer, d.minimum),
        ("padding-below-inset", Some(Layer::Containers), Some(28.0))
    );
}

#[test]
fn holds_a_dense_line_to_the_dense_floor_and_a_default_line_to_the_default_floor() {
    assert!(
        scan(r#"<Chip className="dense px-[9px]">"#)
            .findings
            .is_empty()
    );
    let a = first(r#"<Chip className="dense px-[8px]">"#);
    assert_eq!(
        (a.rule, a.layer, a.minimum),
        ("padding-below-inset", Some(Layer::Chips), Some(9.0))
    );
    let b = first(r#"<Chip className="px-[10px]">"#);
    assert_eq!(
        (b.rule, b.layer, b.minimum),
        ("padding-below-inset", Some(Layer::Chips), Some(12.0))
    );
}

#[test]
fn places_a_line_by_its_layer_token_before_its_element() {
    assert!(
        scan(r#"<Card style={{ padding: "var(--space-container)" }}>"#)
            .findings
            .is_empty()
    );
    let css = scan(".x { gap: 6px; } /* a comment */\n.y { --space-control-gap: 6px; gap: 6px }");
    let names: Vec<_> = css.findings.iter().map(|f| f.rule).collect();
    assert_eq!(names, vec!["gap-below-floor", "gap-below-floor"]);
}

#[test]
fn holds_a_controls_default_gap_and_leaves_unlayered_gaps_at_the_floor() {
    assert_eq!(
        rules(r#"<button className="gap-1 gap-x-3">"#),
        v(&["1:gap-below-floor"])
    );
    assert!(rules(r#"<div className="gap-2">"#).is_empty());
}

#[test]
fn flags_a_control_under_the_24px_hit_floor_as_an_error_and_under_45px_as_a_warning() {
    let tiny = scan(r#"<button className="h-5">"#);
    assert_eq!(
        tiny.findings
            .iter()
            .map(|f| (f.rule, f.minimum))
            .collect::<Vec<_>>(),
        vec![("hit-floor", Some(24.0))]
    );
    let touch = scan(r#"<button className="h-8">"#);
    assert!(touch.findings.is_empty());
    assert_eq!(
        touch
            .warnings
            .iter()
            .map(|f| (f.rule, f.severity.as_str(), f.minimum))
            .collect::<Vec<_>>(),
        vec![("hit-floor-touch", "warning", Some(45.0))]
    );
    assert!(
        scan(r#"<button className="h-12 size-12">"#)
            .warnings
            .is_empty()
    );
    assert!(scan(r#"<div className="h-6">"#).warnings.is_empty());
    assert_eq!(
        rule_names(".t { --target-floor: 1rem; }"),
        vec!["hit-floor"]
    );
}

#[test]
fn warns_never_fails_on_a_value_that_is_not_a_mzizi_number() {
    let r = scan(r#"<div className="py-1 mt-2 h-10">"#);
    assert!(r.findings.is_empty());
    assert_eq!(
        r.warnings
            .iter()
            .map(|f| (f.rule, f.severity.as_str(), f.value, f.minimum))
            .collect::<Vec<_>>(),
        vec![
            ("number-rule", "warning", Some(4.0), Some(5.0)),
            ("number-rule", "warning", Some(8.0), Some(9.0)),
            ("number-rule", "warning", Some(40.0), Some(41.0)),
        ]
    );
    assert!(
        scan(r#"<div className="p-3 m-6 p-0 h-7 px-[11px] px-[13px]">"#)
            .warnings
            .is_empty()
    );
}

#[test]
fn does_not_flag_small_vertical_padding() {
    assert!(rules(r#"<span class="py-0.5 pt-1 pb-px">"#).is_empty());
}

#[test]
fn flags_gaps_under_the_7px_floor_of_a_line_with_no_layer_including_space_xy() {
    assert_eq!(
        rules(r#"<div class="gap-0 gap-x-0 space-y-0">"#),
        v(&[
            "1:gap-below-floor",
            "1:gap-below-floor",
            "1:gap-below-floor"
        ])
    );
    assert!(rules(r#"<div class="gap-2 gap-x-2 space-y-[7px] gap-[7px]">"#).is_empty());
    assert_eq!(
        rules(r#"<div class="gap-1 gap-[4px]">"#),
        v(&["1:gap-below-floor", "1:gap-below-floor"])
    );
}

#[test]
fn flags_negative_margins_of_every_side_not_zero_or_positive_ones() {
    assert_eq!(
        rules(r#"<div class="-mt-2 -mx-1 -m-px -space-x-2">"#),
        v(&[
            "1:negative-margin",
            "1:negative-margin",
            "1:negative-margin",
            "1:negative-margin"
        ])
    );
    assert!(rules(r#"<div class="mt-2 mx-auto -m-0 ml-[-0px]">"#).is_empty());
}

#[test]
fn flags_leading_under_1_5_and_accepts_normal_relaxed_loose_and_fixed_lengths() {
    assert_eq!(
        rules(r#"<p class="leading-none leading-tight leading-snug leading-[1.2]">"#),
        v(&["1:leading-below-floor"; 4])
    );
    assert!(
        rules(
            r#"<p class="leading-normal leading-relaxed leading-loose leading-6 leading-[1.6]">"#
        )
        .is_empty()
    );
}

#[test]
fn honours_the_configured_spacing_unit() {
    let at = |unit: f64| {
        scan_text(
            r#"<div class="px-2">"#,
            &ScanOptions {
                spacing_unit_px: Some(unit),
                ..Default::default()
            },
        )
        .findings
    };
    assert!(at(7.0).is_empty());
    assert_eq!(at(4.0).len(), 1);
}

#[test]
fn does_not_flag_classes_that_merely_look_similar() {
    assert!(rules(r#"<div class="top-0 mb-0 inset-0 gap-y-auto spacing-0 grid-gap">"#).is_empty());
}

#[test]
fn reports_line_and_column() {
    let f = first("line one\n  <div class=\"flex p-0\">");
    assert_eq!(
        (f.line, f.column, f.found.as_str(), f.minimum, f.value),
        (2, 20, "p-0", Some(14.0), Some(0.0))
    );
}

// ---- scanText: CSS and style props ----

#[test]
fn evaluates_padding_shorthands_by_side() {
    assert_eq!(rules(".a { padding: 0; }"), v(&["1:padding-zero"]));
    assert!(rules(".a { padding: 4px 14px; }").is_empty());
    assert_eq!(
        rules(".a { padding: 14px 4px; }"),
        v(&["1:padding-below-inset"])
    );
    assert_eq!(
        rules(".a { padding: 14px 14px 14px 4px; }"),
        v(&["1:padding-below-inset"])
    );
    assert_eq!(
        rules(".a { padding-left: 0.25rem; }"),
        v(&["1:padding-below-inset"])
    );
    assert!(rules(".a { padding-inline: 1rem; padding-block: 2px; }").is_empty());
    assert_eq!(rules(".a { padding-top: 0; }"), v(&["1:padding-zero"]));
}

#[test]
fn skips_values_it_cannot_evaluate() {
    assert!(
        rules(".a { padding: var(--x); gap: calc(1px + 2px); line-height: var(--l); padding: 2% }")
            .is_empty()
    );
    assert!(rules(
        ".a { padding: var(--inset-floor); gap: var(--gap-floor); line-height: var(--leading-floor); }"
    )
    .is_empty());
}

#[test]
fn flags_gap_row_gap_and_column_gap_under_the_7px_floor_of_a_line_with_no_layer() {
    assert_eq!(
        rules(".a { gap: 0; row-gap: 2px; column-gap: 0.125rem }"),
        v(&["1:gap-below-floor"; 3])
    );
    assert!(rules(".a { gap: 7px 8px; gap: 0.5rem; gap: normal }").is_empty());
}

#[test]
fn flags_negative_margins_in_any_shorthand_slot() {
    assert_eq!(rules(".a { margin: 0 -4px; }"), v(&["1:negative-margin"]));
    assert_eq!(
        rules(".a { margin-left: -0.5rem; }"),
        v(&["1:negative-margin"])
    );
    assert!(rules(".a { margin: 0 auto; margin-top: 1rem }").is_empty());
}

#[test]
fn flags_unitless_and_percent_line_heights_under_1_5() {
    assert_eq!(
        rules("a { line-height: 1; } b { line-height: 1.2 } c { line-height: 120% }"),
        v(&["1:leading-below-floor"; 3])
    );
    assert!(rules(
        "a { line-height: 1.5; } b { line-height: 1.7 } c { line-height: normal } d { line-height: 20px }"
    )
    .is_empty());
}

#[test]
fn reads_jsx_style_props_where_a_bare_number_is_pixels() {
    assert_eq!(
        rules("<div style={{ paddingLeft: 4, gap: 2, lineHeight: 1.2, marginTop: -3 }} />"),
        v(&[
            "1:padding-below-inset",
            "1:gap-below-floor",
            "1:leading-below-floor",
            "1:negative-margin"
        ])
    );
    assert!(rules("<div style={{ padding: '14px', gap: 8, lineHeight: 1.5 }} />").is_empty());
}

#[test]
fn reads_apply_lines() {
    assert_eq!(
        rules(".card { @apply flex gap-1 p-0; }"),
        v(&["1:gap-below-floor", "1:padding-zero"])
    );
}

// ---- scanText: comments and exceptions ----

#[test]
fn ignores_code_in_comments() {
    assert!(rules("// never p-0\n/* gap-0 */\n/*\n padding: 0\n*/\n<!-- px-1 -->").is_empty());
}

#[test]
fn does_not_read_a_glob_as_a_comment() {
    assert_eq!(
        rules(r#"const g = "src/**/*.css"; const c = "p-0""#),
        v(&["1:padding-zero"])
    );
}

#[test]
fn excuses_a_finding_with_a_reason_on_the_same_or_the_previous_line() {
    let same = scan(r#"<div class="p-0"> {/* mzizi-spacing-exempt: image fills its frame */}"#);
    assert!(same.findings.is_empty());
    assert_eq!(same.exempted.len(), 1);
    assert_eq!(same.exempted[0].finding.rule, "padding-zero");
    assert_eq!(same.exempted[0].reason, "image fills its frame");

    let prev = scan(
        "/* mzizi-spacing-exempt: avatar stack overlaps by design */\n.a { margin-left: -8px; }",
    );
    assert!(prev.findings.is_empty());
    assert_eq!(prev.exempted.len(), 1);

    let farther = scan("// mzizi-spacing-exempt: why\n\n<div class=\"p-0\">");
    assert_eq!(farther.findings.len(), 1);
}

#[test]
fn rejects_an_exception_with_no_reason_and_keeps_the_finding() {
    assert_eq!(
        rules(r#"<div class="p-0"> <!-- mzizi-spacing-exempt -->"#),
        v(&["1:padding-zero", "1:exemption-without-reason"])
    );
    assert_eq!(
        rules("// mzizi-spacing-exempt:\n<div class=\"p-0\">"),
        v(&["1:exemption-without-reason", "2:padding-zero"])
    );
    assert_eq!(
        rules("// mzizi-spacing-exempt: ok\n<div class=\"p-0\">").len(),
        2
    );
}

#[test]
fn excuses_a_whole_file_from_the_first_20_lines_only() {
    let head = "/* mzizi-spacing-exempt-file: vendored reset */\n.a { padding: 0 }\n";
    assert!(scan(head).findings.is_empty());
    assert_eq!(scan(head).exempted.len(), 1);
    let late = format!(
        "{}/* mzizi-spacing-exempt-file: too late */\n.a {{ padding: 0 }}\n",
        "\n".repeat(25)
    );
    assert_eq!(scan(&late).findings.len(), 1);
}

#[test]
fn notes_the_legacy_opt_out_without_excusing_anything() {
    let r = scan(r#"<html data-mzizi-sizes="legacy"><div class="p-0">"#);
    assert_eq!(r.legacy, vec![1]);
    assert_eq!(r.findings.len(), 1);
}

// ---- scanText: review fixes ----

#[test]
fn a_marker_with_no_reason_excuses_nothing_even_with_code_after_the_comment() {
    assert_eq!(
        rules("/* mzizi-spacing-exempt */ .a{padding:0}"),
        v(&["1:exemption-without-reason", "1:padding-zero"])
    );
    assert_eq!(
        rules(r#"<div class="p-0"> {/* mzizi-spacing-exempt */} <span>"#),
        v(&["1:padding-zero", "1:exemption-without-reason"])
    );
}

#[test]
fn a_trailing_exception_does_not_carry_down_to_the_next_code_line() {
    let r = scan("<div class=\"p-0\"> {/* mzizi-spacing-exempt: image */}\n<div class=\"gap-0\">");
    assert_eq!(r.exempted.len(), 1);
    assert_eq!(
        r.findings.iter().map(|f| f.line).collect::<Vec<_>>(),
        vec![2]
    );
}

#[test]
fn a_standalone_comment_on_the_line_above_still_excuses_the_next_line() {
    assert!(
        scan("// mzizi-spacing-exempt: overlap by design\n<div class=\"-mx-2\">")
            .findings
            .is_empty()
    );
}

#[test]
fn a_marker_in_a_string_or_in_prose_excuses_nothing() {
    assert_eq!(
        rules(r#"const hint = "mzizi-spacing-exempt: because"; <div class="p-0">"#),
        v(&["1:padding-zero"])
    );
}

#[test]
fn reads_past_a_protocol_relative_url_in_css_when_slashes_are_not_a_comment() {
    let text = "a{background:url(//cdn.example/x.png);padding:0}";
    let no_lc = ScanOptions {
        line_comments: Some(false),
        ..Default::default()
    };
    assert_eq!(scan_text(text, &no_lc).findings.len(), 1);
    assert_eq!(scan(text).findings.len(), 1);
    assert_eq!(
        scan_text(r#"<a href=//cdn/x class="p-0">"#, &no_lc)
            .findings
            .len(),
        1
    );
}

#[test]
fn sees_a_block_comment_right_after_a_value() {
    assert_eq!(
        rules(".a { padding:0/* reset */; }"),
        v(&["1:padding-zero"])
    );
    assert!(rules(".a { padding:14px/* padding: 0 */; }").is_empty());
}

#[test]
fn does_not_flag_line_height_0() {
    assert!(rules(".icon { line-height: 0 }").is_empty());
}

#[test]
fn does_not_let_a_src_glob_blank_the_rest_of_the_file() {
    assert_eq!(
        rules("include: [\"src/**\"],\nconst c = \"p-0\""),
        v(&["2:padding-zero"])
    );
}

#[test]
fn accepts_the_html_legal_closer_and_long_trailing_whitespace() {
    assert!(
        rules(r#"<div class="p-0"> <!-- mzizi-spacing-exempt: image fills its frame --!>"#)
            .is_empty()
    );
    let long = format!(
        r#"<div class="p-0"> <!-- mzizi-spacing-exempt: image fills its frame{}-->"#,
        "\t".repeat(50000)
    );
    assert!(rules(&long).is_empty());
}

// ---- label gap and sibling items ----

#[test]
fn holds_a_label_to_field_gap_to_7px_by_default_and_5px_dense() {
    assert_eq!(
        rules(r#"<div className="label-field gap-[6px] --space-label-gap">"#),
        v(&["1:gap-below-floor"])
    );
    assert_eq!(
        rules(r#"<div style={{ gap: "var(--space-label-gap)" }} className="gap-[7px]">"#),
        v(&["1:gap-below-floor"])
    );
    assert_eq!(
        rules(r#"<div className="dense --space-label-gap gap-[4px]">"#),
        v(&["1:gap-below-floor"])
    );
    assert!(rules(r#"<div className="dense --space-label-gap gap-[5px]">"#).is_empty());
}

#[test]
fn holds_sibling_items_to_the_card_gap_14px_dense_7px() {
    assert_eq!(
        rules(r#"<ul className="gap-3">"#),
        v(&["1:gap-below-floor"])
    );
    assert!(rules(r#"<ul className="gap-[14px]">"#).is_empty());
    assert_eq!(
        rules(r#"<div role="toolbar" className="gap-[10px]">"#),
        v(&["1:gap-below-floor"])
    );
    assert!(rules(r#"<li className="dense gap-[7px]">"#).is_empty());
    assert_eq!(
        rules(r#"<li className="dense gap-[6px]">"#),
        v(&["1:gap-below-floor"])
    );
}

// ---- the review of 2026-11-19 ----

#[test]
fn holds_an_unplaced_line_to_14px_padding_and_a_7px_gap_at_any_density() {
    assert_eq!(
        rules(r#"<div class="px-[8px]">"#),
        v(&["1:padding-below-inset"])
    );
    assert_eq!(
        rules(r#"<div class="dense p-[13px]">"#),
        v(&["1:padding-below-inset"])
    );
    assert!(rules(r#"<div class="pl-[14px] dense">"#).is_empty());
    assert_eq!(
        rules(r#"<div class="gap-[4px] dense">"#),
        v(&["1:gap-below-floor"])
    );
}

#[test]
fn holds_a_layer_with_no_gap_token_chips_to_the_unplaced_7px_gap() {
    let f = first(r#"<Chip className="gap-[4px]">"#);
    assert_eq!((f.rule, f.minimum), ("gap-below-floor", Some(7.0)));
    assert!(scan(r#"<Chip className="gap-[8px]">"#).findings.is_empty());
}

#[test]
fn holds_the_gap_between_adjacent_controls_to_9px_dense_too() {
    assert_eq!(
        rules(r#"<button className="gap-2">"#),
        v(&["1:gap-below-floor"])
    );
    assert!(rules(r#"<button className="gap-[9px]">"#).is_empty());
    assert_eq!(
        rules(r#"<button className="gap-1.5 dense">"#),
        v(&["1:gap-below-floor"])
    );
    assert!(
        scan(r#"<button className="dense gap-[9px]">"#)
            .findings
            .is_empty()
    );
}

#[test]
fn holds_the_icon_gap_and_the_label_gap_only_where_the_line_names_their_token() {
    assert_eq!(
        rules(r#"<div className="--space-control-gap gap-[7px]">"#),
        v(&["1:gap-below-floor"])
    );
    assert!(rules(r#"<div className="dense --space-control-gap gap-[7px]">"#).is_empty());
    assert_eq!(
        rules(r#"<div className="--space-label-gap gap-[5px]">"#),
        v(&["1:gap-below-floor"])
    );
    assert!(rules(r#"<div className="dense --space-label-gap gap-[5px]">"#).is_empty());
}

#[test]
fn checks_a_controls_hit_region_at_dense_density_too() {
    assert_eq!(
        rule_names(r#"<button className="dense h-5">"#),
        vec!["hit-floor"]
    );
    let w = scan(r#"<button className="dense h-6">"#).warnings;
    assert_eq!(
        w.iter().map(|f| f.rule).collect::<Vec<_>>(),
        vec!["hit-floor-touch"]
    );
    assert_eq!(
        rule_names(".t { --control-floor: 24px; }"),
        vec!["hit-floor"]
    );
    assert!(scan(".t { --control-floor: 28px; }").findings.is_empty());
}

#[test]
fn places_a_line_by_a_class_name_not_by_a_word_inside_another_class() {
    let a = first(r#"<div className="my-card gap-[4px]">"#);
    assert_eq!(
        (a.rule, a.layer, a.minimum),
        ("gap-below-floor", None, Some(7.0))
    );
    assert!(
        scan(r#"<div className="card gap-[8px]">"#)
            .findings
            .is_empty()
    );
    assert_eq!(
        first(r#"<div className="card gap-[4px]">"#).layer,
        Some(Layer::Cards)
    );
}

#[test]
fn reads_the_number_rule() {
    let ns = [0.0, 1.0, 2.0, 3.0, 5.0, 7.0];
    assert_eq!(
        ns.map(is_mzizi_number),
        [true, false, true, true, true, true]
    );
    assert_eq!(ns.map(next_mzizi_number), [0.0, 2.0, 2.0, 3.0, 5.0, 7.0]);
    assert!(!is_mzizi_number(4.0));
    assert_eq!(next_mzizi_number(4.0), 5.0);
    assert!(!is_mzizi_number(-3.0));
    assert!(!is_mzizi_number(2.5));
}

/// The time bound of the TypeScript tests (200ms). Debug builds of the regex crate are slow, so
/// the bound is checked at 10x here and the real number is printed; see docs/porting/floor.md.
fn under_budget(text: &str) {
    let t0 = Instant::now();
    scan_text(text, &ScanOptions::default());
    let ms = t0.elapsed().as_millis();
    let budget = if cfg!(debug_assertions) { 2000 } else { 200 };
    assert!(ms < budget, "{} chars took {ms}ms", text.len());
}

#[test]
fn parses_a_50000_character_token_in_well_under_200ms() {
    let long = "1".repeat(50000);
    for text in [
        format!(".a {{ padding: {long}px }}"),
        format!(".a {{ line-height: {long}% }}"),
        format!(
            ".a {{ line-height: {}.{} }}",
            "1".repeat(25000),
            "1".repeat(25000)
        ),
        format!("<div className=\"p-[{long}]\">"),
        format!("<div className=\"{}\">", "p-".repeat(25000)),
        format!(".a {{ padding: {}x !important   }}", " ".repeat(50000)),
        format!(".a {{ gap: {}x }}", "!important ".repeat(5000)),
    ] {
        under_budget(&text);
    }
}

// ---- shorthand sides, attachment and linear parsing ----

#[test]
fn errors_on_a_horizontal_floor_breach_even_when_the_vertical_side_warns_first() {
    assert_eq!(
        rule_names(".a { padding: 4px }"),
        vec!["padding-below-inset"]
    );
    assert_eq!(
        rule_names(".a { padding: 10px 4px }"),
        vec!["padding-below-inset"]
    );
    assert_eq!(rule_names(".a { padding: 20px 0 }"), vec!["padding-zero"]);
    assert_eq!(rule_names(".a { padding: 4px 0 }"), vec!["padding-zero"]);
    assert_eq!(rule_names(".a { gap: 8px 4px }"), vec!["gap-below-floor"]);
    assert_eq!(rule_names(".a { gap: 10px 2px }"), vec!["gap-below-floor"]);
    assert_eq!(
        rule_names(".a { padding: 8px }"),
        vec!["padding-below-inset"]
    );
    assert_eq!(
        rule_names("<Box style={{ padding: 4 }} />"),
        vec!["padding-below-inset"]
    );
}

#[test]
fn holds_a_cards_padding_to_the_cards_floor_of_21() {
    let f = scan(".card { padding: 16px }").findings;
    assert_eq!(f.len(), 1);
    assert_eq!(
        (f[0].rule, f[0].layer, f[0].minimum),
        ("padding-below-inset", Some(Layer::Cards), Some(21.0))
    );
}

#[test]
fn falls_back_to_a_number_rule_warning_only_when_no_side_is_under_its_floor() {
    let r = scan(".a { padding: 16px }");
    assert!(r.findings.is_empty());
    assert_eq!(
        r.warnings.iter().map(|f| f.rule).collect::<Vec<_>>(),
        vec!["number-rule"]
    );
}

#[test]
fn attaches_a_named_gap_token_only_to_the_gap_in_its_own_segment() {
    assert_eq!(
        rules(r#"<div className="--space-control-gap"><span className="gap-[7px]">"#),
        v(&["1:gap-below-floor"])
    );
    assert_eq!(
        rules(r#"<div className="--space-control-gap gap-[7px]">"#),
        v(&["1:gap-below-floor"])
    );
    assert!(rules(r#"<div className="dense --space-control-gap gap-[7px]">"#).is_empty());
    assert_eq!(
        rules(".x { --space-control-gap: 7px; gap: 7px }"),
        v(&["1:gap-below-floor"])
    );
}

#[test]
fn does_not_place_a_line_from_a_property_path_such_as_styles_badge_or_theme_page() {
    let a = first(r#"<div style={styles.badge} className="x gap-[4px]">"#);
    assert_eq!((a.layer, a.minimum), (None, Some(7.0)));
    assert_eq!(
        first("const v = theme.page; const g = 'gap-[4px]'").layer,
        None
    );
}

#[test]
fn reads_the_dense_token_boundary_exactly() {
    assert!(!is_dense_line("font-condensed"));
    assert!(!is_dense_line(r#"<p className="condensed">"#));
    assert!(is_dense_line(r#"<table className="table-dense">"#));
    assert!(is_dense_line(r#"<div className="dense">"#));
    assert!(is_dense_line("--space-control-dense"));
}

#[test]
fn reads_the_nit_values_a_control_gap_under_9px_is_held_at_9() {
    assert_eq!(
        rules(r#"<button className="gap-2">"#),
        v(&["1:gap-below-floor"])
    );
}

// ---- hostile inputs stay linear ----

#[test]
fn hostile_inputs_stay_linear() {
    let shapes: Vec<(&str, String)> = vec![
        ("className={ repeated", "<div className={ ".repeat(11000)),
        ("class=\" repeated", "<div class=\"".repeat(10000)),
        ("--space- repeated", "--space-".repeat(12500)),
        ("dense- repeated", "-dense ".repeat(12500)),
        ("gap: repeated", "gap: ".repeat(16000)),
        ("padding: repeated", "padding: 4px ".repeat(7000)),
        ("card selectors", ".card ".repeat(12500)),
        (
            "long class value",
            format!("<div className=\"{}\">", "gap-1 ".repeat(9000)),
        ),
        (
            "long token",
            format!(".a {{ padding: {}px }}", "1".repeat(100000)),
        ),
        (
            "long important",
            format!(".a {{ gap: {}x }}", "!important ".repeat(9000)),
        ),
    ];
    for (name, text) in shapes {
        assert!(text.len() > 50000, "{name}");
        under_budget(&text);
    }
}
