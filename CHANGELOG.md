# Changelog

All notable changes to indiBudget are recorded here.

This project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html), and
the format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.1.0] — 2026-09-21

The first public release. Version 1.0.0 was built and used during development but
never published, so everything here is new to anyone installing indiBudget.

### Added

**Sharing a budget across computers.** A couple or a family can work from one
budget, each from their own computer. One machine hosts and the others connect
across the home network — no cloud service is involved at any point.

- Pairing with a short code read off the host's screen, plus an identity code
  that can be compared aloud to confirm you reached the right computer
- Per-person logins secured with Argon2id
- Five permission areas — Money, Planning, Structure, Reports and Admin — each
  settable to no access, read, or read and write
- Live updates, so a change made on one computer appears on the others within
  seconds
- Edit protection on budgets, accounts, categories and goals: opening one
  someone else is editing says so by name, before the typing rather than after
  the save. Transactions deliberately take no hold, so two people can log the
  evening's receipts at once without waiting on each other
- Maintenance mode, pausing everyone's changes while a backup is taken. Reading
  keeps working, and any administrator can reopen it
- Paired-computer management, including revoking a lost or stolen machine
  without changing anyone's password
- Deactivating someone who has left, without un-pairing the computers they used

**Other features**

- Editing and deleting budgets from the Budgets screen
- Subcategories, organising categories into parent and child groups
- A daily frequency for recurring transactions
- A user agreement shown on first run, with the terms the application is
  distributed under
- Sample data for screenshots and trying the app out, in `sample-data/`

### Changed

- Account balances are now derived from the opening balance plus every
  transaction, rather than stored and updated. A balance can no longer drift out
  of step with the transactions behind it
- Money is handled with decimal arithmetic end to end, on both sides of the
  application, removing the rounding error that floating point introduces
- SimpleFIN credentials moved from browser storage into the application
  database
- Moved to Tailwind CSS 4. Screens look the same apart from a slightly more
  vivid colour palette, most noticeably in reds
- Privacy wording throughout now describes what sharing actually does: your data
  stays on hardware you own and reaches no third party, but it does travel
  between your own computers when sharing is switched on

### Fixed

- Import no longer reports a false duplicate for legitimately similar
  transactions
- Backup import correctly reports real errors rather than silently skipping
- Several create and update operations that did not match their request shape
- Dependency vulnerabilities, and debug logging left in release builds

### Security

- Path-traversal protection on all file operations
- Content Security Policy to guard against script injection
- Sharing traffic is encrypted in transit, and each computer refuses to connect
  if the host's identity ever changes
- Repeated failed sign-ins are slowed down, and a failed attempt reveals nothing
  about whether that login exists
- All known vulnerabilities in third-party packages resolved; `npm audit`
  reports none

[1.1.0]: https://github.com/mattmilano/indiBudget/releases/tag/v1.1.0
