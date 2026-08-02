# indiBudget

A personal budget application with calendar-focused expense tracking. Built with Tauri, Vue.js, and Rust for cross-platform support (Linux, macOS, Windows, and Android).

## Features

- **Calendar View** - Visualize your expenses and income on a calendar
- **Bank Statement Import** - Import transactions from CSV, Excel, and other formats
- **Smart Categorization** - Auto-categorize transactions based on patterns
- **Budget Tracking** - Set spending limits per category and track progress
- **Recurring Transactions** - Manage bills and subscriptions
- **Multiple Accounts** - Track checking, savings, credit cards, and more
- **Savings Goals** - Set and track financial goals
- **Reports & Charts** - Spending analysis, trends, and cash flow reports
- **Balance Projections** - See projected balances based on upcoming transactions
- **Data Security** - Local SQLite database with optional encryption

## Prerequisites

- [Node.js](https://nodejs.org/) (v18 or later)
- [Rust](https://rustup.rs/) (latest stable)
- Platform-specific dependencies for Tauri:
  - **Linux**: `webkit2gtk`, `libappindicator`, `librsvg`
  - **macOS**: Xcode Command Line Tools
  - **Windows**: Microsoft Visual Studio C++ Build Tools

## Installation

```bash
# Clone the repository
git clone https://github.com/mattmilano/indiBudget.git
cd indiBudget

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Project Structure

```
indiBudget/
├── src/                    # Vue.js frontend
│   ├── components/        # Reusable Vue components
│   ├── views/             # Page components
│   ├── stores/            # Pinia state management
│   ├── router/            # Vue Router configuration
│   ├── services/          # API service layer
│   └── types/             # TypeScript type definitions
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── commands/      # Tauri IPC commands
│   │   ├── database/      # SQLite database layer
│   │   ├── models/        # Data models
│   │   └── services/      # Business logic
│   └── Cargo.toml
├── package.json
└── tauri.conf.json
```

## Technology Stack

- **Frontend**: Vue 3, TypeScript, Tailwind CSS, Pinia, Vue Router
- **Backend**: Rust, Tauri 2.0
- **Database**: SQLite (via rusqlite)
- **Charts**: Chart.js, vue-chartjs
- **Calendar**: FullCalendar
- **Date Handling**: date-fns, chrono

## Development

```bash
# Run development server
npm run tauri dev

# Type check
npm run build

# Format code
npm run format
```

## Building for Production

```bash
# Build for current platform
npm run tauri build

# Build for Android (requires Android SDK)
npm run tauri android build
```

## License

The source code is released under the MIT License - see [LICENSE](LICENSE) for details.

Use of the distributed application is additionally governed by the
[User Agreement](LICENSE-USER-AGREEMENT.md), which is presented on first run.
It does not restrict your rights under the MIT License to use, modify, or
redistribute the source code.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
