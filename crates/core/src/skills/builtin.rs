//! Skills that ship inside the app (ADR 0016 addendum).
//!
//! They are embedded in the binary and copied into the Naurix skill store once. Seeding **never links**
//! a skill into any agent folder: the user still chooses that. Removing a built-in skill is respected
//! (a setting records that it was seeded, so it is not added back).

use super::SkillError;
use std::path::Path;

pub struct BuiltinSkill {
    pub slug: &'static str,
    pub skill_md: &'static str,
}

pub const BUILTIN: &[BuiltinSkill] = &[BuiltinSkill {
    slug: "natural-writing",
    skill_md: include_str!("../../resources/skills/natural-writing/SKILL.md"),
}];

pub fn find(slug: &str) -> Option<&'static BuiltinSkill> {
    BUILTIN.iter().find(|b| b.slug == slug)
}

/// Source string recorded for a built-in skill.
pub fn source_for(slug: &str) -> String {
    format!("builtin:{slug}")
}

/// Write the embedded skill as `<root>/<slug>/SKILL.md`.
pub fn materialize(b: &BuiltinSkill, root: &Path) -> Result<(), SkillError> {
    let dir = root.join(b.slug);
    std::fs::create_dir_all(&dir).map_err(|e| SkillError::Io(e.to_string()))?;
    std::fs::write(dir.join("SKILL.md"), b.skill_md).map_err(|e| SkillError::Io(e.to_string()))
}
