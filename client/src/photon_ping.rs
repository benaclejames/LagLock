use photon;

pub async fn fetch_regions_once() -> Vec<photon::PhotonRegion> {
    if cfg!(debug_assertions) {
        println!("Fetching regions...");
    }
    let regions = photon::get_regions_async();
    if cfg!(debug_assertions) {
        println!("Found {} regions", regions.len());
    }
    regions
}

pub async fn ping_cached_regions(regions: &[photon::PhotonRegion]) -> Vec<(photon::PhotonRegion, u128)> {
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