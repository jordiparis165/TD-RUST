use env_logger::{Builder, Target};
use futures_util::{SinkExt, StreamExt};
use log::{info, error};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

async fn handle_connection(stream: TcpStream) {
    let addr = match stream.peer_addr() {
        Ok(a) => a,
        Err(_) => return,
    };
    info!("New connection from {}", addr);

    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("Handshake failed: {e}");
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    if write.send(Message::Text("welcome to rust ws".into())).await.is_err() {
        return;
    }

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(txt)) => {
                info!("Got from {addr}: {txt}");
                if write.send(Message::Text(txt)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                info!("Client {addr} closed");
                break;
            }
            Err(e) => {
                error!("ws error ({addr}): {e}");
                break;
            }
            _ => {}
        }
    }

    info!("Connection closed: {}", addr);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Builder::new().target(Target::Stdout).init();

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("WS listening on ws://127.0.0.1:8080");

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_connection(stream));
    }

    Ok(())
}
