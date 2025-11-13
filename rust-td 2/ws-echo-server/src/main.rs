/*  ### Partie 1 : 

use env_logger::{Builder, Target};
use futures_util::{SinkExt, StreamExt};
use log::{LevelFilter, debug, error, info, trace, warn};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

async fn handle_connection(stream: TcpStream) {
    let addr = stream
        .peer_addr()
        .expect("connected streams should have a peer address");
    info!("New connection from: {}", addr);

    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("WebSocket handshake failed: {}", e);
            return;
        }
    };

    info!("WebSocket connection established: {}", addr);

    let (mut write, mut read) = ws_stream.split();

    if let Err(e) = write.send(Message::Text("welcome to rust ws".into())).await {
        warn!("Failed to send welcome to {}: {}", addr, e);
        return;
    }

    // Echo server: renvoie tout ce qu'on reçoit
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                info!("Received: {}", text);
                if write.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                info!("Client closed connection: {}", addr);
                break;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    info!("Connection closed: {}", addr);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::new()
        .target(Target::Stdout)
        .filter_level(LevelFilter::Info)
        .init();

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("WebSocket server listening on ws://127.0.0.1:8080");

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_connection(stream));
    }

    Ok(())
}
*/

/* ### Partie 2 : */
/* 
use env_logger::{Builder, Target};
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn, LevelFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PriceUpdate {
    symbol: String,
    price: f64,
    source: String,
    timestamp: i64,
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

    {
        let mut c = clients.lock().await;
        *c += 1;
        info!("Client connected: {} ({} active)", addr, *c);
    }

    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("WebSocket handshake failed for {}: {}", addr, e);
            // décrémente le compteur si handshake rate
            let mut c = clients.lock().await;
            *c -= 1;
            return;
        }
    };

    let (mut write, mut read) = ws.split();

    // Welcome JSON
    let welcome = serde_json::json!({
        "type": "connected",
        "message": "Connected to stock price feed"
    });
    if write
        .send(Message::Text(welcome.to_string()))
        .await
        .is_err()
    {
        let mut c = clients.lock().await;
        *c -= 1;
        return;
    }

    // Boucle: reçoit les updates (broadcast) + lit commandes client (/stats)
    loop {
        tokio::select! {
            // 1) reçoit un prix du canal et l'envoie au client
            Ok(update) = rx.recv() => {
                match serde_json::to_string(&update) {
                    Ok(json) => {
                        if write.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => warn!("Serialize error: {e}"),
                }
            }

            // 2) lit ce que le client envoie (ex: /stats)
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        if t.trim() == "/stats" {
                            let count = *clients.lock().await;
                            let _ = write.send(Message::Text(format!(r#"{{"type":"stats","active_clients":{}}}"#, count))).await;
                        } else {
                            // ici on pourrait gérer un filtre d'abonnement, etc.
                            info!("Client {} says: {}", addr, t.trim());
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!("Client closed: {}", addr);
                        break;
                    }
                    Some(Err(e)) => {
                        warn!("WS error from {}: {}", addr, e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    {
        let mut c = clients.lock().await;
        *c -= 1;
        info!("Client {} disconnected ({} active)", addr, *c);
    }
}

async fn price_simulator(tx: broadcast::Sender<PriceUpdate>) {
    use rand::Rng;
    use tokio::time::{interval, Duration};
    let mut iv = interval(Duration::from_secs(2));
    let symbols = ["AAPL", "GOOGL", "MSFT"];
    let sources = ["alpha_vantage", "finnhub"];

    loop {
        iv.tick().await;
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
        let _ = tx.send(update); // ok si aucun abonné
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::new()
        .target(Target::Stdout)
        .filter_level(LevelFilter::Info)
        .init();

    // canal broadcast (taille 100)
    let (tx, _rx) = broadcast::channel::<PriceUpdate>(100);

    // compteur de clients
    let clients = Arc::new(Mutex::new(0u32));

    // tâche producteur (simulateur de prix)
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            price_simulator(tx).await;
        });
    }

    // serveur WS
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("WebSocket server (broadcast) on ws://127.0.0.1:8080");

    loop {
        let (stream, _) = listener.accept().await?;
        let rx = tx.subscribe();
        let clients = clients.clone();
        tokio::spawn(handle_client(stream, rx, clients));
    }
}
*/

use env_logger::{Builder, Target};
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn, LevelFilter};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PriceUpdate {
    symbol: String,
    price: f64,
    source: String,
    timestamp: i64,
}


async fn handle_client(stream: TcpStream, mut rx: broadcast::Receiver<PriceUpdate>) {
    let addr = match stream.peer_addr() {
        Ok(a) => a,
        Err(_) => return,
    };
    info!("New client connected: {}", addr);

    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("WebSocket handshake failed: {}", e);
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    let welcome = serde_json::json!({
        "type": "connected",
        "message": "Connected to stock price feed"
    });

    if write
        .send(Message::Text(welcome.to_string()))
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
      
            Ok(price_update) = rx.recv() => {
                match serde_json::to_string(&price_update) {
                    Ok(json) => {
                        if write.send(Message::Text(json)).await.is_err() {
                            info!("Client disconnected: {}", addr);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize price update: {}", e);
                    }
                }
            }

         
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        info!("Received from {}: {}", addr, text);
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

    info!("Connection handler finished: {}", addr);
}

async fn fake_price_poller(tx: broadcast::Sender<PriceUpdate>) {
    use rand::Rng;
    use tokio::time::{interval, Duration};

    let mut timer = interval(Duration::from_secs(2));
    let symbols = vec!["AAPL", "GOOGL", "MSFT"];
    let sources = vec!["alpha_vantage", "finnhub"];

    loop {
        timer.tick().await;

        let mut rng = rand::thread_rng();
        let symbol = symbols[rng.gen_range(0..symbols.len())];
        let source = sources[rng.gen_range(0..sources.len())];
        let price = rng.gen_range(100.0..200.0);

        let update = PriceUpdate {
            symbol: symbol.to_string(),
            price,
            source: source.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        info!("Broadcasting: {} @ ${:.2} ({})", update.symbol, update.price, update.source);
        let _ = tx.send(update);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::new()
        .target(Target::Stdout)
        .filter_level(LevelFilter::Info)
        .init();


    let (tx, _rx) = broadcast::channel::<PriceUpdate>(100);


    {
        let txc = tx.clone();
        tokio::spawn(async move {
            fake_price_poller(txc).await;
        });
    }

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("WebSocket (fake DB) listening on ws://127.0.0.1:8080");

    while let Ok((stream, _)) = listener.accept().await {
        let rx = tx.subscribe();
        tokio::spawn(handle_client(stream, rx));
    }

    Ok(())
}
