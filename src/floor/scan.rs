//! The spacing-floor scan: padding, gaps, margins and line heights under the Bundu floor in
//! one file's text. Port of `check-spacing.ts`: pure and offline, a string in, findings out.
//!
//! Regexes with lookaround in the TypeScript (`(?<![\w-])`, `(?![\w-])`) are ported by hand
//! (Rust's `regex` has none); each such spot says how. Columns are UTF-16 offsets, as in JS.
#![allow(missing_docs)]

use std::sync::OnceLock;

use regex::{Regex, RegexBuilder};
use serde_json::{Map, Value};

use super::tokens::{
    CONTROL_DRAWN_FLOOR_PX, EXEMPT_MARKER, HIT_DESKTOP_PX, HIT_TOUCH_PX, Kind, LEADING_BODY, Layer,
    NUMBER_RULE_LIMIT_PX, Tier, is_mzizi_number, js_num, next_mzizi_number, num, required_px,
};

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| {
                RegexBuilder::new($pat)
                    .size_limit(256 << 20)
                    .build()
                    .expect("static regex")
            })
        }
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// One finding. `line` and `column` are 1-based.
#[derive(Clone, Debug)]
pub struct Finding {
    pub rule: &'static str,
    pub severity: Severity,
    pub layer: Option<Layer>,
    pub line: usize,
    pub column: usize,
    pub found: String,
    pub message: String,
    pub minimum: Option<f64>,
    pub value: Option<f64>,
    /// `exemption-without-reason` lists line and column before found/message (key order of the TS object).
    line_first: bool,
}

impl Finding {
    /// The JSON object, in the TypeScript key order. `file` leads when given.
    pub fn to_json(&self, file: Option<&str>, reason: Option<&str>) -> Value {
        let mut m = Map::new();
        if let Some(f) = file {
            m.insert("file".into(), f.into());
        }
        m.insert("rule".into(), self.rule.into());
        m.insert("severity".into(), self.severity.as_str().into());
        if self.line_first {
            m.insert("line".into(), self.line.into());
            m.insert("column".into(), self.column.into());
        }
        if let Some(l) = self.layer {
            m.insert("layer".into(), l.as_str().into());
        }
        m.insert("found".into(), self.found.clone().into());
        if let Some(v) = self.minimum {
            m.insert("minimum".into(), num(v));
        }
        if let Some(v) = self.value {
            m.insert("value".into(), num(v));
        }
        m.insert("message".into(), self.message.clone().into());
        if !self.line_first {
            m.insert("line".into(), self.line.into());
            m.insert("column".into(), self.column.into());
        }
        if let Some(r) = reason {
            m.insert("reason".into(), r.into());
        }
        Value::Object(m)
    }
}

#[derive(Clone, Debug)]
pub struct Exempted {
    pub finding: Finding,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ScanOptions {
    /// Whether `//` starts a comment. `None` is the TypeScript default, true.
    pub line_comments: Option<bool>,
    /// Pixels per Tailwind spacing step. Default 4.
    pub spacing_unit_px: Option<f64>,
    /// Pixels per rem and em. Default 16.
    pub rem_px: Option<f64>,
}

struct Opts {
    spacing_unit_px: f64,
    rem_px: f64,
    line_comments: bool,
}

#[derive(Debug, Default)]
pub struct ScanResult {
    /// Errors: each one fails the check.
    pub findings: Vec<Finding>,
    pub warnings: Vec<Finding>,
    pub exempted: Vec<Exempted>,
    /// Lines (1-based) carrying the legacy sizes opt-out.
    pub legacy: Vec<usize>,
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn is_word_dash(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn spaces(n: usize) -> String {
    " ".repeat(n)
}

/// Does the text before a `/*` end where a block comment may open after a value?
/// Port of `/(?:\d(?:px|rem|em)?|[%)};{,\s])$/`.
fn css_tail_ok(prefix: &str) -> bool {
    let Some(last) = prefix.chars().next_back() else {
        return false;
    };
    if last.is_whitespace() || "%)};{,".contains(last) || last.is_ascii_digit() {
        return true;
    }
    ["px", "rem", "em"].iter().any(|u| {
        prefix
            .strip_suffix(u)
            .is_some_and(|p| p.chars().next_back().is_some_and(|c| c.is_ascii_digit()))
    })
}

enum Block {
    Css,
    Html,
}

/// Blank out comments (keeping every column), and return the lines without them.
fn strip_comments(text: &str, line_comments: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut block: Option<Block> = None;
    for raw in text.split('\n') {
        let mut line = String::new();
        let mut i = 0;
        while i < raw.len() {
            if let Some(b) = &block {
                let closer = if matches!(b, Block::Css) { "*/" } else { "-->" };
                match raw[i..].find(closer) {
                    None => {
                        line.push_str(&spaces(utf16_len(&raw[i..])));
                        i = raw.len();
                    }
                    Some(at) => {
                        let end = i + at + closer.len();
                        line.push_str(&spaces(utf16_len(&raw[i..end])));
                        i = end;
                        block = None;
                    }
                }
                continue;
            }
            let prev = if i == 0 {
                ' '
            } else {
                raw[..i].chars().next_back().unwrap_or(' ')
            };
            // A comment opens at a token boundary: not inside a glob, a URL, a path or an attribute value.
            let starts_here = !(prev.is_ascii_alphabetic() || "_*/:.\"'`(=]-".contains(prev));
            if raw[i..].starts_with("<!--") {
                block = Some(Block::Html);
                line.push_str("    ");
                i += 4;
            } else if raw[i..].starts_with("/*") && (starts_here || css_tail_ok(&raw[..i])) {
                block = Some(Block::Css);
                line.push_str("  ");
                i += 2;
            } else if line_comments && starts_here && raw[i..].starts_with("//") {
                line.push_str(&spaces(utf16_len(&raw[i..])));
                i = raw.len();
            } else {
                let c = raw[i..].chars().next().unwrap_or(' ');
                line.push(c);
                i += c.len_utf8();
            }
        }
        out.push(line);
    }
    out
}

/// The reason after an exception marker; `None` when the marker is absent, `Some("")` when
/// there is no reason worth the name.
fn exemption_reason(line: &str, marker: &str) -> Option<String> {
    let at = line.find(marker)?;
    let mut rest = &line[at + marker.len()..];
    if rest.trim_start().starts_with(':')
        && let Some(c) = rest.find(':')
    {
        rest = &rest[c + 1..];
    }
    for closer in ["*/", "--!>", "-->"] {
        if let Some(i) = rest.find(closer) {
            rest = &rest[..i];
        }
    }
    let rest = rest
        .trim_end_matches(|c: char| "\"'`);".contains(c) || c.is_whitespace())
        .trim();
    re!(three_letters, r"[A-Za-z]{3,}");
    Some(if three_letters().is_match(rest) {
        rest.to_string()
    } else {
        String::new()
    })
}

fn strip_important(s: &str) -> String {
    let t = s.trim_end();
    let n = "!important".len();
    if t.len() >= n
        && t.is_char_boundary(t.len() - n)
        && t[t.len() - n..].eq_ignore_ascii_case("!important")
    {
        t[..t.len() - n].trim_end().to_string()
    } else {
        s.to_string()
    }
}

fn length_to_px(raw: &str, opts: &Opts, unitless_is_px: bool) -> Option<f64> {
    re!(len, r"^(-?(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+))(px|rem|em)?$");
    let v = strip_important(raw.trim());
    let c = len().captures(&v)?;
    let n: f64 = c.get(1)?.as_str().parse().ok()?;
    match c.get(2).map(|m| m.as_str()) {
        Some("px") => Some(n),
        Some(_) => Some(n * opts.rem_px),
        None => (n == 0.0 || unitless_is_px).then_some(n),
    }
}

/// Split a CSS value on whitespace at paren depth 0.
fn split_value(v: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in v.trim().chars() {
        if ch == '(' {
            depth += 1;
        }
        if ch == ')' {
            depth -= 1;
        }
        if ch.is_whitespace() && depth == 0 {
            if !cur.is_empty() {
                parts.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

/// The token after the last `:` that is outside square brackets and parens (the variant prefix).
fn after_variants(token: &str) -> &str {
    let mut depth = 0i32;
    let mut last = 0;
    for (i, ch) in token.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ':' if depth == 0 => last = i + 1,
            _ => {}
        }
    }
    &token[last..]
}

fn tailwind_value(raw: &str, opts: &Opts) -> Option<f64> {
    re!(plain, r"^[0-9]+(\.[0-9]+)?$");
    if raw == "px" {
        return Some(1.0);
    }
    if plain().is_match(raw) {
        return raw.parse::<f64>().ok().map(|n| n * opts.spacing_unit_px);
    }
    let inner = raw.strip_prefix('[')?.strip_suffix(']')?;
    if inner.is_empty() {
        return None;
    }
    length_to_px(&inner.replace('_', " "), opts, false)
}

struct Hit {
    f: Finding,
    /// Byte offset in the (comment-blanked) line; converted to a UTF-16 column at the end.
    index: usize,
}

#[allow(clippy::too_many_arguments)]
fn hit(
    rule: &'static str,
    severity: Severity,
    layer: Option<Layer>,
    found: &str,
    index: usize,
    minimum: f64,
    value: Option<f64>,
    message: String,
) -> Hit {
    Hit {
        f: Finding {
            rule,
            severity,
            layer,
            line: 0,
            column: 0,
            found: found.to_string(),
            message,
            minimum: Some(minimum),
            value,
            line_first: false,
        },
        index,
    }
}

re!(token_control, r"--space-(?:control|field|label)");
re!(token_chip, r"--space-chip");
re!(token_pill, r"--space-pill");
re!(token_card, r"--space-card");
re!(token_container, r"--space-container");
re!(token_layout, r"--space-(?:page|section)");
re!(
    tag_controls,
    r#"<(?:button|input|select|textarea|Button|Input|Select|Textarea)(?-u:\b)|role=["']button["']"#
);
re!(tag_chip, r"<Chip(?-u:\b)");
re!(tag_pill, r"<(?:Badge|Pill)(?-u:\b)");
re!(tag_card, r"<(?:Card|article)(?-u:\b)");
re!(tag_container, r"<(?:dialog|Dialog|Sheet|Panel)(?-u:\b)");
re!(tag_layout, r"<(?:main|section|header|footer|nav)(?-u:\b)");
re!(class_attr, r#"class(?:Name)?=\{?\s*(["'`])"#);

const CLASS_VALUE_WINDOW: usize = 2048;

fn take_utf16(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    let mut used = 0;
    for (i, c) in s.char_indices() {
        used += c.len_utf16();
        if used > n {
            return &s[..i];
        }
    }
    s
}

/// The text of each class attribute value on a line. Linear: each value is bounded by the window.
fn class_values(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for c in class_attr().captures_iter(line) {
        let (Some(m), Some(q)) = (c.get(0), c.get(1)) else {
            continue;
        };
        let window = take_utf16(&line[m.end()..], CLASS_VALUE_WINDOW);
        out.push(match window.find(q.as_str()) {
            Some(end) => &window[..end],
            None => window,
        });
    }
    out
}

/// True when the line names one of these classes, as a token of a class value or as a `.name`
/// selector. The TS used `(?<![\w-])(?:alt)(?![\w-])`: for names with no name a prefix of
/// another (true of every list here) that is exactly "a maximal run of `[\w-]` equals a name".
fn class_named(line: &str, values: &[&str], names: &[&str]) -> bool {
    if values
        .iter()
        .any(|v| v.split(|c| !is_word_dash(c)).any(|t| names.contains(&t)))
    {
        return true;
    }
    // `(?:^|[\s{},;>+~])\.(?:alt)(?![\w-])`
    for (i, c) in line.char_indices() {
        if c != '.' {
            continue;
        }
        let prev_ok = line[..i]
            .chars()
            .next_back()
            .is_none_or(|p| p.is_whitespace() || "{},;>+~".contains(p));
        if !prev_ok {
            continue;
        }
        let rest = &line[i + 1..];
        if names.iter().any(|n| {
            rest.strip_prefix(n)
                .is_some_and(|after| after.chars().next().is_none_or(|a| !is_word_dash(a)))
        }) {
            return true;
        }
    }
    false
}

/// The layer a line places itself in: a layer token first, then a tag or a class name.
pub fn layer_of_line(line: &str) -> Option<Layer> {
    let values = class_values(line);
    let tokens: [(&Regex, Layer); 6] = [
        (token_control(), Layer::Controls),
        (token_chip(), Layer::Chips),
        (token_pill(), Layer::Pills),
        (token_card(), Layer::Cards),
        (token_container(), Layer::Containers),
        (token_layout(), Layer::Layout),
    ];
    for (re, layer) in tokens {
        if re.is_match(line) {
            return Some(layer);
        }
    }
    let rules: [(&Regex, &[&str], Layer); 6] = [
        (tag_controls(), &["btn", "button", "input"], Layer::Controls),
        (tag_chip(), &["chip"], Layer::Chips),
        (tag_pill(), &["pill", "badge"], Layer::Pills),
        (tag_card(), &["card"], Layer::Cards),
        (
            tag_container(),
            &["dialog", "sheet", "panel"],
            Layer::Containers,
        ),
        (tag_layout(), &["page", "layout"], Layer::Layout),
    ];
    for (tag, names, layer) in rules {
        if tag.is_match(line) || class_named(line, &values, names) {
            return Some(layer);
        }
    }
    None
}

/// A line is dense when one of its tokens is `dense` or ends in `-dense`, or when it states
/// the dense density. `condensed` is not a dense token.
pub fn is_dense_line(line: &str) -> bool {
    re!(density, r#"density=["']dense["']"#);
    if density().is_match(line) {
        return true;
    }
    line.split(|c: char| c.is_whitespace() || "\"'`{}()[];,:=<>".contains(c))
        .any(|t| t == "dense" || (utf16_len(t) > 6 && t.ends_with("-dense")))
}

struct Segment {
    start: usize,
    gap_token: Option<&'static str>,
}

/// `--space-(?:control-gap|label-gap)(?![\w-])`, the first occurrence in `s`.
fn named_gap(s: &str) -> Option<&'static str> {
    let mut from = 0;
    while let Some(at) = s[from..].find("--space-") {
        let rest = &s[from + at + "--space-".len()..];
        for (suffix, full) in [
            ("control-gap", "--space-control-gap"),
            ("label-gap", "--space-label-gap"),
        ] {
            if let Some(after) = rest.strip_prefix(suffix)
                && after.chars().next().is_none_or(|c| !is_word_dash(c))
            {
                return Some(full);
            }
        }
        from += at + 1;
    }
    None
}

fn segment_line(line: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in line.char_indices() {
        if "\"'`;{}<>,".contains(c) {
            out.push(Segment {
                start,
                gap_token: named_gap(&line[start..i]),
            });
            start = i + c.len_utf8();
        }
    }
    out.push(Segment {
        start,
        gap_token: named_gap(&line[start..]),
    });
    out
}

fn gap_token_at(segs: &[Segment], index: usize) -> Option<&'static str> {
    let mut lo = 0usize;
    let mut hi = segs.len() as isize - 1;
    while (lo as isize) < hi {
        let mid = ((lo as isize + hi + 1) >> 1) as usize;
        if segs[mid].start <= index {
            lo = mid;
        } else {
            hi = mid as isize - 1;
        }
    }
    segs.get(lo).and_then(|s| s.gap_token)
}

struct Ctx {
    layer: Option<Layer>,
    dense: bool,
    sibling: bool,
    segments: Vec<Segment>,
}

fn is_sibling_element(line: &str) -> bool {
    re!(
        sib,
        r#"<(?:li|ul|ol)(?-u:\b)|role=["'](?:list|listitem|toolbar|grid|gridcell)["']"#
    );
    sib().is_match(line)
}

fn floor_message(kind: Kind, ctx: &Ctx, min: f64, tier: Tier) -> String {
    let where_ = match ctx.layer {
        Some(l) => format!("the {} layer", l.as_str()),
        None => "a line with no layer (or a layer with no such token)".to_string(),
    };
    let what = match tier {
        Tier::Dense => "dense floor (the absolute floor)",
        Tier::Unplaced => "unplaced floor",
        Tier::Default => "default floor",
    };
    format!(
        "the {} {} for {} is {}px",
        kind.as_str(),
        what,
        where_,
        js_num(min)
    )
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Horizontal,
    Vertical,
    All,
}

fn padding_hit(side: Side, px: f64, found: &str, index: usize, ctx: &Ctx) -> Option<Hit> {
    let (min, tier) = required_px(Kind::Padding, ctx.layer, ctx.dense, false, None);
    if px == 0.0 {
        return Some(hit(
            "padding-zero",
            Severity::Error,
            ctx.layer,
            found,
            index,
            min,
            Some(0.0),
            format!(
                "`{found}` removes the inset; {}",
                floor_message(Kind::Padding, ctx, min, tier)
            ),
        ));
    }
    if side != Side::Vertical && px < min {
        return Some(hit(
            "padding-below-inset",
            Severity::Error,
            ctx.layer,
            found,
            index,
            min,
            Some(px),
            format!(
                "`{found}` is {}px; {}",
                js_num(px),
                floor_message(Kind::Padding, ctx, min, tier)
            ),
        ));
    }
    None
}

/// Every floor error of a shorthand's sides; else the first number-rule warning.
fn settle(floors: Vec<Option<Hit>>, warns: Vec<Option<Hit>>) -> Vec<Hit> {
    let mut seen: Vec<String> = Vec::new();
    let mut errs = Vec::new();
    for h in floors.into_iter().flatten() {
        if seen.contains(&h.f.message) {
            continue;
        }
        seen.push(h.f.message.clone());
        errs.push(h);
    }
    if !errs.is_empty() {
        return errs;
    }
    warns.into_iter().flatten().take(1).collect()
}

fn gap_hit(px: f64, found: &str, index: usize, ctx: &Ctx, gap_token: Option<&str>) -> Option<Hit> {
    let (min, tier) = required_px(Kind::Gap, ctx.layer, ctx.dense, ctx.sibling, gap_token);
    if px >= min {
        return None;
    }
    Some(hit(
        "gap-below-floor",
        Severity::Error,
        ctx.layer,
        found,
        index,
        min,
        Some(px),
        format!(
            "`{found}` is {}px; {}",
            js_num(px),
            floor_message(Kind::Gap, ctx, min, tier)
        ),
    ))
}

fn margin_hit(found: &str, index: usize, value: Option<f64>) -> Hit {
    hit(
        "negative-margin",
        Severity::Error,
        None,
        found,
        index,
        0.0,
        value,
        format!(
            "`{found}` is a negative margin; it can pull text under its container edge and overlap hit regions"
        ),
    )
}

fn leading_hit(ratio: f64, found: &str, index: usize) -> Option<Hit> {
    if ratio >= LEADING_BODY {
        return None;
    }
    Some(hit(
        "leading-below-floor",
        Severity::Error,
        None,
        found,
        index,
        LEADING_BODY,
        Some(ratio),
        format!(
            "`{found}` is a line height of {}; body leading is at least {} (--leading-floor)",
            js_num(ratio),
            js_num(LEADING_BODY)
        ),
    ))
}

/// A declared height on a control: under 24px is an error, under 45px a touch warning.
fn hit_hit(px: f64, found: &str, index: usize, ctx: &Ctx) -> Option<Hit> {
    if ctx.layer != Some(Layer::Controls) {
        return None;
    }
    if px < HIT_DESKTOP_PX {
        return Some(hit(
            "hit-floor",
            Severity::Error,
            ctx.layer,
            found,
            index,
            HIT_DESKTOP_PX,
            Some(px),
            format!(
                "`{found}` is {}px; the hit region of a control is at least {}px on a desktop ({}px on touch)",
                js_num(px),
                js_num(HIT_DESKTOP_PX),
                js_num(HIT_TOUCH_PX)
            ),
        ));
    }
    if px < HIT_TOUCH_PX {
        return Some(hit(
            "hit-floor-touch",
            Severity::Warning,
            ctx.layer,
            found,
            index,
            HIT_TOUCH_PX,
            Some(px),
            format!(
                "`{found}` is {}px; on touch the hit region is {}px (--target-floor). Keep the {}px region with hit-area or a token such as h-control, or mark a dense control",
                js_num(px),
                js_num(HIT_TOUCH_PX),
                js_num(HIT_TOUCH_PX)
            ),
        ));
    }
    None
}

/// The number rule, as a warning: a value that is not a Mzizi number, with the one that replaces it.
fn number_hit(px: f64, found: &str, index: usize, ctx: &Ctx) -> Option<Hit> {
    if px > NUMBER_RULE_LIMIT_PX || is_mzizi_number(px) {
        return None;
    }
    let next = next_mzizi_number(px);
    Some(hit(
        "number-rule",
        Severity::Warning,
        ctx.layer,
        found,
        index,
        next,
        Some(px),
        format!(
            "`{found}` is {}px, not a multiple of 3, a multiple of 7 or a prime; the nearest Mzizi number at or above it is {}px",
            js_num(px),
            js_num(next)
        ),
    ))
}

fn leading_name(name: &str) -> Option<f64> {
    Some(match name {
        "none" => 1.0,
        "tight" => 1.25,
        "snug" => 1.375,
        "normal" => 1.5,
        "relaxed" => 1.625,
        "loose" => 2.0,
        _ => return None,
    })
}

fn tailwind_hits(line: &str, opts: &Opts, ctx: &Ctx) -> Vec<Hit> {
    re!(tokens, r#"(?:[^\s"'`{}<>,;=()]|\[[^\]\s"'`]*\])+"#);
    re!(sized, r"^(-?)(h|min-h|size|w|min-w)-(.+)$");
    re!(
        parsed,
        r"^(-?)(p[xytrblse]?|gap-x|gap-y|gap|space-x|space-y|m[xytrblse]?|leading)-(.+)$"
    );
    re!(arb_leading, r"^\[((?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+))\]$");
    let mut hits = Vec::new();
    for m in tokens().find_iter(line) {
        let token = m.as_str();
        let base = after_variants(token).trim_matches('!');
        let index = m.start();
        if let Some(c) = sized().captures(base) {
            let neg = !c[1].is_empty();
            let px = if neg {
                None
            } else {
                tailwind_value(&c[3], opts)
            };
            let Some(px) = px else { continue };
            let h = hit_hit(px, base, index, ctx).or_else(|| {
                if px > 0.0 {
                    number_hit(px, base, index, ctx)
                } else {
                    None
                }
            });
            hits.extend(h);
            continue;
        }
        let Some(c) = parsed().captures(base) else {
            continue;
        };
        let neg = !c[1].is_empty();
        let kind = &c[2];
        let raw_value = &c[3];
        let found = base;

        if kind == "leading" {
            if neg {
                continue;
            }
            let ratio = match arb_leading().captures(raw_value) {
                Some(a) => a[1].parse::<f64>().ok(),
                None => leading_name(raw_value),
            };
            if let Some(r) = ratio {
                hits.extend(leading_hit(r, found, index));
            }
            continue;
        }
        let Some(px) = tailwind_value(raw_value, opts) else {
            continue;
        };
        if kind.starts_with('m') {
            if neg && px != 0.0 {
                hits.push(margin_hit(found, index, Some(-px)));
            } else if !neg {
                hits.extend(number_hit(px, found, index, ctx));
            }
        } else if kind.starts_with("space-") || kind.starts_with("gap") {
            if neg && kind.starts_with("space-") {
                if px != 0.0 {
                    hits.push(margin_hit(found, index, Some(-px)));
                }
            } else if !neg {
                let h = gap_hit(px, found, index, ctx, gap_token_at(&ctx.segments, index))
                    .or_else(|| number_hit(px, found, index, ctx));
                hits.extend(h);
            }
        } else if !neg {
            let side = if kind == "p" {
                Side::All
            } else if matches!(kind, "px" | "pl" | "pr" | "ps" | "pe") {
                Side::Horizontal
            } else {
                Side::Vertical
            };
            let h = padding_hit(side, px, found, index, ctx)
                .or_else(|| number_hit(px, found, index, ctx));
            hits.extend(h);
        }
    }
    hits
}

re!(
    css_decl,
    r"(padding(?:-(?:top|right|bottom|left|inline|block)(?:-(?:start|end))?)?|padding(?:Top|Right|Bottom|Left|Inline|Block)(?:Start|End)?|gap|row-gap|column-gap|grid-gap|grid-row-gap|grid-column-gap|rowGap|columnGap|margin(?:-(?:top|right|bottom|left|inline|block)(?:-(?:start|end))?)?|margin(?:Top|Right|Bottom|Left|Inline|Block)(?:Start|End)?|line-height|lineHeight|height|min-height|minHeight|--target-floor|--control-floor)\s*:\s*([^;{},\n(]{0,512}(?:\([^)\n]{0,512}\)[^;{},\n(]{0,512})*)"
);

fn kebab(prop: &str) -> String {
    let mut s = String::new();
    for c in prop.chars() {
        if c.is_ascii_uppercase() {
            s.push('-');
            s.push(c.to_ascii_lowercase());
        } else {
            s.push(c);
        }
    }
    s
}

fn declaration_hits(line: &str, opts: &Opts, ctx: &Ctx) -> Vec<Hit> {
    re!(pct, r"^(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)%$");
    re!(unitless, r"^(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)$");
    re!(vertical_name, r"-(top|bottom|block)");
    let mut hits = Vec::new();
    let mut pos = 0;
    while pos <= line.len() {
        let Some(caps) = css_decl().captures_at(line, pos) else {
            break;
        };
        let (Some(whole), Some(p), Some(v)) = (caps.get(0), caps.get(1), caps.get(2)) else {
            break;
        };
        // `(?<![\w-])`: a declaration name must not continue a longer word. On a miss, resume one
        // character later, as the JS engine's next start position would.
        if line[..whole.start()]
            .chars()
            .next_back()
            .is_some_and(is_word_dash)
        {
            pos = whole.start()
                + line[whole.start()..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8);
            continue;
        }
        pos = whole.end().max(whole.start() + 1);
        let prop = p.as_str();
        let name = kebab(prop);
        let index = whole.start();
        // JSX separates props with commas; CSS never has a comma at depth 0 in these values.
        let mut value = strip_important(v.as_str());
        let mut depth = 0i32;
        let mut end = value.len();
        for (i, ch) in value.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    end = i;
                    break;
                }
                _ => {}
            }
        }
        value.truncate(end);
        let mut value: &str = value.trim();
        // `.replace(/^["'`]|["'`\]]+$/g, "")`
        if value.starts_with(['"', '\'', '`']) {
            value = &value[1..];
        }
        value = value.trim_end_matches(['"', '\'', '`', ']']).trim();
        let found = format!("{prop}: {value}");
        let parts = split_value(value);
        let px: Vec<Option<f64>> = parts.iter().map(|p| length_to_px(p, opts, true)).collect();

        if name == "--target-floor" || name == "--control-floor" {
            let min = if name == "--control-floor" {
                CONTROL_DRAWN_FLOOR_PX
            } else {
                HIT_DESKTOP_PX
            };
            if let Some(&Some(v)) = px.first()
                && v < min
            {
                hits.push(hit(
                    "hit-floor",
                    Severity::Error,
                    None,
                    &found,
                    index,
                    min,
                    Some(v),
                    format!(
                        "`{found}` is {}px; {name} may not be under {}px",
                        js_num(v),
                        js_num(min)
                    ),
                ));
            }
        } else if name == "height" || name == "min-height" {
            if let Some(&Some(v)) = px.first()
                && v > 0.0
            {
                hits.extend(
                    hit_hit(v, &found, index, ctx).or_else(|| number_hit(v, &found, index, ctx)),
                );
            }
        } else if name == "line-height" {
            let ratio = if unitless().is_match(value) {
                value.parse::<f64>().ok()
            } else if pct().is_match(value) {
                value
                    .trim_end_matches('%')
                    .parse::<f64>()
                    .ok()
                    .map(|n| n / 100.0)
            } else {
                None
            };
            // `line-height: 0` collapses an icon or wrapper box; it is not body leading.
            if let Some(r) = ratio
                && r != 0.0
            {
                hits.extend(leading_hit(r, &found, index));
            }
        } else if name.starts_with("margin") {
            if let Some(neg) = px.iter().flatten().find(|v| **v < 0.0) {
                hits.push(margin_hit(&found, index, Some(*neg)));
            }
        } else if name.contains("gap") {
            let token = gap_token_at(&ctx.segments, index);
            let mut floors = Vec::new();
            let mut warns = Vec::new();
            for v in px.iter().flatten() {
                floors.push(gap_hit(*v, &found, index, ctx, token));
                warns.push(number_hit(*v, &found, index, ctx));
            }
            hits.extend(settle(floors, warns));
        } else if name.starts_with("padding") {
            let logical = if vertical_name().is_match(&name) {
                Some(Side::Vertical)
            } else if name == "padding" {
                None
            } else {
                Some(Side::Horizontal)
            };
            let idx: [usize; 4] = match parts.len() {
                1 => [0, 0, 0, 0],
                2 => [0, 1, 0, 1],
                3 => [0, 1, 2, 1],
                _ => [0, 1, 2, 3],
            };
            let at = |i: usize| px.get(i).copied().flatten();
            let sides: Vec<(Side, Option<f64>)> = match logical {
                Some(l) => px.iter().map(|v| (l, *v)).collect(),
                None => vec![
                    (Side::Vertical, at(idx[0])),
                    (Side::Horizontal, at(idx[1])),
                    (Side::Vertical, at(idx[2])),
                    (Side::Horizontal, at(idx[3])),
                ],
            };
            let mut floors = Vec::new();
            let mut warns = Vec::new();
            for (side, v) in sides {
                let Some(v) = v else { continue };
                floors.push(padding_hit(side, v, &found, index, ctx));
                warns.push(number_hit(v, &found, index, ctx));
            }
            hits.extend(settle(floors, warns));
        }
    }
    hits
}

fn no_reason(line: usize, text: &str, marker: &str) -> Finding {
    let at = text.find(marker).map_or(0, |b| utf16_len(&text[..b]));
    Finding {
        rule: "exemption-without-reason",
        severity: Severity::Error,
        layer: None,
        line,
        column: at + 1,
        found: marker.to_string(),
        message: format!(
            "`{marker}` needs a reason after it (`{marker}: <why this is below the floor>`); without one it excuses nothing"
        ),
        minimum: None,
        value: None,
        line_first: true,
    }
}

fn slice16(s: &str, a: usize, b: usize) -> String {
    let u: Vec<u16> = s.encode_utf16().collect();
    let b = b.min(u.len());
    let a = a.min(b);
    String::from_utf16_lossy(&u[a..b])
}

/// Scan one file's text.
pub fn scan_text(text: &str, options: &ScanOptions) -> ScanResult {
    re!(legacy_re, r#"data-mzizi-sizes\s*=\s*["']?legacy"#);
    let opts = Opts {
        spacing_unit_px: options.spacing_unit_px.unwrap_or(4.0),
        rem_px: options.rem_px.unwrap_or(16.0),
        line_comments: options.line_comments.unwrap_or(true),
    };
    let raw: Vec<&str> = text.split('\n').collect();
    let code = strip_comments(text, opts.line_comments);
    let mut findings: Vec<Finding> = Vec::new();
    let mut exempted: Vec<Exempted> = Vec::new();
    let mut legacy: Vec<usize> = Vec::new();

    let mut line_reason: std::collections::HashMap<usize, String> =
        std::collections::HashMap::new();
    let mut file_reason: Option<String> = None;
    // A marker counts only inside a comment: where strip_comments blanked the raw text.
    let in_comment = |i: usize| -> bool {
        let Some(at) = raw[i].find(EXEMPT_MARKER) else {
            return false;
        };
        let at16 = utf16_len(&raw[i][..at]);
        slice16(&code[i], at16, at16 + EXEMPT_MARKER.len())
            .trim()
            .is_empty()
    };
    let file_marker_name = format!("{EXEMPT_MARKER}-file");
    for (i, line) in raw.iter().enumerate() {
        if legacy_re().is_match(line) {
            legacy.push(i + 1);
        }
        if !in_comment(i) {
            continue;
        }
        if let Some(fm) = exemption_reason(line, &file_marker_name) {
            // Only the first 20 lines can excuse a whole file; further down it excuses nothing.
            if i >= 20 {
                continue;
            }
            if !fm.is_empty() {
                file_reason.get_or_insert(fm);
            } else {
                findings.push(no_reason(i + 1, line, &file_marker_name));
            }
            continue;
        }
        let Some(reason) = exemption_reason(line, EXEMPT_MARKER) else {
            continue;
        };
        if !reason.is_empty() {
            line_reason.insert(i + 1, reason);
        } else {
            findings.push(no_reason(i + 1, line, EXEMPT_MARKER));
        }
    }

    for (i, line) in code.iter().enumerate() {
        let line_no = i + 1;
        let ctx = Ctx {
            layer: layer_of_line(line),
            dense: is_dense_line(line),
            sibling: is_sibling_element(line),
            segments: segment_line(line),
        };
        let mut hits = tailwind_hits(line, &opts, &ctx);
        hits.extend(declaration_hits(line, &opts, &ctx));
        if hits.is_empty() {
            continue;
        }
        // The line above excuses this one only when it is a comment on its own line.
        let above_blank = match i.checked_sub(1).map(|p| &code[p]) {
            None => false,
            Some(p) => p.chars().all(|c| c.is_whitespace() || c == '{' || c == '}'),
        };
        let above = if above_blank {
            line_reason.get(&(line_no - 1))
        } else {
            None
        };
        let reason = line_reason.get(&line_no).or(above).or(file_reason.as_ref());
        for h in hits {
            let mut f = h.f;
            f.line = line_no;
            f.column = utf16_len(&line[..h.index]) + 1;
            match reason {
                Some(r) => exempted.push(Exempted {
                    finding: f,
                    reason: r.clone(),
                }),
                None => findings.push(f),
            }
        }
    }

    findings.sort_by(|a, b| a.line.cmp(&b.line).then(a.column.cmp(&b.column)));
    let (errors, warnings): (Vec<_>, Vec<_>) = findings
        .into_iter()
        .partition(|f| f.severity == Severity::Error);
    ScanResult {
        findings: errors,
        warnings,
        exempted,
        legacy,
    }
}
