# Contributing

Thanks for your interest in improving Phone-TV. This document covers
the basics: how to get the project running, how to send a change, and
what we expect from contributions.

## Development setup

Requirements:

- Rust stable (`rustup toolchain install stable`)
- Linux system deps (Debian/Ubuntu):
  ```
  sudo apt-get install -y libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev \
    libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libasound2-dev
  ```
- `adb` available on `PATH`

Build & run:

```
cargo run --bin phone-tv
```

Run lint & format checks (same as CI):

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build
```

## Sending a change

1. Open an issue first for non-trivial work so we can align on scope.
2. Fork, create a branch named `feat/...`, `fix/...`, or `docs/...`.
3. Keep commits focused. Conventional Commits style is preferred
   (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`).
4. Make sure `cargo fmt`, `cargo clippy`, and `cargo build` pass.
5. Open a pull request against `main`. Fill in the PR template.

## Code style

- Match the existing style; rustfmt is the source of truth.
- No `unwrap()` / `expect()` on user-reachable paths — return `Result`
  and surface a clear error.
- Keep modules cohesive — UI in `src/ui/`, ADB plumbing in `src/adb.rs`,
  security checks in `src/security/`, etc.
- No comments that explain *what* the code does; only explain *why*
  when it isn't obvious.

## Reporting bugs

Use the issue templates. Include:

- Phone-TV version and OS
- Device brand / Android version (when relevant)
- Reproduction steps and the actual vs. expected behaviour

## Security issues

Do not open a public issue. Follow [SECURITY.md](./SECURITY.md).
