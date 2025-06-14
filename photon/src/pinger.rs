use std::net::{SocketAddr, UdpSocket};
use std::time::Instant;
use dns_lookup::lookup_host;
use websocket::url::Url;
use rand::{thread_rng, Rng};

pub(crate) struct Pinger {
    endpoint: SocketAddr,
    ping_bytes: [u8; 13],
    cur_id: u8,
    name: String
}

impl Pinger {
    pub fn new(host: &String, port: u16, name: &String) -> Self {
        let url = Url::parse(host).unwrap();
        let host = url.host_str().unwrap();
        let ips = lookup_host(host).unwrap();
        
        Pinger {
            endpoint: SocketAddr::new(ips[0], port),
            ping_bytes: [0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x7d, 0x00],
            cur_id: 0,
            name: name.clone()
        }
    }
    
    fn gen_random_cur_id(&mut self) {
        self.cur_id = thread_rng().gen_range(0, 255);
        self.ping_bytes[12] = self.cur_id;
    }
    
    fn ping(&mut self, socket: &UdpSocket) -> u128 {
        self.gen_random_cur_id();
        
        let start_time = Instant::now();
        socket.send(&self.ping_bytes).unwrap();
        socket.recv(&mut self.ping_bytes).unwrap();
        
        if self.cur_id != self.ping_bytes[12] {
            panic!("{}: cur_id mismatch", self.name);
        }
        
        start_time.elapsed().as_millis()
    }
    
    pub fn start_ping(&mut self, sample_size: i32) -> u128 {
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.connect(&self.endpoint).unwrap();
        
        let mut samples: Vec<u128> = Vec::with_capacity(sample_size as usize);
        for _ in 0..sample_size {
            self.gen_random_cur_id();
            samples.push(self.ping(&socket));
        }
        
        let avg = samples.iter().sum::<u128>() / sample_size as u128;
        avg
    }
}