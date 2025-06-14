use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhotonRegion {
    pub short_name: String,
    pub address: String,
}