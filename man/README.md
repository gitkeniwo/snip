# Generated man pages

The `*.1` files in this directory are generated from snip's clap command tree.
Do not edit them or `src/commands/man_pages.rs` by hand. The generated index
embeds every page in the `snip` binary.

The committed pages always describe the complete default command tree,
including `snip tui`. A `--no-default-features` binary intentionally embeds the
same stable page set.

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
```
