mod models;
mod photon_ping;
mod background_pinger;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::thread;
use websocket::{ClientBuilder, OwnedMessage};
use crate::models::{RegionPingData, get_ping_data_json};
use crate::background_pinger::start_background_pinger;

fn main() {
    let mut client = ClientBuilder::new("ws://127.0.0.1:8080")
        .unwrap()
        .connect_insecure()
        .unwrap();

    let ping_data = Arc::new(Mutex::new(HashMap::new()));

    start_background_pinger(Arc::clone(&ping_data));

    let (mut receiver, mut sender) = client.split().unwrap();

    for message in receiver.incoming_messages() {
        let unwrapped_msg = message.unwrap();
        match unwrapped_msg {
            OwnedMessage::Ping(ping) => {
                if ping.len() == 32 {
                    let mut timestamp_bytes = [0; 16];
                    let mut rtt_bytes = [0; 16];
                    timestamp_bytes.copy_from_slice(&ping[0..16]);
                    rtt_bytes.copy_from_slice(&ping[16..32]);

                    let sent_timestamp = u128::from_be_bytes(timestamp_bytes);
                    let rtt = u128::from_be_bytes(rtt_bytes);

                    let current_timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
                    let time_difference = current_timestamp - sent_timestamp;

                    // Log only if needed for debugging
                    if cfg!(debug_assertions) {
                        println!("Ping - Server timestamp: {}, RTT: {}ms, Time diff: {}ms", 
                                 sent_timestamp, rtt, time_difference);
                    }
                }
                sender.send_message(&OwnedMessage::Pong(ping));
            }
            OwnedMessage::Text(text) => {
                println!("Received text message: {}", text);

                // Check if this is a play message with a future timestamp
                if text.starts_with("PLAY:") {
                    let parts: Vec<&str> = text.splitn(3, ':').collect();
                    if parts.len() >= 3 {
                        if let Ok(target_timestamp) = parts[1].parse::<u128>() {
                            let message_content = parts[2];

                            // Get current timestamp
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .expect("Time went backwards")
                                .as_millis();

                            if target_timestamp > now {
                                // Calculate how long to wait
                                let wait_time = target_timestamp - now;
                                println!("Received play message: '{}' to be played at timestamp {}. Current time: {}. Waiting for {} ms", 
                                         message_content, target_timestamp, now, wait_time);

                                // Wait until the specified timestamp
                                thread::sleep(Duration::from_millis(wait_time as u64));

                                // Play the message
                                println!("PLAYING NOW: {}", message_content);
                                // Here you would trigger the actual playback
                            } else {
                                // The timestamp is in the past, play immediately
                                println!("PLAYING IMMEDIATELY (timestamp already passed): {}", message_content);
                                // Here you would trigger the actual playback
                            }
                        } else {
                            println!("Invalid timestamp format in play message: {}", text);
                        }
                    } else {
                        println!("Invalid play message format: {}", text);
                    }
                }
                else if text.starts_with("REQUEST_PING:") {
                    // Check if a specific region is requested
                    let parts: Vec<&str> = text.splitn(2, ':').collect();
                    let target_region = if parts.len() > 1 && !parts[1].is_empty() {
                        parts[1]
                    } else {
                        // If no region specified, use all regions
                        ""
                    };

                    let json_data = get_ping_data_json(&ping_data, target_region);
                    sender.send_message(&OwnedMessage::Text(json_data));
                }
            }
            _ => {
                println!("Unknown Recv: {:?}", unwrapped_msg);
            }
        }

    }
}
