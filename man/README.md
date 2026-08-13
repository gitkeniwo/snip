# Generated man pages

The `*.1`, `*.5`, and `*.7` files in this directory are generated. Do not edit
them or `src/commands/man_pages.rs` by hand. The generated index embeds every
page in the `snip` binary.

`examples/generate-man.rs` contains the page manifest. Each substantive page
combines clap-generated command skeletons with prose from `man/parts/*.md`.
Short compatibility pages are real generated stubs that point readers to their
grouped page; they do not use `.so`, so they also work from release archives and
`snip man show`.

The 15 command pages in section 1 are `snip`, `snip-tui`, `snip-query`,
`snip-create`, `snip-edit`, `snip-trash`, `snip-init`, `snip-doctor`,
`snip-git`, `snip-gist`, `snip-config`, `snip-theme`, `snip-keys`, `snip-man`,
and `snip-completion`. Section 5 contains `sniplib`, `snip-config`, `snip-keys`,
and `snip-theme`; section 7 contains `snip-agents`.

The committed pages always describe the complete default command tree,
including `snip tui`. A `--no-default-features` binary intentionally embeds the
same stable page set. During generation and `--check`, every non-hidden clap
command must be covered by a page manifest entry, every alias target must exist,
and section-1 prose must contain at least two examples and a `SEE ALSO` section.

Update the pages:

```bash
cargo run --locked --all-features --example generate-man
```

Check that committed pages are current:

```bash
cargo run --locked --all-features --example generate-man -- --check
```

Preview the root page or a command page:

```bash
cargo run --locked --all-features --example generate-man -- --preview
cargo run --locked --all-features --example generate-man -- --preview snip-create
cargo run --locked --all-features --example generate-man -- --preview sniplib.5
```

Prose files use `%%SECTION NAME` markers. Plain lines form paragraphs and lines
indented by four spaces form literal blocks. The generator escapes roff control
characters; prose must not contain raw roff requests.
