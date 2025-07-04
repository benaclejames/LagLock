mod models;
mod client_data;
mod message_handler;

use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use websocket::sync::Server;
use websocket::OwnedMessage;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use crate::models::{ClientsRegistry, DEFAULT_PHOTON_TARGET_REGION, PhotonPingsResponse};
use crate::message_handler::{send_play_message_to_all, request_photon_pings_from_all};
use crate::client_data::{ClientData, ClientDataExt};

fn main() {
    // Create a WebSocket server that will listen on 127.0.0.1:8080
    let server = Server::bind("127.0.0.1:8080").unwrap();

    // Create a shared registry for all connected clients
    let clients: ClientsRegistry = Arc::new(Mutex::new(HashMap::new()));

    println!("WebSocket server started on 127.0.0.1:8080");

    // Listen for connections
    for connection in server.filter_map(Result::ok) {
        // Clone the clients registry for this thread
        let thread_clients = clients.clone();

        // Spawn a new thread for each connection
        thread::spawn(move || {
            // Accept the connection
            if let Ok(websocket_client) = connection.accept() {
                println!("Client connected");

                // Get the client's IP address
                let ip = websocket_client.peer_addr().unwrap();
                println!("Connection from: {}", ip);

                // Set the client to non-blocking mode
                let _ = websocket_client.set_nonblocking(true);

                // Create a ClientData instance
                let client_data = ClientData::new(websocket_client);

                // Wrap client_data in Arc<Mutex<>> for thread-safe sharing
                let client_data = Arc::new(Mutex::new(client_data));

                // Add the client to the registry
                {
                    let mut locked_clients = thread_clients.lock().unwrap();
                    locked_clients.insert(ip, client_data.clone());
                    println!("Added client {} to registry. Total clients: {}", ip, locked_clients.len());
                }

                // Clone for ping thread
                let ping_client_data = client_data.clone();

                // Spawn a thread to send ping messages every 2 seconds
                thread::spawn(move || {
                    loop {
                        // Sleep for 2 seconds
                        thread::sleep(Duration::from_secs(2));

                        // Try to acquire lock and send ping
                        if let Ok(mut locked_client_data) = ping_client_data.lock() {
                            // Get current timestamp in milliseconds
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .expect("Time went backwards")
                                .as_millis();

                            // Convert timestamp to bytes
                            let timestamp_bytes = now.to_be_bytes().to_vec();
                            let cur_ping_bytes = locked_client_data.smoothed_ping.unwrap_or(0).to_be_bytes().to_vec();

                            // Send message with timestamp and estimated ping
                            let message = OwnedMessage::Ping(vec![timestamp_bytes, cur_ping_bytes].concat());

                            if cfg!(debug_assertions) {
                                println!("Sending ping to client {} with timestamp {}", ip, now);
                            }
                            if let Err(e) = locked_client_data.client.send_message(&message) {
                                println!("Error sending ping to client {}: {:?}", ip, e);
                                break;
                            }
                        } else {
                            // If we can't acquire the lock, the client might be disconnected
                            println!("Could not acquire lock for client {}, possibly disconnected", ip);
                            break;
                        }
                    }
                });

                // Loop to handle messages
                loop {
                    // Try to acquire lock
                    let mut message_option = None;
                    let mut should_break = false;

                    {
                        // Try to acquire lock
                        match client_data.lock() {
                            Ok(mut locked_client_data) => {
                                // Try to read a message from the client
                                match locked_client_data.client.recv_message() {
                                    Ok(msg) => message_option = Some(msg),
                                    Err(e) => {
                                        // If it's an IO error, it might be because no message is available
                                        // in non-blocking mode, so we'll just continue
                                        match e {
                                            websocket::WebSocketError::IoError(_) => {
                                                // No message available, we'll continue the loop
                                            },
                                            _ => {
                                                println!("Error receiving message: {:?}", e);
                                                should_break = true;
                                            }
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                println!("Error acquiring lock: {:?}", e);
                                should_break = true;
                            }
                        }
                    };

                    if should_break {
                        // Remove client from registry before breaking
                        {
                            let mut locked_clients = thread_clients.lock().unwrap();
                            locked_clients.remove(&ip);
                            println!("Removed client {} from registry due to error. Total clients: {}", ip, locked_clients.len());
                        }
                        break;
                    }

                    // If no message was received, sleep a short time and try again
                    if message_option.is_none() {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }

                    // Process the message
                    let message = message_option.unwrap();
                    match message {
                        OwnedMessage::Close(_) => {
                            // Client wants to close connection
                            if let Ok(mut locked_client_data) = client_data.lock() {
                                let _ = locked_client_data.client.send_message(&OwnedMessage::Close(None));
                            }

                            // Remove client from registry
                            {
                                let mut locked_clients = thread_clients.lock().unwrap();
                                locked_clients.remove(&ip);
                                println!("Removed client {} from registry due to close message. Total clients: {}", ip, locked_clients.len());
                            }

                            println!("Client {} disconnected", ip);
                            break;
                        }
                        OwnedMessage::Ping(ping) => {
                            // Respond to ping with pong
                            if let Ok(mut locked_client_data) = client_data.lock() {
                                let _ = locked_client_data.client.send_message(&OwnedMessage::Pong(ping));
                            }
                        }
                        OwnedMessage::Text(text) => {
                            println!("Received message from {}: {}", ip, text);

                            // Check if this is a command to send a play message to all clients
                            if text.starts_with("SEND_PLAY:") {
                                // Check if format is SEND_PLAY:message or SEND_PLAY:region:message
                                let parts: Vec<&str> = text.trim_start_matches("SEND_PLAY:").splitn(2, ':').collect();

                                let (target_region, message_content) = if parts.len() > 1 {
                                    // Format is SEND_PLAY:region:message
                                    (parts[0], parts[1])
                                } else {
                                    // Format is SEND_PLAY:message, use default region
                                    (DEFAULT_PHOTON_TARGET_REGION, parts[0])
                                };

                                println!("Received command to send play message: {} for region: {}", message_content, target_region);

                                // Send play message to all clients
                                send_play_message_to_all(&thread_clients, message_content, target_region);

                                // Also send confirmation to the client that sent the command
                                if let Ok(mut locked_client_data) = client_data.lock() {
                                    let _ = locked_client_data.client.send_message(&OwnedMessage::Text(
                                        format!("Play message '{}' sent to all clients", message_content)
                                    ));
                                }
                            } else if text.starts_with("REQUEST_PING:") {
                                // Check if a specific region is requested
                                let parts: Vec<&str> = text.splitn(2, ':').collect();
                                let target_region = if parts.len() > 1 && !parts[1].is_empty() {
                                    parts[1]
                                } else {
                                    DEFAULT_PHOTON_TARGET_REGION
                                };

                                if cfg!(debug_assertions) {
                                    println!("Received command to request photon pings from all clients for region {}", target_region);
                                }
                                request_photon_pings_from_all(&thread_clients, target_region);
                            } else if text.starts_with("PHOTON_PINGS:") {
                                let json_content = text.trim_start_matches("PHOTON_PINGS:");
                                match serde_json::from_str::<PhotonPingsResponse>(json_content) {
                                    Ok(response) => {
                                        if cfg!(debug_assertions) {
                                            println!("Received photon pings from client {}", ip);
                                            println!("Number of regions: {}", response.regions.len());

                                            // Process the ping data as needed
                                            for region_info in &response.regions {
                                                println!("Region: {}, Latency: {}ms, Last updated: {}", 
                                                         region_info.region, 
                                                         region_info.latency,
                                                         region_info.last_updated);
                                            }
                                        }

                                        // Store the ping data for later use
                                        if let Ok(mut locked_client_data) = client_data.lock() {
                                            // Store the photon ping data
                                            locked_client_data.photon_pings = Some(response.regions);
                                            // Mark that we're no longer waiting for photon pings
                                            locked_client_data.waiting_for_photon_pings = false;

                                            // Acknowledge receipt
                                            let _ = locked_client_data.client.send_message(&OwnedMessage::Text(
                                                "Photon ping data received".to_string()
                                            ));
                                        }
                                    },
                                    Err(e) => {
                                        println!("Error parsing photon ping data from client {}: {:?}", ip, e);
                                    }
                                }
                            } else {
                                // Echo text messages back to the client
                                if let Ok(mut locked_client_data) = client_data.lock() {
                                    let _ = locked_client_data.client.send_message(&OwnedMessage::Text(format!("Echo: {}", text)));
                                }
                            }
                        }
                        OwnedMessage::Binary(data) => {
                            // Echo binary messages back to the client
                            println!("Received binary data from {}: {} bytes", ip, data.len());
                            if let Ok(mut locked_client_data) = client_data.lock() {
                                let _ = locked_client_data.client.send_message(&OwnedMessage::Binary(data));
                            }
                        }
                        OwnedMessage::Pong(data) => {
                            // Calculate round-trip latency
                            if data.len() == 32 {  // 2 x u128 is 32 bytes
                                // Extract timestamp from pong data
                                let mut timestamp_bytes = [0u8; 16];
                                timestamp_bytes.copy_from_slice(&data[..16]);
                                let sent_time = u128::from_be_bytes(timestamp_bytes);

                                // Normally we might care about estimated ping here but for now w/e

                                // Get current time
                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .expect("Time went backwards")
                                    .as_millis();

                                // Calculate latency
                                let latency = now - sent_time;

                                // Store the ping data and update smoothed ping
                                if let Ok(mut locked_client_data) = client_data.lock() {
                                    locked_client_data.add_ping(now, latency);
                                }

                                if cfg!(debug_assertions) {
                                    println!("Round-trip latency for client {}: {} ms", ip, latency);
                                }
                            } else {
                                println!("Received pong with invalid data format from {}", ip);
                            }
                        }
                    }
                }
            }
        });
    }
}
