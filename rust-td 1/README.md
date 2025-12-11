## Features
- Multi-source fetching (Alpha Vantage, Finnhub, YahooMock)
- Parallel fetching with Tokio
- Periodic execution (every minute)
- PostgreSQL persistence via sqlx
- Graceful shutdown (Ctrl+C)
- Structured logging with tracing

## Setup
1. Install PostgreSQL and create a database:

```bash
createdb stockdb
psql stockdb < migrations/0001_create_stock_prices.sql
```

2. Copy `.env.example` to `.env` and update values:

```bash
cp .env.example .env
# Edit .env to set real values
```

3. Optionally set environment variables (PowerShell):

```powershell
$env:DATABASE_URL = 'postgresql://user:password@localhost/stockdb'
$env:ALPHA_VANTAGE_KEY = 'your_alpha_key'
$env:FINNHUB_KEY = 'your_finnhub_key'
```

## Run
- Run the app in continuous mode (fetch every minute):

```bash
cargo run
```

- Run a single fetch cycle and exit (useful for testing):

```bash
cargo run -- --fetch-once
```

- Query latest values from DB and exit:

```bash
cargo run -- --query-latest
```

