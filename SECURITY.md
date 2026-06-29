# Security Policy — rust-samp

## Reporting a vulnerability

Found a security vulnerability? Please do not open a public issue.

**Contact:** open a private [Security Advisory](https://github.com/NullSablex/rust-samp/security/advisories/new)
on GitHub, or e-mail the maintainer directly.

Expected response within **7 business days**.

---

## Scope

This policy covers the source of the `rust-samp` workspace crates
(`rust-samp`, `rust-samp-sdk`, `rust-samp-codegen`) in the repository
`NullSablex/rust-samp`. It is a toolkit for building SA-MP / open.mp
server plugins; vulnerabilities in a downstream plugin built **with** the
SDK should be reported to that plugin's maintainers.

---

## Dependencies

The runtime dependency surface is intentionally small (see each crate's
`Cargo.toml`); optional features (`encoding`, `compression`) pull in
additional crates only when explicitly enabled. Dependency advisories are
tracked through GitHub's Dependabot and `cargo`'s advisory database.

---

## Supported versions

Only the most recent release published to [crates.io](https://crates.io/crates/rust-samp)
and the [Releases](https://github.com/NullSablex/rust-samp/releases) page
receives security fixes.
