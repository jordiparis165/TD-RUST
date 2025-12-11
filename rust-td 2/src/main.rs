use env_logger::{Builder, Target};
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn, LevelFilter};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio::time::{interval, Duration};
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PriceUpdate {
    symbol: String,
    price: f64,
    source: String,
    timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Subscription {
    All,
    Symbol(String),
}

fn parse_subscription(cmd: &str) -> Option<Subscription> {
    let trimmed = cmd.trim();
    if trimmed.eq_ignore_ascii_case("SUB ALL") {
        return Some(Subscription::All);
    }
    if let Some(rest) = trimmed.strip_prefix("SUB ") {
        let sym = rest.trim().to_uppercase();
        if !sym.is_empty() {
            return Some(Subscription::Symbol(sym));
        }
    }
    None
}

async fn handle_client(
    stream: TcpStream,
    mut rx: broadcast::Receiver<PriceUpdate>,
    clients: Arc<Mutex<u32>>,
) {
    let addr = match stream.peer_addr() {
        Ok(a) => a,
        Err(_) => return,
    };

    // track active clients
    {
        let mut count = clients.lock().await;
        *count += 1;
        info!("Client connected: {} ({} active)", addr, *count);
    }

    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("WebSocket handshake failed for {}: {}", addr, e);
            let mut count = clients.lock().await;
            *count -= 1;
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // welcome message
    let welcome = serde_json::json!({
        "type": "connected",
        "message": "Connected to stock price feed"
    });
    if write
        .send(Message::Text(welcome.to_string()))
        .await
        .is_err()
    {
        let mut count = clients.lock().await;
        *count -= 1;
        return;
    }

    // per-client filter: None = all, Some(sym) = only that symbol
    let mut filter: Subscription = Subscription::All;

    loop {
        tokio::select! {
            // broadcast path
            Ok(update) = rx.recv() => {
                match &filter {
                    Subscription::All => {}
                    Subscription::Symbol(sym) if &update.symbol != sym => continue,
                    _ => {}
                }

                match serde_json::to_string(&update) {
                    Ok(json) => {
                        if write.send(Message::Text(json)).await.is_err() {
                            info!("Client disconnected: {}", addr);
                            break;
                        }
                    }
                    Err(e) => warn!("Serialize error: {e}"),
                }
            }

            // incoming messages
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        let trimmed = t.trim();
                        if trimmed.eq_ignore_ascii_case("/stats") {
                            let count = *clients.lock().await;
                            let _ = write.send(Message::Text(format!(r#"{{"type":"stats","active_clients":{}}}"#, count))).await;
                        } else if let Some(sub) = parse_subscription(trimmed) {
                            filter = sub.clone();
                            let label = match &filter {
                                Subscription::All => "ALL".to_string(),
                                Subscription::Symbol(s) => s.clone(),
                            };
                            let _ = write.send(Message::Text(format!(r#"{{"type":"subscribed","filter":"{}"}}"#, label))).await;
                        } else {
                            info!("Client {} says: {}", addr, trimmed);
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!("Client closed connection: {}", addr);
                        break;
                    }
                    Some(Err(e)) => {
                        warn!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // decrement active clients
    {
        let mut count = clients.lock().await;
        *count -= 1;
        info!("Client {} disconnected ({} active)", addr, *count);
    }
}

async fn fake_price_poller(tx: broadcast::Sender<PriceUpdate>) {
    use rand::Rng;

    let mut timer = interval(Duration::from_secs(2));
    let symbols = ["AAPL", "GOOGL", "MSFT"];
    let sources = ["alpha_vantage", "finnhub"];

    loop {
        timer.tick().await;

        let mut rng = rand::thread_rng();
        let symbol = symbols[rng.gen_range(0..symbols.len())];
        let source = sources[rng.gen_range(0..sources.len())];
        let price: f64 = rng.gen_range(100.0..200.0);

        let update = PriceUpdate {
            symbol: symbol.to_string(),
            price,
            source: source.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        info!("Broadcasting: {} @ {:.2} ({})", update.symbol, update.price, update.source);
        let _ = tx.send(update);
    }
}

async fn db_price_poller(pool: sqlx::Pool<sqlx::Postgres>, tx: broadcast::Sender<PriceUpdate>) {
    let mut timer = interval(Duration::from_secs(5));

    loop {
        timer.tick().await;
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (symbol, source)
                symbol, price, source, timestamp
            FROM stock_prices
            ORDER BY symbol, source, timestamp DESC
            "#,
        )
        .fetch_all(&pool)
        .await;

        match rows {
            Ok(rows) => {
                for row in rows {
                    let update = PriceUpdate {
                        symbol: row.try_get("symbol").unwrap_or_default(),
                        price: row.try_get("price").unwrap_or(0.0),
                        source: row.try_get("source").unwrap_or_default(),
                        timestamp: row.try_get("timestamp").unwrap_or_default(),
                    };
                    let _ = tx.send(update);
                }
            }
            Err(e) => {
                warn!("DB poll failed: {}", e);
            }
        }
    }
}

async fn start_feed(tx: broadcast::Sender<PriceUpdate>) -> bool {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        match PgPoolOptions::new().max_connections(5).connect(&url).await {
            Ok(pool) => {
                info!("Using DB feed (polling every 5s)");
                let pool_clone = pool.clone();
                let txc = tx.clone();
                tokio::spawn(async move {
                    db_price_poller(pool_clone, txc).await;
                });
                return true;
            }
            Err(e) => {
                warn!("Failed to connect DB, falling back to fake feed: {}", e);
            }
        }
    } else {
        info!("No DATABASE_URL set, using fake feed");
    }

    let txc = tx.clone();
    tokio::spawn(async move {
        fake_price_poller(txc).await;
    });
    false
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::new()
        .target(Target::Stdout)
        .filter_level(LevelFilter::Info)
        .init();

    // broadcast channel and client counter
    let (tx, _rx) = broadcast::channel::<PriceUpdate>(100);
    let clients = Arc::new(Mutex::new(0u32));

    // spawn producer (DB if available, else fake)
    let using_db = start_feed(tx.clone()).await;

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    if using_db {
        info!("WebSocket listening on ws://127.0.0.1:8080 (DB feed)");
    } else {
        info!("WebSocket listening on ws://127.0.0.1:8080 (fake feed)");
    }

    while let Ok((stream, _)) = listener.accept().await {
        let rx = tx.subscribe();
        let clients = clients.clone();
        tokio::spawn(handle_client(stream, rx, clients));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_subscription_handles_all_and_symbol() {
        assert_eq!(parse_subscription("SUB ALL"), Some(Subscription::All));
        assert_eq!(
            parse_subscription("SUB aapl"),
            Some(Subscription::Symbol("AAPL".into()))
        );
        assert_eq!(parse_subscription("SUB  aapl   "), Some(Subscription::Symbol("AAPL".into())));
        assert_eq!(parse_subscription("SUB"), None);
        assert_eq!(parse_subscription("/stats"), None);
    }
}
