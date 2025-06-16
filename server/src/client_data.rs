use std::time::{SystemTime, UNIX_EPOCH};
use websocket::sync::Client;
use std::net::TcpStream;
use std::collections::VecDeque;

pub use crate::models::ClientData;

// Extension trait for ClientData
pub trait ClientDataExt {
    fn new(client: Client<TcpStream>) -> ClientData;
    fn add_ping(&mut self, timestamp: u128, latency: u128);
    fn update_smoothed_ping(&mut self);
}

impl ClientDataExt for ClientData {
    // Create a new ClientData instance
    fn new(client: Client<TcpStream>) -> ClientData {
        ClientData {
            client,
            ping_history: VecDeque::new(),
            smoothed_ping: None,
            photon_pings: None,
            waiting_for_photon_pings: false,
        }
    }

    // Add a new ping latency to the history and update the smoothed ping
    fn add_ping(&mut self, timestamp: u128, latency: u128) {
        // Add the new ping to the history
        self.ping_history.push_back((timestamp, latency));

        // Calculate the smoothed ping (average of pings in the last 30 seconds)
        self.update_smoothed_ping();

        // Log the current ping and smoothed ping if in debug mode
        if cfg!(debug_assertions) {
            println!("Current ping: {} ms, Smoothed ping: {} ms", 
                     latency, 
                     self.smoothed_ping.unwrap_or(0));
        }
    }

    // Update the smoothed ping based on the ping history
    fn update_smoothed_ping(&mut self) {
        // Get the current time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        // Keep only pings from the last 30 seconds
        let thirty_seconds_ago = now - 30_000;
        while let Some((timestamp, _)) = self.ping_history.front() {
            if *timestamp < thirty_seconds_ago {
                self.ping_history.pop_front();
            } else {
                break;
            }
        }

        // Calculate the average ping if we have any data
        if !self.ping_history.is_empty() {
            let sum: u128 = self.ping_history.iter().map(|(_, latency)| latency).sum();
            self.smoothed_ping = Some(sum / self.ping_history.len() as u128);
        } else {
            self.smoothed_ping = None;
        }
    }
}
