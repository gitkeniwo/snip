//! The skill tells agents which snip version it describes, so a copy fetched
//! for one binary is not trusted by another. The release bump must update it.

#[test]
fn skill_names_the_crate_version() {
    let skill = include_str!("../skills/snip/SKILL.md");
    let expected = format!("Written for **snip {}**.", env!("CARGO_PKG_VERSION"));
    assert!(
        skill.contains(&expected),
        "skills/snip/SKILL.md must contain {expected:?}; update it with the version bump"
    );
}
