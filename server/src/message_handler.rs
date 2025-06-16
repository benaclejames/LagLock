use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use websocket::OwnedMessage;
use crate::models::{ClientsRegistry, DEFAULT_PHOTON_TARGET_REGION};

// Function to get the highest RTT among all connected clients
pub fn get_highest_rtt(clients: &ClientsRegistry, target_region: &str) -> u128 {
    let locked_clients = clients.lock().unwrap();
    let mut highest_server_ping = 0;
    let mut highest_photon_ping = 0;

    for (_, client_data) in locked_clients.iter() {
        if let Ok(locked_client) = client_data.lock() {
            // Check server-client ping
            if let Some(rtt) = locked_client.smoothed_ping {
                highest_server_ping = highest_server_ping.max(rtt);
            }

            // Check photon pings if available
            if let Some(photon_pings) = &locked_client.photon_pings {
                for ping_info in photon_pings {
                    // Only consider pings for the target region
                    if ping_info.region == target_region {
                        highest_photon_ping = highest_photon_ping.max(ping_info.latency);
                    }
                }
            }
        }
    }

    // Return the sum of the highest server ping and the highest photon ping
    highest_server_ping + highest_photon_ping
}

// Function to send a play message to all clients with a future timestamp
pub fn send_play_message_to_all(clients: &ClientsRegistry, message: &str, target_region: &str) {
    // First, request photon pings from all clients for the target region
    println!("Requesting photon pings from all clients for region {} before sending play message", target_region);
    request_photon_pings_from_all(clients, target_region);

    // Wait for all clients to respond with their photon pings (with a timeout)
    let max_wait_time = Duration::from_secs(2); // Maximum wait time of 2 seconds
    let start_time = SystemTime::now();

    let mut all_clients_responded = false;
    while !all_clients_responded && SystemTime::now().duration_since(start_time).unwrap() < max_wait_time {
        // Check if all clients have responded
        all_clients_responded = true;

        let locked_clients = clients.lock().unwrap();
        for (_, client_data) in locked_clients.iter() {
            if let Ok(locked_client) = client_data.lock() {
                if locked_client.waiting_for_photon_pings {
                    // At least one client is still waiting for photon pings
                    all_clients_responded = false;
                    break;
                }
            }
        }

        if !all_clients_responded {
            // Sleep a short time before checking again
            thread::sleep(Duration::from_millis(50));
        }
    }

    if !all_clients_responded {
        println!("Not all clients responded with photon pings within the timeout period");
    } else {
        println!("All clients responded with photon pings");
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    // Get the highest RTT among all clients (sum of highest server ping and highest photon ping for the target region)
    let highest_rtt = get_highest_rtt(clients, target_region);
    println!("Highest RTT (sum of highest server ping and highest photon ping for region {}): {} ms", target_region, highest_rtt);

    // Calculate a future timestamp that gives all clients enough time to receive and process the message
    // We multiply by 1.5 to add some buffer
    let future_timestamp = now + (highest_rtt * 3 / 2);

    // Create the play message with the future timestamp
    let play_message = format!("PLAY:{}:{}:{}", future_timestamp, message, highest_rtt);

    // Send the message to all clients
    let locked_clients = clients.lock().unwrap();
    for (addr, client_data) in locked_clients.iter() {
        if let Ok(mut locked_client) = client_data.lock() {
            match locked_client.client.send_message(&OwnedMessage::Text(play_message.clone())) {
                Ok(_) => println!("Sent play message to client {}", addr),
                Err(e) => println!("Error sending play message to client {}: {:?}", addr, e),
            }
        }
    }

    println!("Sent play message to all clients with future timestamp: {}", future_timestamp);
}

pub fn request_photon_pings_from_all(clients: &ClientsRegistry, target_region: &str) {
    for (addr, client_data) in clients.lock().unwrap().iter() {
        if let Ok(mut locked_client) = client_data.lock() {
            // Mark that we're waiting for photon pings from this client
            locked_client.waiting_for_photon_pings = true;
            // Clear any previous photon ping data
            locked_client.photon_pings = None;

            match locked_client.client.send_message(&OwnedMessage::Text(format!("REQUEST_PING:{}", target_region))) {
                Ok(_) => {
                    if cfg!(debug_assertions) {
                        println!("Sent ping message to client {} for region {}", addr, target_region);
                    }
                },
                Err(e) => println!("Error sending ping message to client {}: {:?}", addr, e),
            }
        }
    }
}