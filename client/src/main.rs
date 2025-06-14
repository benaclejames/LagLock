use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use websocket::{ClientBuilder, Message, OwnedMessage};
use websocket::header::q;
use photon;

type RegionPingData = HashMap<String, (u128, SystemTime)>;

async fn fetch_regions_once() -> Vec<photon::PhotonRegion> {
    println!("Fetching regions...");
    let regions = photon::get_regions_async();
    println!("Found {} regions. Pinging...", regions.len());
    regions
}

async fn ping_cached_regions(regions: &[photon::PhotonRegion]) -> Vec<(photon::PhotonRegion, u128)> {
    println!("Pinging {} cached regions...", regions.len());
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
            println!("Starting ping cycle...");

            let ping_results = rt.block_on(ping_cached_regions(&cached_regions));

            {
                let mut data = ping_data.lock().unwrap();
                let now = SystemTime::now();
                
                for (region, latency) in ping_results {
                    let region_name = format!("{:?}", region);
                    data.insert(region_name.clone(), (latency, now));
                    println!("{}: {:?}", region_name, latency);
                }
            }
            
            println!("Ping cycle finished.");
            thread::sleep(Duration::from_secs(30));
        }
    });
}

fn get_ping_data_json(ping_data: &Arc<Mutex<RegionPingData>>) -> String {
    let data = ping_data.lock().unwrap();
    let mut json = Vec::new();
    
    for (region, (latency, last_updated)) in data.iter() {
        let timestamp = last_updated.duration_since(UNIX_EPOCH).unwrap().as_millis();
        
        json.push(format!(r#"{{"region": "{}", "latency": {}, "last_updated": {}}}"#, region, latency, timestamp));
    }
    
    format!("[{}]", json.join(","))
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

                    println!("sent_timestamp: {}", sent_timestamp);
                    println!("rtt {}", rtt);
                    println!("time_diff: {}", time_difference);
                }
                println!("Ping: {:?}", ping);
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
                else if text.starts_with("GET_PING_DATA:") {
                    let json_data = get_ping_data_json(&ping_data);
                    sender.send_message(&OwnedMessage::Text(json_data));
                }
            }
            _ => {
                println!("Unknown Recv: {:?}", unwrapped_msg);
            }
        }

    }
}
