pub mod net {
    use sqlite::{self, Connection, Row, Statement};
    use std::{
        sync::{self, Arc},
        fs::metadata,
        collections::HashMap,
        error::Error,
        net::{IpAddr, Ipv4Addr},
        thread::{self, spawn, JoinHandle},
        time::{Duration, Instant},
    };
    use futures::{
        stream::{FuturesUnordered, StreamExt},
        future::join_all
    };

    use rand::random;
    use surge_ping::{Client, Config, IcmpPacket, PingIdentifier, PingSequence, ICMP};
    use tokio::time;

    pub struct Block {
        num: [u8;3],
        client: Option<Arc<surge_ping::Client>>,
        pub results: Option<Vec<(u8, bool)>>
    }

    impl Block {
        pub fn new(block: [u8;3]) -> Block {
            Block{
                num: block,
                client: Some(Arc::new(Client::new(&Config::default()).unwrap())),
                results: None,
            }
        }

        pub fn ip_from_u8(&self, byte: u8) -> IpAddr {
            IpAddr::V4( Ipv4Addr::from([self.num[0], self.num[1], self.num[2], byte]))
        }

        pub async fn process(&mut self) -> Result<(), Box<dyn Error>> {
            let spawn_time = Instant::now();
            let mut tasks = Vec::new();
            let mut client = self.client.take().unwrap();
            for byte in 0..=255 {
                let client_clone = client.clone();
                let ip = self.ip_from_u8(byte);
                tasks.push(
                    tokio::task::spawn(
                        ping(client_clone, ip)
                    )
                );
            }
            // println!("Took {:?}ms to spawn 255 tasks", spawn_time.elapsed().as_millis());
            let mut res = futures::future::join_all(tasks).await;
            self.results = Some(res.into_iter()
                .zip(0..=255)
                .map(|(v, k)| (k, v.unwrap()))
            .collect::<Vec<(u8, bool)>>());
            Ok(())
        }
    }

    pub async fn ping(client: Arc<surge_ping::Client>, addr: IpAddr) -> bool {
        let payload = [0; 56];
        let mut pinger = client.pinger(addr, PingIdentifier(random())).await;
        pinger.timeout(Duration::from_millis(300));
        let mut interval = tokio::time::interval(Duration::from_millis(300));
        let mut res = false;
        for idx in 0..1 {
            interval.tick().await;
            res = match pinger.ping(PingSequence(idx), &payload).await {
                Ok((IcmpPacket::V4(packet), dur)) => {println!(
                    "No.{}: {} bytes from {}: icmp_seq={} ttl={:?} time={:0.2?}",
                    idx,
                    packet.get_size(),
                    packet.get_source(),
                    packet.get_sequence(),
                    packet.get_ttl(),
                    dur
                ); 
                true},
                Ok((IcmpPacket::V6(packet), dur)) => {println!(
                    "No.{}: {} bytes from {}: icmp_seq={} hlim={} time={:0.2?}",
                    idx,
                    packet.get_size(),
                    packet.get_source(),
                    packet.get_sequence(),
                    packet.get_max_hop_limit(),
                    dur
                ); true},
                Err(_e) => {/*println!("No.{}: {} ping {}", idx, pinger.host, e); */false},
            };
        }
        res
    }
}