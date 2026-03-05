use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use rand::RngExt;
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use std::{net::SocketAddr, time::Duration};
use tokio::time;

// This represents a standard price update
#[derive(Serialize, Clone)]
struct Tick {
    symbol: String,
    price: f64,
    volume: i32,
}

// This represents our High-Priority Risk Alert
#[derive(Serialize, Clone)]
struct RiskAlert {
    level: String,
    message: String,
}

// An enum so we can send either Ticks or Alerts over the same WebSocket
#[derive(Serialize)]
#[serde(tag = "type", content = "payload")]
enum WsMessage {
    Batch(Vec<Tick>), // We send an array of ticks to save network overhead
    Risk(RiskAlert),  // Sent immediately, bypassing the batch
}

#[tokio::main]
async fn main() {
    // Set up our standard WebSocket route
    let app = Router::new().route("/ws", get(ws_handler));
    
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("Aviator Mock Backend running on ws://127.0.0.1:8080/ws");
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let (mut sender, mut _receiver) = socket.split();

    // Firing every 25ms will give us 40 updates per second, which is a reasonable rate for a mock data feed
    let mut ticker = time::interval(Duration::from_millis(25));

    // The risk ticker fires every 5 seconds, simulating a critical alert that needs to be sent immediately
    let mut risk_ticker = time::interval(Duration::from_secs(5));

    println!("Client connected! Starting data firehose...");

    loop {
        tokio::select! {
            // STANDARD DATA HOSE
            _ = ticker.tick() => {
                
                let batch = {
                    // We create rng inside here
                    let mut rng = rand::rng(); 
                    let mut temp_batch = Vec::with_capacity(1000);
                    
                    for _ in 0..1000 {
                        temp_batch.push(Tick {
                            symbol: "AAPL".to_string(),
                            price: 150.0 + rng.random_range(-2.0..2.0),
                            volume: rng.random_range(100..1000),
                        });
                    }
                    // Return the data out of the block
                    temp_batch
                };

                let msg = WsMessage::Batch(batch);
                let json = serde_json::to_string(&msg).unwrap();
                
                if sender.send(Message::Text(json.into())).await.is_err() {
                    println!("Client disconnected.");
                    break;
                }
            }
            
            // PRIORITY RISK ALERT
            _ = risk_ticker.tick() => {
                let alert = WsMessage::Risk(RiskAlert {
                    level: "CRITICAL".to_string(),
                    message: "Exposure limit breached on AAPL!".to_string(),
                });
                
                let json = serde_json::to_string(&alert).unwrap();
                
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
                println!("Fired Risk Alert!");
            }
        }
    }
}