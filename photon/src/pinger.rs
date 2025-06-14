use std::net::{SocketAddr, UdpSocket};
use std::time::Instant;
use dns_lookup::lookup_host;
use websocket::url::Url;
use rand::{thread_rng, Rng};
use crate::photon_region::PhotonRegion;

pub struct Pinger {
    endpoint: SocketAddr,
    ping_bytes: [u8; 13],
    name: String
}

impl Pinger {
    pub fn new(photon_region: &PhotonRegion) -> Self {
        let url = Url::parse(&*photon_region.address).unwrap();
        let host = url.host_str().unwrap();
        let ips = lookup_host(host).unwrap();
        
        Pinger {
            endpoint: SocketAddr::new(ips[0], 5055),
            ping_bytes: [0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x00],
            name: photon_region.short_name.clone()
        }
    }
    
    fn gen_random_cur_id() -> u8 {
        thread_rng().gen_range(0, 255)
    }
    
    fn ping(&self, id: u8, socket: &UdpSocket) -> u128 {
        let mut temp_ping_bytes = self.ping_bytes.clone();
        temp_ping_bytes[12] = id;
        
        let start_time = Instant::now();
        socket.send(&temp_ping_bytes).unwrap();
        socket.recv(&mut temp_ping_bytes).unwrap();
        
        if id != temp_ping_bytes[12] {
            panic!("{}: cur_id mismatch", self.name);
        }
        
        start_time.elapsed().as_millis()
    }
    
    pub fn start_ping(&self, sample_size: i32) -> u128 {
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.connect(&self.endpoint).unwrap();
        
        let mut samples: Vec<u128> = Vec::with_capacity(sample_size as usize);
        for _ in 0..sample_size {
            let random_id = Pinger::gen_random_cur_id();
            samples.push(self.ping(random_id, &socket));
        }
        
        let avg = samples.iter().sum::<u128>() / sample_size as u128;
        avg
    }
}