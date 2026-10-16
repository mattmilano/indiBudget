---
title: "Introducing indiBudget: Your Household Budget, on Your Calendar and Your Own Computers"
description: "A free, open-source personal budget app built around a bill calendar — accounts, budgets, goals, and reports kept on your own computer, and with version 1.1, shared across the whole household over your own network. No account, no subscription, no cloud."
date: 2026-10-16
author: "Indomitus Group"
tags: ["indiBudget", "launch", "release", "personal-finance", "budgeting", "Quicken-alternative", "Linux", "open-source", "privacy"]
---

Most people don't think about their money in categories. They think about it in dates. The mortgage goes out on the 1st, the paycheck lands on the 15th, the car insurance hits in March. The question that actually matters is "will there be enough in checking on the 28th?", and a pie chart doesn't answer it.

The software that does answer it has drifted somewhere uncomfortable. Quicken is a subscription now. The newer budgeting apps want your bank login and keep your finances on their servers. And if you've moved to Linux, the choice has mostly been "run Windows for one program" or "go back to a spreadsheet."

We kept hearing the same request: *I just want to see my bills on a calendar, keep my money on my own computer, and let my partner use it too. Why is that so hard?*

So we built indiBudget, and we never properly introduced it. With version 1.1 out today, this is that introduction: what indiBudget is, everything it does, and what's new. It's free, it's open source, and it runs on Windows, macOS, and Linux.

## Your money, on a calendar

The **Bill Calendar** is the heart of indiBudget. Every transaction you've made and every bill still to come sits on the day it belongs to. Each day shows a summary of bills and deposits. Each month shows the income and expenses to expect. Click a day to see exactly what's on it, or switch to a list when you'd rather read than look.

Flip to **Upcoming Bills** and the calendar shows what's scheduled, with the next 30 days down the side, so the 28th stops being a surprise. Scheduled bills can be edited right there: click **Edit** on one and change the amount, the account, the category, how often it repeats, the next due date, or when it ends. Move next month's electric bill to the 20th and every one after it follows.

## A dashboard that answers "how are we doing?"

Open indiBudget and the **Dashboard** has the short version:

- your net worth, and how it moved over the last week and month
- every account's balance
- how each budget is tracking
- the bills due soon
- your latest activity
- how close each savings goal is

New to the app? It starts you off with a welcome and the two things to do first: add an account, or import a statement.

## Accounts and transactions

- **Seven account types** — checking, savings, credit card, cash, investment, loan, and other — in **twelve currencies**, with the institution and a masked account number (`****1234`) so you can tell your cards apart.
- **Balances that can't drift.** Every balance is worked out from the opening balance plus the transactions behind it, so it's never out of step with them. Net worth treats credit cards and loans as what you owe.
- **Transfers stay paired.** Moving money between accounts creates both sides together, and deleting one takes the other with it.
- **Reconcile against your statement.** Each transaction moves from pending to cleared to reconciled, or void, and the reconciliation screen matches your records against the bank's.
- **Split a transaction** across several categories — the one supermarket trip that was groceries, household, and a birthday present.
- **Find anything.** Search the text, filter by account, category, type, or date range, and see income, expenses, and net for whatever you're looking at. Export any set to CSV.
- **See where it goes, by payee.** The Payees page shows your spending patterns merchant by merchant.

## Budgets that fit how you're paid

Set budgets **weekly, every two weeks, monthly, quarterly, or yearly**, and roll unused money into the next period if you like. Don't know where to start? Pick a template — **50/30/20**, **zero-based**, **minimalist**, or **family** — and preview it before you commit. Progress bars warn you before the month does.

## Categories that fill themselves in

indiBudget comes with **more than 35 colour-coded categories**, and you can add as many of your own as you like, grouped into **subcategories**. Its categorisation engine recognises **hundreds of real-world merchants**. Your own rules sit on top, from plain text to regular expressions, in the order you choose.

New transactions are categorised as they're imported. Old ones can be done in one click, or in batches by keyword, saving the keyword as a rule for next time.

## Bills, subscriptions, and goals

- **Recurring transactions** daily, weekly, every two weeks, monthly, quarterly, or yearly. A bill on the 31st lands on the last day of shorter months and goes back to the 31st after them.
- **Subscriptions, found for you.** indiBudget looks through your history for recurring charges and tells you how confident it is in each one.
- **Subscriptions, counted.** Cancel one and indiBudget keeps a running total of what you've saved, by the month and by the year.
- **Reminders** arrive as native notifications on your desktop before a bill is due, as early as you choose.
- **Goals with a number attached.** Savings, debt payoff, a big purchase, an emergency fund, or anything else. Give a goal a date and indiBudget tells you how much a month keeps you on pace.

## Reports that answer real questions

- net worth, and its history day by day
- cash flow, with running balances
- spending by category
- income against expenses
- monthly trends
- your savings rate, with your best and worst months against a 20% target
- year-over-year comparison

Every report exports to **PDF** or **CSV**.

## Bring your history with you

Import **CSV, Excel, OFX, QFX, and QIF** — QIF being the format Quicken exports. indiBudget recognises the format on its own and walks you through matching up the columns when it needs to.

Duplicate detection means importing the same file twice doesn't double anything. Transactions are categorised on the way in, and an import history keeps track of what came from where.

Would rather your bank fed it? Optional **SimpleFIN** sync brings transactions in on demand, daily, or weekly. Your bank password never touches indiBudget; SimpleFIN gives it an access token and nothing more.

## One budget for the whole household

This is the headline of 1.1. A couple or a family can now work from one budget, each from their own computer.

Press **Start hosting** on the computer that keeps the budget. The others find it on your home network by name — "indiBudget on STUDY-DESKTOP" — and pair with a short code read across the room. Everyone signs in as themselves. There's no server to install, nothing hosted, and no cloud in the middle. The budget never leaves your house.

It works the way a household actually does, not just "two people can open the file":

- **Everyone gets their own login, and their own reach.** Add people from the **People** screen. Access is set in five areas: Money, Planning, Structure, Reports, and Admin. Ready-made presets cover the common cases. A partner gets everything. A teenager can track their own goals and see the spending charts without seeing the mortgage.
- **You're told before you type, not after you save.** Open a budget or an account someone else is editing and it says so by name — "Jess is editing this account right now." Two people logging the evening's receipts at once never wait on each other, and nobody's work is ever quietly overwritten.
- **Changes show up almost at once.** The moment someone saves, every other computer hears about it and catches up, without anyone pressing refresh.
- **It's always clear whose budget you're in.** A slim blue bar across the top of the window says which computer you're connected to and who you're signed in as. On the host, it says who else is working in the budget. One button disconnects.
- **Removing access is immediate.** Lose a laptop, and you revoke that computer without anyone changing a password. Someone moves out, and you deactivate them without re-pairing anything. Both take effect on their very next click.
- **A pause button for backups.** Close the budget for maintenance and everyone can still look, but changes wait until it reopens. Any administrator can reopen it, so a forgotten sign never strands anyone.
- **Every connection is encrypted** and pinned to the host computer, and the key that proves a computer was paired lives in your operating system's own keychain.

## Your data is yours

indiBudget has no account to create and no server of ours for your information to pass through. Nothing is ever sent to us or to anyone else. With sharing switched off, which is how it starts, your budget never leaves the computer it's on. With sharing on, it travels only between your own computers, over your own network, encrypted.

If you'd like a lock on it too, turn on **AES-256-GCM encryption** for the data on disk, protected by a password run through Argon2id. Take a full backup whenever you like, and indiBudget will remind you if it's been a while.

We spent the last stretch before this release on the details that decide whether you trust software with your money:

- **Money adds up exactly.** Amounts are handled as exact decimals from end to end, so a cent never goes missing to rounding.
- **Dates mean the day they say.** A bill due on the 20th shows on the 20th, wherever in the world you are.
- **Your budget file is always complete.** It's brought fully up to date after every change, so a copy taken by hand has everything in it.
- **It warns you about risky places.** If your budget ends up on a network drive or inside a Dropbox, OneDrive, or iCloud folder, indiBudget tells you. Those are exactly the places database files get corrupted, and it tells you how to move it somewhere safe. To share between computers, use hosting, which is the thing that actually works.
- **One window at a time.** Open indiBudget twice on the same computer and the second window says so, rather than two copies quietly disagreeing about who's editing what.

## Make it yours

Light, dark, or follow your system. Your currency and your date format.

## New in 1.1

Version 1.0 went out quietly, without an announcement. If you've been running it, here's what 1.1 brings:

- **Sharing a budget across computers**, with everything above: pairing, finding the host automatically, per-person logins and access, the People screen, named edit protection, live updates, the connection bar, and maintenance mode.
- **Editing scheduled transactions in place**, from the Recurring page or straight from the calendar, instead of deleting and recreating them.
- **Editing and deleting budgets.**
- **Subcategories.**
- **A daily frequency** for recurring transactions.
- **Balances derived from transactions** and **exact decimal money** throughout.
- **Fixes**, including:
  - saving an account no longer shifts its balance
  - a goal contribution now adds to what's saved instead of replacing it
  - dates no longer show a day early in the Americas
  - monthly bills on the 29th to 31st no longer drift
  - import no longer flags legitimately similar transactions as duplicates
- **The risky-location warning**, **one window at a time**, and **a complete budget file** after every change.
- **Security hardening:** protection against path tricks in file operations, a content security policy, sign-in throttling, and every known vulnerability in third-party packages resolved.

The full list is in the [changelog](https://github.com/mattmilano/indiBudget/blob/v1.1.0/CHANGELOG.md).

## Free, and open source

indiBudget is free. Not a trial, not a free tier with the good parts held back. All of it, for as long as you want it.

The source code is published under the **MIT license**, so anyone can read exactly what it does with their data, and build it themselves if they'd rather. The ready-to-install versions we distribute come with a short user agreement you'll see on first run.

## Getting it

indiBudget 1.1 is on [GitHub](https://github.com/mattmilano/indiBudget/releases/tag/v1.1.0) for **Windows**, **macOS** (Apple Silicon and Intel), and **Linux** (AppImage, .deb, and .rpm).

If you're leaving Quicken, export a QIF, add an account in indiBudget, import the file into it, and see your own bills on the calendar before you decide anything. If someone else in the house manages money with you, start hosting and add them from the **People** screen. They'll be on the same budget in a couple of minutes.

Your money. Your computers. Nobody else's business.
