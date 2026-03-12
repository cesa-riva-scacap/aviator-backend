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
    isin: String,
    name: String,
    xetra_mid: f64,
    xetra_spr: f64,
    lsx_spr: f64,
    gettex_spr: f64,
    trade_gate_spr: f64,
    bid_size: f64,
    ask_size: f64,
    vol_xetra: f64,
    vol_lsx: f64,
    vol_gettex: f64,
    vwap: f64,
    day_high: f64,
    day_low: f64,
    ytd_perf: f64,
    moving_avg: f64,
    rsi: f64,
    macd: f64,
    bollinger: f64,
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
    const BATCH_SIZE: usize = 3000;
    let (mut sender, mut _receiver) = socket.split();

    let mut symbols = Vec::with_capacity(BATCH_SIZE);
    
    for i in 0..BATCH_SIZE {
        symbols.push(format!("GB00BLD4Z{:03}", i));
    }

    /* Firing every 25ms will give us 40 updates per second, 
     * which is a reasonable rate for a mock data feed 
     * */
    let mut ticker = time::interval(Duration::from_millis(25));

    /* The risk ticker fires every 5 seconds, simulating a 
     * critical alert that needs to be sent immediately 
     * */
    let mut risk_ticker = time::interval(Duration::from_secs(5));

    println!("Client connected! Starting data firehose...");

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let batch = {
                    let mut rng = rand::rng(); 
                    let mut temp_batch = Vec::with_capacity(BATCH_SIZE);
                    
                    for i in 0..BATCH_SIZE {
                        let base_price = 100.0 + rng.random_range(0.0..50.0);

                        temp_batch.push(Tick {
                            isin: symbols[i].clone(),
                            name: "CoinShares ETP".to_string(),
                            xetra_mid: base_price + rng.random_range(-0.05..0.05),
                            xetra_spr: rng.random_range(0.05..0.15),
                            lsx_spr: rng.random_range(0.06..0.16),
                            gettex_spr: rng.random_range(0.07..0.17),
                            trade_gate_spr: rng.random_range(0.08..0.18),
                            bid_size: rng.random_range(100.0..5000.0),
                            ask_size: rng.random_range(100.0..5000.0),
                            vol_xetra: rng.random_range(1000.0..50000.0),
                            vol_lsx: rng.random_range(500.0..20000.0),
                            vol_gettex: rng.random_range(100.0..10000.0),
                            vwap: base_price + rng.random_range(-1.0..1.0),
                            day_high: base_price + rng.random_range(1.0..5.0),
                            day_low: base_price - rng.random_range(1.0..5.0),
                            ytd_perf: rng.random_range(-15.0..25.0),
                            moving_avg: base_price + rng.random_range(-2.0..2.0),
                            rsi: rng.random_range(20.0..80.0),
                            macd: rng.random_range(-0.5..0.5),
                            bollinger: rng.random_range(-1.0..1.0),
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
                // 1. Create a quick scope to get the random symbol and drop `rng`
                let alert = {
                    let mut rng = rand::rng();
                    // Pick a random number between 0 and 999
                    let random_index = rng.random_range(0..1000);
                    let random_symbol = &symbols[random_index];
                    
                    WsMessage::Risk(RiskAlert {
                        level: "CRITICAL".to_string(),
                        // Inject the random symbol into the message
                        message: format!("Exposure limit breached on {}!", random_symbol),
                    })
                }; // `rng` is destroyed right here!
                
                let json = serde_json::to_string(&alert).unwrap();
                
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
                // Also print the specific symbol to the backend terminal
                if let WsMessage::Risk(risk_alert) = &alert {
                    println!("Fired Risk Alert: {}", risk_alert.message);
                }
            }
        }
    }
}