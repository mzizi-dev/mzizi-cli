//! The Mzizi skill corpus (port of `loadMziziSkills`).
//!
//! The TypeScript CLI read the `@nyuchi/mzizi-skills` npm package. The binary embeds a
//! snapshot instead (`skills/`, written by `scripts/refresh-skills.sh`), so `plan` stays
//! offline and needs no Node. The hard-coded two-skill fallback is kept for an empty corpus.

use super::skills_data::EMBEDDED;

/// One skill: its name and full SKILL.md text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// Skill name.
    pub name: String,
    /// SKILL.md, frontmatter included.
    pub body: String,
}

fn fallback() -> Vec<Skill> {
    vec![
        Skill {
            name: "mzizi-component-install".into(),
            body: include_str!("fallback-mzizi-component-install.txt").into(),
        },
        Skill {
            name: "mzizi-tokens".into(),
            body: include_str!("fallback-mzizi-tokens.txt").into(),
        },
    ]
}

/// Skills from `(name, body)` entries, or the fallback when there are none.
pub fn load_from(entries: &[(&str, &str)]) -> Vec<Skill> {
    if entries.is_empty() {
        return fallback();
    }
    entries
        .iter()
        .map(|(n, b)| Skill {
            name: (*n).into(),
            body: (*b).into(),
        })
        .collect()
}

/// The embedded corpus.
pub fn load_mzizi_skills() -> Vec<Skill> {
    load_from(EMBEDDED)
}
