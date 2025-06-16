mod models;
mod photon_ping;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::thread;
use websocket::{ClientBuilder, OwnedMessage};
use crate::models::{RegionPingData, get_ping_data_json};
use crate::photon_ping::{fetch_regions_once, ping_cached_regions};

fn main() {
    let mut client = ClientBuilder::new("ws://127.0.0.1:8080")
        .unwrap()
        .connect_insecure()
        .unwrap();

    let ping_data = Arc::new(Mutex::new(HashMap::new()));

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

                    if cfg!(debug_assertions) {
                        println!("Starting on-demand ping for region: {}", target_region);
                    }

                    // Create a tokio runtime for async operations
                    let rt = tokio::runtime::Runtime::new().unwrap();

                    // Fetch regions
                    let regions = rt.block_on(fetch_regions_once());

                    // Filter regions if a specific one is requested
                    let regions_to_ping = if !target_region.is_empty() {
                        regions.into_iter()
                            .filter(|r| r.short_name == target_region)
                            .collect::<Vec<_>>()
                    } else {
                        regions
                    };

                    if regions_to_ping.is_empty() {
                        println!("No matching regions found for: {}", target_region);
                        // Send an empty response
                        let json_data = get_ping_data_json(&ping_data, target_region);
                        sender.send_message(&OwnedMessage::Text(json_data));
                        continue;
                    }

                    // Ping the regions
                    let ping_results = rt.block_on(ping_cached_regions(&regions_to_ping));

                    // Update the ping data
                    {
                        let mut data = ping_data.lock().unwrap();
                        let now = SystemTime::now();

                        for (region, latency) in ping_results {
                            let region_name = region.short_name.clone();
                            data.insert(region_name.clone(), (latency, now));

                            if cfg!(debug_assertions) {
                                println!("Region {}: {}ms", region_name, latency);
                            }
                        }
                    }

                    // Send the ping data back to the server
                    let json_data = get_ping_data_json(&ping_data, target_region);
                    sender.send_message(&OwnedMessage::Text(json_data));

                    if cfg!(debug_assertions) {
                        println!("On-demand ping cycle finished.");
                    }
                }
            }
            _ => {
                println!("Unknown Recv: {:?}", unwrapped_msg);
            }
        }

    }
}
