use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

// Type alias for storing region ping data
pub type RegionPingData = HashMap<String, (u128, SystemTime)>;

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

pub fn get_ping_data_json(ping_data: &std::sync::Arc<std::sync::Mutex<RegionPingData>>, target_region: &str) -> String {
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