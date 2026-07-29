# Generated man pages

The `*.1` files in this directory are generated from snip's clap command tree.
Do not edit them by hand.

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
