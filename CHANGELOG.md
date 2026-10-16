# Changelog

All notable changes to indiBudget are recorded here.

This project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html), and
the format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.1.0] — 2026-09-21

The first release to be announced. Version 1.0.0 was released quietly, without
an announcement; the changes below are what 1.1.0 adds to it.

### Added

**Sharing a budget across computers.** A couple or a family can work from one
budget, each from their own computer. One machine hosts and the others connect
across the home network — no cloud service is involved at any point.

- Pairing with a short code read off the host's screen, plus an identity code
  that can be compared aloud to confirm you reached the right computer
- Per-person logins secured with Argon2id
- A People screen for adding household members, choosing what each can reach
  (with ready-made presets), resetting passwords, and deactivating or removing
  someone. Anyone can change their own password
- Five permission areas — Money, Planning, Structure, Reports and Admin — each
  settable to no access, read, or read and write
- Finding the host on your network automatically, rather than typing its
  address (typing it still works where the network does not pass the
  announcements on)
- A list of who is connected right now, by person and computer, and a slim
  blue bar across the top of the window naming the host (when joined) or
  whoever is connected (when hosting), with a button to disconnect
- Live updates: the host tells every connected computer the moment something
  changes, so it appears on the others almost at once. A five-second check
  runs underneath in case that message is lost
- Edit protection on budgets, accounts, categories and goals: opening one
  someone else is editing says so by name, before the typing rather than after
  the save. Transactions deliberately take no hold, so two people can log the
  evening's receipts at once without waiting on each other. Underneath, a save
  built on a record that changed since it was opened is refused, with what
  you typed kept in place
- Maintenance mode, pausing everyone's changes while a backup is taken. Reading
  keeps working, and any administrator can reopen it
- Paired-computer management, including revoking a lost or stolen machine
  without changing anyone's password
- Deactivating someone who has left, without un-pairing the computers they used

**Other features**

- Editing scheduled (recurring) transactions in place — description, amount,
  expense or income, account, category, payee, frequency, next due date and
  end date — from the Recurring page or straight from the calendar, rather
  than deleting and recreating them
- An MIT `LICENSE` file, and a user agreement that covers only the distributed
  application, not the source code

### Changed

- A dismissable warning when the database is on a network drive or inside a
  folder a sync service (Dropbox, OneDrive, iCloud Drive and others) keeps in
  step, where it could be corrupted. `INDIBUDGET_DATA_DIR` points indiBudget
  at a local folder instead
- Only one window can have the budget open at a time, so two windows never
  keep separate ideas of who is editing what
- The database file is brought fully up to date after every change, so a copy
  taken by hand is always complete
- Moved to Tailwind CSS 4. Screens look the same apart from a slightly more
  vivid colour palette, most noticeably in reds
- Privacy wording throughout now describes what sharing actually does: your data
  stays on hardware you own and reaches no third party, but it does travel
  between your own computers when sharing is switched on

### Fixed

- Saving an account edit no longer moves its balance: the balance box showed
  the current balance but saved it as the opening balance, counting every
  transaction a second time
- Recording a goal contribution added it in place of the amount already saved,
  rather than on top of it
- Dates such as "Next Due" showed the day before in time zones west of
  Greenwich, including all of the Americas
- A monthly bill set up on the 29th, 30th or 31st no longer drifts to an
  earlier day after a short month

### Security

- Sharing traffic is encrypted in transit, and each computer refuses to connect
  if the host's identity ever changes
- The key that proves a computer was paired is kept in the operating system's
  keychain where one is available. Pairing the same computer again replaces
  its earlier entry rather than adding a second
- Repeated failed sign-ins are slowed down, and a failed attempt reveals nothing
  about whether that login exists
- Access changes, deactivation and revoking a computer take effect on the very
  next request, not the next sign-in. Nobody can deactivate, demote or remove
  their own account
- All known vulnerabilities in third-party packages resolved; `npm audit`
  reports none

## [1.0.0] — late June or early July 2026

The first release, published quietly without an announcement. The exact date
was not recorded.

### Added

- **Bill Calendar** in month and list views, showing actual transactions or
  upcoming bills, with daily summaries, monthly expected income and expenses,
  and a 30-day upcoming-bills list
- **Dashboard** with net worth and its 7- and 30-day change, account balances,
  budget status, upcoming bills, recent activity and goal progress
- **Accounts** of seven types in twelve currencies, with net worth that treats
  credit cards and loans as liabilities, linked transfers between accounts,
  and statement reconciliation
- **Transactions** with a pending, cleared, reconciled and void workflow, split
  transactions, full-text search and filtering, and CSV export
- **Budgets** over five periods with optional rollover, templates (50/30/20,
  zero-based, minimalist and family), and editing and deleting
- **Categories**: over 35 built in plus your own, with subcategories, an
  auto-categorisation engine recognising hundreds of merchants, text and
  regular-expression rules, and batch categorisation by keyword
- **Recurring transactions** daily, weekly, every two weeks, monthly, quarterly
  or yearly, with detection of recurring charges in your history and a running
  total of what cancelled subscriptions save
- **Savings goals** of five types, with the monthly amount needed to stay on pace
- **Reports**: net worth and its history, cash flow, spending by category,
  income against expenses, monthly trends, savings rate and year-over-year
  comparison, exported to PDF or CSV
- **Import** from CSV, Excel, OFX, QFX and QIF with duplicate detection and an
  import history, and optional **SimpleFIN** bank sync
- **Payee analytics**, native bill **notifications**, full JSON **backup and
  restore** with reminders, and light, dark and system **themes**
- Optional **AES-256-GCM encryption** of data at rest, with Argon2id key
  derivation
- Balances derived from the opening balance plus every transaction, rather
  than stored and updated, so a balance cannot drift out of step, and money
  handled with exact decimal arithmetic end to end
- A user agreement shown on first run, and sample data for screenshots and
  trying the app out, in `sample-data/`

### Security

- SimpleFIN credentials are kept in the application database rather than
  browser storage
- Path-traversal protection on all file operations, and a Content Security
  Policy to guard against script injection

[1.1.0]: https://github.com/mattmilano/indiBudget/releases/tag/v1.1.0
[1.0.0]: https://github.com/mattmilano/indiBudget/releases/tag/v1.0.0
