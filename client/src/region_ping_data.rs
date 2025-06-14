use std::time::SystemTime;
use photon::PhotonRegion;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionPingData {
    pub region: PhotonRegion,
    pub rtt: u128,
    pub last_updated: SystemTime
}