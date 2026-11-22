//! The Mzizi spacing layers and hit-area floors, as the floor check enforces them.
//!
//! Port of `spacing-floor.ts` (owner rulings, 2026-11-19). The TypeScript doc comments are
//! the specification; the values are copied unchanged.
#![allow(missing_docs)]

use serde_json::{Map, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Layout,
    Containers,
    Cards,
    Controls,
    Chips,
    Pills,
}

impl Layer {
    /// Layers in the order the TypeScript report lists them.
    pub const ALL: [Layer; 6] = [
        Layer::Layout,
        Layer::Containers,
        Layer::Cards,
        Layer::Controls,
        Layer::Chips,
        Layer::Pills,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Layout => "layout",
            Layer::Containers => "containers",
            Layer::Cards => "cards",
            Layer::Controls => "controls",
            Layer::Chips => "chips",
            Layer::Pills => "pills",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Padding,
    Gap,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Padding => "padding",
            Kind::Gap => "gap",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SpacingToken {
    pub token: &'static str,
    pub layer: Layer,
    pub kind: Kind,
    pub px: f64,
    pub dense: f64,
}

const fn t(token: &'static str, layer: Layer, kind: Kind, px: f64, dense: f64) -> SpacingToken {
    SpacingToken {
        token,
        layer,
        kind,
        px,
        dense,
    }
}

/// Every per-layer spacing token.
pub const SPACING_LAYERS: [SpacingToken; 16] = [
    t("--space-page-x", Layer::Layout, Kind::Padding, 17.0, 17.0),
    t(
        "--space-page-x-wide",
        Layer::Layout,
        Kind::Padding,
        28.0,
        21.0,
    ),
    t("--space-section", Layer::Layout, Kind::Gap, 49.0, 28.0),
    t(
        "--space-container",
        Layer::Containers,
        Kind::Padding,
        28.0,
        21.0,
    ),
    t(
        "--space-container-gap",
        Layer::Containers,
        Kind::Gap,
        21.0,
        14.0,
    ),
    t("--space-card", Layer::Cards, Kind::Padding, 21.0, 14.0),
    t("--space-card-gap", Layer::Cards, Kind::Gap, 14.0, 7.0),
    t("--space-card-stack", Layer::Cards, Kind::Gap, 7.0, 7.0),
    t(
        "--space-control-x",
        Layer::Controls,
        Kind::Padding,
        17.0,
        12.0,
    ),
    t(
        "--space-field-x",
        Layer::Controls,
        Kind::Padding,
        17.0,
        12.0,
    ),
    t("--space-control-gap", Layer::Controls, Kind::Gap, 9.0, 7.0),
    t(
        "--space-control-between",
        Layer::Controls,
        Kind::Gap,
        9.0,
        9.0,
    ),
    t("--space-label-gap", Layer::Controls, Kind::Gap, 7.0, 5.0),
    t("--space-chip-x", Layer::Chips, Kind::Padding, 12.0, 9.0),
    t("--space-pill-x", Layer::Pills, Kind::Padding, 9.0, 5.0),
    t("--space-pill-gap", Layer::Pills, Kind::Gap, 5.0, 3.0),
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Floor {
    pub default: f64,
    pub dense: f64,
}

const NAMED_ONLY_GAP_TOKENS: [&str; 2] = ["--space-control-gap", "--space-label-gap"];

/// The floor of a layer and kind: the smallest default and the smallest dense value of its tokens.
pub fn floor_of(layer: Layer, kind: Kind) -> Option<Floor> {
    let tokens: Vec<&SpacingToken> = SPACING_LAYERS
        .iter()
        .filter(|t| {
            t.layer == layer
                && t.kind == kind
                && !(kind == Kind::Gap && NAMED_ONLY_GAP_TOKENS.contains(&t.token))
        })
        .collect();
    if tokens.is_empty() {
        return None;
    }
    Some(Floor {
        default: tokens.iter().map(|t| t.px).fold(f64::INFINITY, f64::min),
        dense: tokens.iter().map(|t| t.dense).fold(f64::INFINITY, f64::min),
    })
}

/// Body line height (`--leading-floor`, WCAG 1.4.12).
pub const LEADING_BODY: f64 = 1.5;
pub const UNPLACED_PADDING_PX: f64 = 14.0;
pub const UNPLACED_GAP_PX: f64 = 7.0;
pub const HIT_TOUCH_PX: f64 = 45.0;
pub const HIT_DESKTOP_PX: f64 = 24.0;
/// The drawn minimum of a control at desktop density (`--control-floor`).
pub const CONTROL_DRAWN_FLOOR_PX: f64 = 28.0;
pub const NUMBER_RULE_LIMIT_PX: f64 = 10000.0;
/// The marker that excuses a line: `mzizi-spacing-exempt: <reason>`. A reason is required.
pub const EXEMPT_MARKER: &str = "mzizi-spacing-exempt";

fn token_floor(token: &str, dense: bool) -> Option<f64> {
    SPACING_LAYERS
        .iter()
        .find(|x| x.token == token)
        .map(|t| if dense { t.dense } else { t.px })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    Default,
    Dense,
    Unplaced,
}

/// The minimum a value must reach, and the tier it is held to.
pub fn required_px(
    kind: Kind,
    layer: Option<Layer>,
    dense: bool,
    sibling: bool,
    gap_token: Option<&str>,
) -> (f64, Tier) {
    let tier = if dense { Tier::Dense } else { Tier::Default };
    if sibling && kind == Kind::Gap {
        if let Some(card) = token_floor("--space-card-gap", dense) {
            return (card, tier);
        }
    }
    if kind == Kind::Gap && layer == Some(Layer::Controls) {
        if let Some(named) = gap_token.and_then(|g| token_floor(g, dense)) {
            return (named, tier);
        }
    }
    match layer.and_then(|l| floor_of(l, kind)) {
        None => (
            match kind {
                Kind::Padding => UNPLACED_PADDING_PX,
                Kind::Gap => UNPLACED_GAP_PX,
            },
            Tier::Unplaced,
        ),
        Some(f) => (if dense { f.dense } else { f.default }, tier),
    }
}

fn is_prime(n: f64) -> bool {
    if n < 2.0 {
        return false;
    }
    let mut d = 2.0;
    while d * d <= n {
        if n % d == 0.0 {
            return false;
        }
        d += 1.0;
    }
    true
}

/// The number rule: a whole number that is 0, a multiple of 3, a multiple of 7, or a prime.
pub fn is_mzizi_number(px: f64) -> bool {
    if !px.is_finite() || px.fract() != 0.0 || px < 0.0 {
        return false;
    }
    px == 0.0 || px % 3.0 == 0.0 || px % 7.0 == 0.0 || is_prime(px)
}

/// The smallest Mzizi number at or above `px` (the rule rounds up, never down).
pub fn next_mzizi_number(px: f64) -> f64 {
    if px > NUMBER_RULE_LIMIT_PX {
        return px;
    }
    let mut n = px.ceil().max(0.0);
    while !is_mzizi_number(n) {
        n += 1.0;
    }
    n
}

/// JavaScript's `Number.prototype.toString` for the values this tool prints.
pub fn js_num(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if n == 0.0 {
        return "0".into();
    }
    let a = n.abs();
    if !(1e-6..1e21).contains(&a) {
        let s = format!("{n:e}");
        return match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        };
    }
    format!("{n}")
}

/// A JSON number the way `JSON.stringify` writes it (`null` when not finite).
pub fn num(n: f64) -> Value {
    serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number)
}

fn floor_json(f: Option<Floor>, out: &mut Map<String, Value>, key: &str) {
    if let Some(f) = f {
        let mut m = Map::new();
        m.insert("default".into(), num(f.default));
        m.insert("dense".into(), num(f.dense));
        out.insert(key.into(), Value::Object(m));
    }
}

/// `SPACING_FLOOR` as the report carries it, with the TypeScript key order.
pub fn spacing_floor_json() -> Value {
    let mut layers = Map::new();
    for layer in Layer::ALL {
        let mut m = Map::new();
        floor_json(floor_of(layer, Kind::Padding), &mut m, "padding");
        floor_json(floor_of(layer, Kind::Gap), &mut m, "gap");
        layers.insert(layer.as_str().into(), Value::Object(m));
    }
    let mut unplaced = Map::new();
    unplaced.insert("paddingPx".into(), num(UNPLACED_PADDING_PX));
    unplaced.insert("gapPx".into(), num(UNPLACED_GAP_PX));
    let mut root = Map::new();
    root.insert("leadingBody".into(), num(LEADING_BODY));
    root.insert("layers".into(), Value::Object(layers));
    root.insert("unplaced".into(), Value::Object(unplaced));
    // Deprecated keys kept for readers of the 0.11 shape.
    root.insert("insetPx".into(), num(14.0));
    root.insert("controlGapPx".into(), num(9.0));
    root.insert("labelGapPx".into(), num(7.0));
    root.insert("itemGapPx".into(), num(14.0));
    Value::Object(root)
}
