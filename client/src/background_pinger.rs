use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, Duration};
use crate::models::RegionPingData;
use crate::photon_ping::{fetch_regions_once, ping_cached_regions};

pub fn start_background_pinger(ping_data: Arc<Mutex<RegionPingData>>) {
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