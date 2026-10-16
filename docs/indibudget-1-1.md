---
title: "indiBudget 1.1: Your Household Budget, on Your Calendar and Your Own Computers"
description: "A free, open-source personal budget app built around a bill calendar — accounts, budgets, goals, and reports kept on your own computer, and now shared across the whole household over your own network. No account, no subscription, no cloud."
date: 2026-10-16
author: "Indomitus Group"
tags: ["indiBudget", "release", "personal-finance", "budgeting", "Quicken-alternative", "Linux", "open-source", "privacy"]
---

Most people don't think about their money in categories. They think about it in dates. The mortgage goes out on the 1st, the paycheck lands on the 15th, the car insurance hits in March. The question that actually matters is "will there be enough in checking on the 28th?", and a pie chart doesn't answer it.

The software that does answer it has drifted somewhere uncomfortable. Quicken is a subscription now. The newer budgeting apps want your bank login and keep your finances on their servers. And if you've moved to Linux, the choice has mostly been "run Windows for one program" or "go back to a spreadsheet."

We kept hearing the same request: *I just want to see my bills on a calendar, keep my money on my own computer, and let my partner use it too. Why is that so hard?*

It isn't anymore. indiBudget 1.1 is out today, it's free, and it's open source. (1.0 was the version we built and lived with ourselves; 1.1 is the first one we're handing to everybody.)

## Your money, on a calendar

The **Bill Calendar** is the heart of indiBudget. Every transaction you've made and every bill still to come sits on the day it belongs to. Each day shows a summary of what's going out and what's coming in, and each month shows what to expect in total. Click a day to see exactly what's on it.

Switch to **Upcoming Bills** and the calendar shows what's scheduled, with the next 30 days listed down the side. Scheduled transactions are first-class citizens. Click **Edit** on one, right there in the calendar, and change the amount, the account, the category, how often it repeats, the next due date, or when it ends. Move next month's electric bill to the 20th and every one after it follows. Nothing to delete and set up again.

## Everything a household budget needs

- **Accounts and net worth.** Checking, savings, credit cards, cash, investments, and loans, in twelve currencies. Balances are always worked out from the transactions behind them, so they can't drift. Net worth treats what you owe as what you owe. Reconcile against a bank statement and transfer between accounts with both sides kept in step.
- **Budgets that fit how you're paid.** Weekly, every two weeks, monthly, quarterly, or yearly, with optional rollover. Start from a template — 50/30/20, zero-based, minimalist, or family — and watch the progress bars warn you before the month does.
- **Categorisation that does the boring part.** Over 35 built-in categories plus your own, with subcategories. An engine that recognises hundreds of real merchants, and your own rules on top.
- **Subscriptions, found and counted.** indiBudget spots the recurring charges in your history. When you cancel one, it keeps a running total of what you've saved by the year.
- **Goals with a number attached.** Savings, debt payoff, a big purchase, or an emergency fund. indiBudget tells you how much a month keeps you on pace.
- **Reports that answer real questions.** Net worth over time, cash flow, spending by category, income against expenses, savings rate, and year over year. Export to PDF or CSV.
- **Bring your history with you.** Import CSV, Excel, OFX, QFX, and **QIF — the format Quicken exports** — with duplicate detection, so importing the same file twice doesn't double anything. If you'd like your bank to feed it, optional **SimpleFIN** sync brings transactions in without your bank password ever touching indiBudget.

## One budget for the whole household

This is the big one. A couple or a family can now work from one budget, each from their own computer.

Press **Start hosting** on the computer that keeps the budget. The others find it on your home network by name — "indiBudget on STUDY-DESKTOP" — and pair with a short code read across the room. Everyone signs in as themselves. There's no server to install, nothing hosted, and no cloud in the middle. The budget never leaves your house.

It works the way a household actually does, not just "two people can open the file":

- **Everyone gets their own login, and their own reach.** Access is set in five areas: Money, Planning, Structure, Reports, and Admin. Ready-made presets cover the common cases. A partner gets everything. A teenager can track their own goals and see the spending charts without seeing the mortgage.
- **You're told before you type, not after you save.** Open a budget or an account someone else is editing and it says so by name — "Jess is editing this account right now." Two people logging the evening's receipts at the same time never wait on each other, and nobody's work is ever quietly overwritten.
- **Changes show up almost at once.** The moment someone saves, every other computer hears about it and catches up, without anyone pressing refresh.
- **It's always clear whose budget you're in.** A slim blue bar across the top of the window says which computer you're connected to and who you're signed in as. On the host, it says who else is working in the budget. One button disconnects.
- **Removing access is immediate.** Lose a laptop, and you revoke that computer without anyone changing a password. Someone moves out, and you deactivate them without re-pairing anything. Both take effect on their very next click, not the next time they sign in.
- **Every connection is encrypted** and pinned to the host computer, and the key that proves a computer was paired lives in your operating system's own keychain.

## Your data is yours

indiBudget has no account to create and no server of ours for your information to pass through. Nothing is ever sent to us or to anyone else. With sharing switched off, which is how it starts, your budget never leaves the computer it's on. With sharing on, it travels only between your own computers, over your own network, encrypted.

If you'd like a lock on it too, turn on **AES-256-GCM encryption** for the data on disk, with an Argon2id-protected password. Take a full backup whenever you like, and indiBudget will remind you if you haven't in a while.

We spent the last stretch before release on the details that decide whether you trust software with your money:

- **Money adds up exactly.** Amounts are handled as exact decimals from end to end, so a cent never goes missing to rounding.
- **Dates mean the day they say.** A bill due on the 20th shows on the 20th, wherever you are. A bill set up for the 31st lands on the last day of shorter months and goes back to the 31st after them.
- **Your budget file is always complete.** It's brought fully up to date after every change, so a copy taken by hand has everything in it.
- **It warns you about risky places.** If your budget ends up on a network drive or inside a Dropbox, OneDrive, or iCloud folder, indiBudget tells you. Those are exactly the places database files get corrupted, and it tells you how to move it somewhere safe. To share between computers, use hosting, which is the thing that actually works.
- **One window at a time.** Open indiBudget twice on the same computer and the second window says so, rather than two copies quietly disagreeing about who's editing what.

## Free, and open source

indiBudget is free. Not a trial, not a free tier with the good parts held back. All of it, for as long as you want it.

The source code is published under the **MIT license**, so anyone can read exactly what it does with their data, and build it themselves if they'd rather. The ready-to-install versions we distribute come with a short user agreement you'll see on first run.

## Getting it

indiBudget 1.1 is on [GitHub](https://github.com/mattmilano/indiBudget/releases/tag/v1.1.0) for **Windows**, **macOS** (Apple Silicon and Intel), and **Linux** (AppImage, .deb, and .rpm).

If you're leaving Quicken, export a QIF, add an account in indiBudget, import the file into it, and see your own bills on the calendar before you decide anything. If someone else in the house manages money with you, start hosting and add them from the **People** screen. They'll be on the same budget in a couple of minutes.

Your money. Your computers. Nobody else's business.
