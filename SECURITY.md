# Security Policy

## Supported Versions

The latest minor release on `main` is supported. Older versions do not
receive security fixes.

| Version | Supported          |
| ------- | ------------------ |
| latest  | :white_check_mark: |
| < latest | :x:               |

## Reporting a Vulnerability

**Please do not open a public issue for security vulnerabilities.**

Report security issues privately through GitHub's
[private vulnerability reporting](https://github.com/micferna/PHONE-TV-ANDROID/security/advisories/new).

You should receive an acknowledgement within 72 hours. We will work
with you to confirm the issue and publish a fix and advisory once
resolved.

## Scope

In scope:

- Vulnerabilities in the application code (Rust crates under `src/`)
- Supply chain issues in dependencies (`Cargo.toml` / `Cargo.lock`)
- Issues in CI/CD workflows (`.github/workflows/`)

Out of scope:

- Issues in third-party services (Android, ADB, the LLM provider)
- Findings that require physical access to an unlocked device
- Best-practice suggestions without a concrete impact

## Disclosure

We follow coordinated disclosure: a fix is published before the
advisory is made public, and credit is given to reporters who wish it.
