# Agent skills

Skills that teach an AI agent to operate this project. They are plain
Markdown with YAML frontmatter — the format Claude Code, the Claude Agent SDK,
and Claude.ai all load — so any agent runtime that reads skills can use them,
and any agent without skill support can simply read `SKILL.md` as documentation.

| Skill | Teaches |
|---|---|
| [`snip`](snip/SKILL.md) | Using the `snip` CLI: vocabulary, selectors, JSON contract, safe concurrent editing, and the on-disk format. |

## Installing

Copy or symlink the skill directory into the agent's skills folder. Symlinking
keeps it current as the CLI evolves:

```bash
# Claude Code / Claude Desktop, available in every project
mkdir -p ~/.claude/skills
ln -s "$PWD/skills/snip" ~/.claude/skills/snip
```

```bash
# Scoped to one project instead
mkdir -p /path/to/project/.claude/skills
ln -s "$PWD/skills/snip" /path/to/project/.claude/skills/snip
```

For the Claude Agent SDK, point your skills directory at `skills/`. For agents
that take a system prompt rather than skill files, paste `snip/SKILL.md` and
attach `snip/references/*.md` as needed.

The skill assumes `snip` is on `PATH` and that a library is reachable — via
`--library`, `$SNIP_LIBRARY`, an ancestor `snip.toml`, or the `default_library`
config key. Install the binary with `cargo install --path .` from the repository
root.

## Authoring notes

Keep `SKILL.md` under ~500 lines and push exhaustive detail into `references/`,
which the agent loads only when it needs it. Everything documented should be
verified against the actual CLI — the reference files quote real JSON payloads
and real error messages, and stale ones are worse than none.
