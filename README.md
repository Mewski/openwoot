# openwoot

Open keyboard firmware for Wooting hall-effect keyboards, written in Rust, with its own
configuration protocol and a host command-line tool. Supported boards: Wooting 80HE (GD32F425)
and Wooting 60HE+ (GD32F303).

The branch `main` holds the earlier C firmware; this branch is the Rust rewrite.

## State

**Scaffold.** The workspace builds a firmware image for every board, the host tests pass and
`cargo xtask check` enforces the rules of the tree. The firmware does nothing yet: no key
scanning, no USB, no lighting. Do not flash these images.

## Build

Requirements: [rustup](https://rustup.rs) (the pinned toolchain in `rust-toolchain.toml` is
installed on first use), and for `cargo xtask check` also a nightly `rustfmt`, `taplo` and
`cargo-sort`.

```sh
cargo test --workspace                 # host tests
cargo xtask boards                     # list the boards
cargo xtask build --all                # build/<board>/openwoot-<board>.{elf,hex,bin}
cargo xtask flash wooting-80he --dry-run   # check an image the way flashing would
cargo xtask check                      # every rule of the tree, formatting, lints, images
```

[docs/building.md](docs/building.md) explains the build, [docs/architecture.md](docs/architecture.md)
the crates, [docs/adding-a-board.md](docs/adding-a-board.md) what a new board needs.

## Licence

MIT, see [LICENSE](LICENSE).
