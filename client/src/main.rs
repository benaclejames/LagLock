use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use websocket::{ClientBuilder, OwnedMessage};
use serde::{Serialize, Deserialize};
use photon;

type RegionPingData = HashMap<String, (u128, SystemTime)>;

#[derive(Serialize, Deserialize)]
struct RegionPingInfo {
    region: String,
    latency: u128,
    last_updated: u128,
}

#[derive(Serialize, Deserialize)]
struct PhotonPingsResponse {
    regions: Vec<RegionPingInfo>,
}

async fn fetch_regions_once() -> Vec<photon::PhotonRegion> {
    if cfg!(debug_assertions) {
        println!("Fetching regions...");
    }
    let regions = photon::get_regions_async();
    if cfg!(debug_assertions) {
        println!("Found {} regions", regions.len());
    }
    regions
}

async fn ping_cached_regions(regions: &[photon::PhotonRegion]) -> Vec<(photon::PhotonRegion, u128)> {
    if cfg!(debug_assertions) {
        println!("Pinging {} regions...", regions.len());
    }
    let handles: Vec<_> = regions
        .iter()
        .cloned()
        .map(|region| {
            tokio::spawn(async move {
                let pinger = photon::Pinger::new(&region);
                let latency = pinger.start_ping(20);
                (region, latency)
            })
        })
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(pair) => results.push(pair),
            Err(e) => eprintln!("A task panicked: {e:?}"),
        }
    }

    results
}

fn start_background_pinger(ping_data: Arc<Mutex<RegionPingData>>) {
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let cached_regions = rt.block_on(fetch_regions_once());

        loop {
            if cfg!(debug_assertions) {
                println!("Starting ping cycle...");
            }

            let ping_results = rt.block_on(ping_cached_regions(&cached_regions));

            {
                let mut data = ping_data.lock().unwrap();
                let now = SystemTime::now();

                for (region, latency) in ping_results {
                    let region_name = region.short_name.clone();
                    data.insert(region_name.clone(), (latency, now));

                    // Log only if needed for debugging
                    if cfg!(debug_assertions) {
                        println!("Region {}: {}ms", region_name, latency);
                    }
                }
            }

            if cfg!(debug_assertions) {
                println!("Ping cycle finished.");
            }
            thread::sleep(Duration::from_secs(30));
        }
    });
}

fn get_ping_data_json(ping_data: &Arc<Mutex<RegionPingData>>, target_region: &str) -> String {
    let data = ping_data.lock().unwrap();
    let mut regions = Vec::new();

    for (region, (latency, last_updated)) in data.iter() {
        // If a target region is specified, only include data for that region
        if target_region.is_empty() || region == target_region {
            let timestamp = last_updated.duration_since(UNIX_EPOCH).unwrap().as_millis();

            regions.push(RegionPingInfo {
                region: region.clone(),
                latency: *latency,
                last_updated: timestamp,
            });
        }
    }

    let response = PhotonPingsResponse { regions };
    format!("PHOTON_PINGS:{}", serde_json::to_string(&response).unwrap())
}

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
