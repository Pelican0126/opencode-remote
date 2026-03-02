# Security Policy

## Supported Versions

This project is maintained on a best-effort basis. Please report vulnerabilities against the latest `master` branch.

## Our Security Principles

- We do not collect or embed your API keys in this repository.
- Real secrets must stay in your local `.env` only.
- The repository should only contain safe templates such as `.env.example`.

## How to Report a Vulnerability

Please do **not** open a public issue for sensitive vulnerabilities.

Send a private report with:

- Impact summary
- Reproduction steps
- Affected files/commands
- Suggested fix (optional)

Recommended private channels:

- GitHub Security Advisories (preferred)
- Private email/contact channel configured by repository owner

We will acknowledge the report as soon as possible and coordinate disclosure after a fix is available.

## Do Not Post Sensitive Data in Issues

When creating issues/PRs, never include:

- API keys, tokens, passwords
- `.env` content
- Private base URLs or internal hostnames
- Customer data or production logs containing secrets

If secret leakage is suspected, rotate the secret immediately.
