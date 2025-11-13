//**Part 1 – Intro to Async & Tokio Runtime (30 min)**
 
use rand::Rng;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use dotenv::dotenv;
/* 
async fn fetch_mock_price(symbol: &str) -> f64 {
    let mut rng = rand::thread_rng();
    sleep(Duration::from_millis(500)).await;
    let price: f64 = rng.gen_range(100.0..200.0);
    println!("{}: ${:.2}", symbol, price);
    price
}

#[tokio::main]
async fn main() {
    let start = Instant::now();

    fetch_mock_price("AAPL").await;
    fetch_mock_price("GOOG").await;
    fetch_mock_price("AMZN").await;

    println!("Done in {:?}", start.elapsed());
}
*/


//**Part 2 – Async API Calls & Parallel Fetching (60 min)**
use reqwest;
use serde::Deserialize;
use std::env;
use chrono::Utc;
use tracing::{info, error, instrument};
use tracing_subscriber;
use sqlx::Row;
use tracing::Level;
use tokio::time::interval;
use std::time::Duration;
use tokio::signal;
use clap::Parser;


#[derive(Deserialize, Debug)]
struct GlobalQuote {
    #[serde(rename = "Global Quote")]
    quote: Quote,
}

#[derive(Deserialize, Debug)]
struct Quote {
    #[serde(rename = "01. symbol")]
    symbol: String,
    #[serde(rename = "05. price")]
    price: String,
}

#[derive(Deserialize, Debug)]
struct FinnhubQuote {
    c: f64, // current price
    t: i64, // timestamp
}

#[derive(Debug)]
struct StockPrice {
    symbol: String,
    price: f64,
    source: String,
    timestamp: i64,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Fetch once and exit
    #[arg(long)]
    fetch_once: bool,

    /// Query latest prices from DB and exit
    #[arg(long)]
    query_latest: bool,
}

async fn fetch_alpha_vantage(symbol: &str) -> Result<StockPrice, Box<dyn std::error::Error>> {
    // Try to read API key; if missing, return a mock price
    let api_key = match env::var("ALPHA_VANTAGE_KEY") {
        Ok(k) => k,
        Err(_) => return Ok(fetch_mock_price(symbol, "AlphaVantage")),
    };

    let url = format!(
        "https://www.alphavantage.co/query?function=GLOBAL_QUOTE&symbol={}&apikey={}",
        symbol, api_key
    );

    // If the HTTP call or parsing fails, fall back to mock
    match reqwest::get(&url).await {
        Ok(resp) => match resp.json::<GlobalQuote>().await {
            Ok(data) => {
                if let Ok(price) = data.quote.price.parse::<f64>() {
                    return Ok(StockPrice {
                        symbol: symbol.to_string(),
                        price,
                        source: "AlphaVantage".to_string(),
                        timestamp: Utc::now().timestamp(),
                    });
                }
                // parsing failed -> fallback
                Ok(fetch_mock_price(symbol, "AlphaVantage"))
            }
            Err(_) => Ok(fetch_mock_price(symbol, "AlphaVantage")),
        },
        Err(_) => Ok(fetch_mock_price(symbol, "AlphaVantage")),
    }
}

async fn fetch_finnhub(symbol: &str) -> Result<StockPrice, Box<dyn std::error::Error>> {
    let api_key = match env::var("FINNHUB_KEY") {
        Ok(k) => k,
        Err(_) => return Ok(fetch_mock_price(symbol, "Finnhub")),
    };

    let url = format!("https://finnhub.io/api/v1/quote?symbol={}&token={}", symbol, api_key);

    match reqwest::get(&url).await {
        Ok(resp) => match resp.json::<FinnhubQuote>().await {
            Ok(data) => Ok(StockPrice {
                symbol: symbol.to_string(),
                price: data.c,
                source: "Finnhub".to_string(),
                timestamp: data.t,
            }),
            Err(_) => Ok(fetch_mock_price(symbol, "Finnhub")),
        },
        Err(_) => Ok(fetch_mock_price(symbol, "Finnhub")),
    }
}

fn fetch_mock_price(symbol: &str, source: &str) -> StockPrice {
    let mut rng = rand::thread_rng();
    let price = rng.gen_range(100.0..200.0);
    StockPrice {
        symbol: symbol.to_string(),
        price,
        source: source.to_string(),
        timestamp: Utc::now().timestamp(),
    }
}
async fn save_price(pool: &PgPool, price: &StockPrice) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO stock_prices (symbol, price, source, timestamp) VALUES ($1, $2, $3, $4)"#,
    )
    .bind(&price.symbol)
    .bind(price.price)
    .bind(&price.source)
    .bind(price.timestamp)
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Deserialize, Debug)]
struct YahooQuote {
    symbol: Option<String>,
    regularMarketPrice: Option<f64>,
    regularMarketTime: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct YahooResult {
    result: Vec<YahooQuote>,
}

#[derive(Deserialize, Debug)]
struct YahooQuoteResponse {
    quoteResponse: YahooResult,
}

async fn fetch_yahoo(symbol: &str) -> Result<StockPrice, Box<dyn std::error::Error>> {
    // Yahoo public quote endpoint
    let url = format!("https://query1.finance.yahoo.com/v7/finance/quote?symbols={}", symbol);

    match reqwest::get(&url).await {
        Ok(resp) => match resp.json::<YahooQuoteResponse>().await {
            Ok(data) => {
                if let Some(q) = data.quoteResponse.result.into_iter().next() {
                    if let Some(price) = q.regularMarketPrice {
                        return Ok(StockPrice {
                            symbol: symbol.to_string(),
                            price,
                            source: "Yahoo".to_string(),
                            timestamp: q.regularMarketTime.unwrap_or_else(|| Utc::now().timestamp()),
                        });
                    }
                }
                // fallback
                Ok(fetch_mock_price(symbol, "Yahoo"))
            }
            Err(_) => Ok(fetch_mock_price(symbol, "Yahoo")),
        },
        Err(_) => Ok(fetch_mock_price(symbol, "Yahoo")),
    }
}

async fn query_latest(pool: &PgPool, symbols: &[&str]) -> Result<(), sqlx::Error> {
    for &sym in symbols {
        let res = sqlx::query(
            r#"SELECT symbol, price, source, timestamp, created_at FROM stock_prices WHERE symbol = $1 ORDER BY timestamp DESC LIMIT 1"#,
        )
        .bind(sym)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = res {
            let symbol: String = row.try_get("symbol")?;
            let price: f64 = row.try_get("price")?;
            let source: String = row.try_get("source")?;
            let timestamp: i64 = row.try_get("timestamp")?;
            println!("Latest {}: {} (source={}, ts={})", symbol, price, source, timestamp);
        } else {
            println!("No data for {}", sym);
        }
    }

    Ok(())
}

#[instrument(skip(pool))]
async fn fetch_and_save_all(pool: Option<&PgPool>, symbols: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    info!(count = symbols.len(), "Starting fetch cycle");

    for symbol in symbols {
        let (a_res, f_res, y_res) = tokio::join!(
            fetch_alpha_vantage(symbol),
            fetch_finnhub(symbol),
            fetch_yahoo(symbol)
        );

        if let Ok(a) = a_res {
            info!(symbol = %a.symbol, source = %a.source, price = a.price, "Alpha result");
            if let Some(pool) = pool { save_price(pool, &a).await?; }
        } else { error!(symbol = %symbol, "Alpha failed"); }

        if let Ok(f) = f_res {
            info!(symbol = %f.symbol, source = %f.source, price = f.price, "Finnhub result");
            if let Some(pool) = pool { save_price(pool, &f).await?; }
        } else { error!(symbol = %symbol, "Finnhub failed"); }

        if let Ok(y) = y_res {
            info!(symbol = %y.symbol, source = %y.source, price = y.price, "Yahoo result");
            if let Some(pool) = pool { save_price(pool, &y).await?; }
        } else { error!(symbol = %symbol, "Yahoo failed (unexpected)"); }
    }

    info!("Completed fetch cycle");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    // Setup tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let cli = Cli::parse();

    // Optional database connection
    let db_url = env::var("DATABASE_URL").ok();
    let pool = if let Some(ref url) = db_url {
        Some(
            PgPoolOptions::new()
                .max_connections(5)
                .connect(url)
                .await?,
        )
    } else {
        None
    };

    let symbols = vec!["AAPL".to_string(), "GOOG".to_string(), "AMZN".to_string()];

    if cli.query_latest {
        if let Some(ref pool) = pool {
            query_latest(pool, &["AAPL", "GOOG", "AMZN"]).await?;
            return Ok(());
        } else {
            println!("DATABASE_URL not set; no data to query");
            return Ok(());
        }
    }

    if cli.fetch_once {
        fetch_and_save_all(pool.as_ref(), &symbols).await?;
        return Ok(());
    }

    info!("Starting periodic fetcher");

    let mut interval = interval(Duration::from_secs(60));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = fetch_and_save_all(pool.as_ref(), &symbols).await {
                    error!("Fetch cycle failed: {}", e);
                }
            }
            _ = signal::ctrl_c() => {
                info!("Shutdown requested via ctrl-c");
                break;
            }
        }
    }

    info!("Shutting down: closing DB pool");
    if let Some(pool) = pool {
        pool.close().await;
    }

    info!("Shutdown complete");
    Ok(())
}

