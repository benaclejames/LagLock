use serde::{Serialize, Deserialize};
use std::collections::{VecDeque, HashMap};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use websocket::sync::Client;
use std::net::TcpStream;

#[derive(Serialize, Deserialize)]
pub struct RegionPingInfo {
    pub region: String,
    pub latency: u128,
    pub last_updated: u128,
}

#[derive(Serialize, Deserialize)]
pub struct PhotonPingsResponse {
    pub regions: Vec<RegionPingInfo>,
}

// Structure to store client data including ping history
pub struct ClientData {
    pub client: Client<TcpStream>,
    pub ping_history: VecDeque<(u128, u128)>,
    pub smoothed_ping: Option<u128>,
    pub photon_pings: Option<Vec<RegionPingInfo>>,
    pub waiting_for_photon_pings: bool,
}

pub type ClientsRegistry = Arc<Mutex<HashMap<SocketAddr, Arc<Mutex<ClientData>>>>>;

pub const DEFAULT_PHOTON_TARGET_REGION: &str = "us";