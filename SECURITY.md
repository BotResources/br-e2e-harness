# Security policy

## Reporting a vulnerability

If you believe you have found a security issue in any fixture in this
repository, email **security@botresources.ai** with:

- A description of the issue and its impact.
- Reproduction steps or a proof of concept.
- Affected crate and version (or commit SHA).

**Do not** open a public issue, pull request, or Discussion. Public disclosure
before a fix is shipped is not authorized.

We aim to acknowledge reports within 5 business days. We do not currently run a
public bug-bounty program — reports are accepted on a goodwill basis, with no
guarantee of reward.

## Scope

- **In scope:** the source code in this repository.
- **Out of scope:** the BotResources platform, hosted services, third-party
  dependencies (report upstream), and any deployment-specific configuration.

**Note on threat model:** these are test fixtures, designed to run only on
isolated, throwaway test networks. "The admin API signs tokens without
authentication" is the documented purpose, not a vulnerability. A report is in
scope when a fixture misbehaves *within* that model (e.g. a malformed input
crashes the process, or the fixture leaks key material it should not).

## Supported versions

We support the latest tagged version of each crate (`<crate>-vX.Y.Z`) and its
matching container image. Older tags receive no fixes; upgrade to the latest.
