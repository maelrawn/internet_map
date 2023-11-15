/*
Structure of database (relevant for following module):
The database has one table "blocks" which holds all checked pairs of
big-end octets in column "block".
When a block is checked, a table is created whose name is the integer
corresponding to those octets.
Each of these tables has columns "ip" and "result". "ip" is the pair of octets
that make up the lower half of the ip, and "result" is a bool indicating the
response of the server.
After this table is inserted into the database, the corresponding entry in
"blocks" is created. The job is finished when "blocks" contains all entries
in the u16 space.
*/
pub mod imap {
    use sqlite::{self, Connection, Row, Statement};
    use std::{
        sync::{self, Arc},
        fs::metadata,
        collections::VecDeque,
        error::Error,
        net::{IpAddr, Ipv4Addr},
        thread::{self, spawn, JoinHandle},
        time::Duration,
    };
    use futures::future::join_all;
    use rand::random;
    use surge_ping::{Client, Config, IcmpPacket, PingIdentifier, PingSequence, ICMP};
    use tokio::time;
    const BLOCK_SIZE: u16 = 0x0001; //this works because this is the big end

    pub async fn batch_jobs(
        init_block: u16,
        range: u16, 
        db_path: String
    ) -> Result<Vec<(u16, Vec<(u16, bool)>)>, Box<dyn Error>> {
        let mut tasks = Vec::new();
        let range_iter = init_block..(init_block + range);
        for block in range_iter {
            println!("assigning job {:?}", block);
            let db_path_clone = db_path.clone();
            tasks.push(tokio::spawn(complete_job(block.clone(), db_path_clone)));
        }
        println!("awaiting tasks");
        let results = join_all(tasks).await;
        let results = results.into_iter()
            .zip(init_block..(init_block + range))
            .map(|(res, range)| 
                (range, res.unwrap()))
            .collect();
        Ok(results)
    }

    pub async fn complete_job(block: u16, db_path: String) -> Vec<(u16, bool)> {
        println!("awaiting pings on block {:?}", block);
        let db = sqlite::open(db_path).unwrap();
        let block_res = tokio::spawn(ping_block(block)).await.unwrap()
    }

    pub fn write_results(
        ip_block: u16,
        result_vec: Vec<(u16, bool)>,
        db: &sqlite::Connection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ip = ip_block.to_be_bytes();
        println!("Storing results of block {:?}.{:?}.x.x...", ip[0] ,ip[1]);
        db.execute("BEGIN TRANSACTION;");
        let tracker_reset = format!("DELETE * FROM blocks WHERE block = {:?};", ip_block);
        db.execute(tracker_reset);

        let results_reset = format!("DROP TABLE block_{:?};", ip_block);
        db.execute(results_reset);

        let create = format!("CREATE TABLE block_{:?} (ip INTEGER, res INTEGER);", ip_block);
        db.execute(create)?;
        for entry in result_vec {
            let insert = format!(
                "INSERT INTO block_{:?} VALUES ({:?}, {:?});",
                ip_block, entry.0, entry.1
            );
            db.execute(insert)?; // exit and propagate error if insert fails
        }

        let progress_set = format!("INSERT INTO blocks VALUES ({:?});", ip_block);
        db.execute(progress_set)?;
        db.execute("COMMIT;");
        Ok(())
    }

    pub async fn ping_block(block: u16) -> Vec<(u16, bool)> {
        println!("pinging block {:?}", block);
        let mut tasks = Vec::new();
        let range = 0..=65535;
        let client = Client::new(&Config::default()).unwrap();
        for le in range {
            tasks.push(tokio::spawn(ping(client.clone(), u16s_to_ipv4(block, le))));
        }
        println!("Pings issued");
        join_all(tasks).await
            .into_iter()
            .zip(0..=65535)
            .map(|(res, num)| (num, res.unwrap()))
            .collect()
    }

    pub async fn ping(client: surge_ping::Client, addr: IpAddr) -> bool {
        let payload = [0; 56];
        let mut pinger = client.pinger(addr, PingIdentifier(random())).await;
        pinger.timeout(Duration::from_secs(1));
        let mut interval = time::interval(Duration::from_secs(1));
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
                ); true},
                Ok((IcmpPacket::V6(packet), dur)) => {println!(
                    "No.{}: {} bytes from {}: icmp_seq={} hlim={} time={:0.2?}",
                    idx,
                    packet.get_size(),
                    packet.get_source(),
                    packet.get_sequence(),
                    packet.get_max_hop_limit(),
                    dur
                ); true},
                Err(e) => {println!("No.{}: {} ping {}", idx, pinger.host, e); false},
            };
        }
        res
    }

    pub fn u16s_to_ipv4(upper: u16, lower: u16) -> IpAddr {
        let uppers = upper.to_be_bytes();
        let lowers = lower.to_be_bytes();
        let ipv4 = Ipv4Addr::from([uppers[0], uppers[1], lowers[0], lowers[1]]);
        IpAddr::V4(ipv4)
    }

    pub fn create_if_not_exists(db_path: &String) -> Result<sqlite::Connection, sqlite::Error> {
        let db;
        match std::fs::metadata(db_path) {
            Ok(data) => { 
                println!("Opening database...");
                db = sqlite::open(db_path)?;
            }
            Err(_) => {
                println!("Creating database...");
                let flags = sqlite::OpenFlags::new().set_create().set_read_write();
                db = sqlite::open(db_path)?;
                let query = "CREATE TABLE blocks (block INTEGER);";
                db.execute(query);
                let index = "CREATE UNIQUE INDEX idx_block ON blocks (block);";
                db.execute(query);
            }
        }
        Ok(db)
    }

}

//     dead code to be pruned
//     pub async fn ping(client: Client, addr: IpAddr) -> (u16, bool) {
//         let payload = [0; 56];
//         let mut pinger = client.pinger(addr, PingIdentifier(0)).await;
//         pinger.timeout(Duration::from_secs(1));
//         let octets = match addr {
//             IpAddr::V4(ipv4) => ipv4.octets(),
//             IpAddr::V6(ipv6) => [0, 0, 0, 0],
//         };
//         let ip = u16::from_be_bytes([octets[2], octets[3]]);
//         match pinger.ping(PingSequence(0), &payload).await {
//             Ok((IcmpPacket::V4(packet), dur)) => (ip, true),
//             _ => (ip, false)
//         }
//     }

//     // TODO: Finish rewrite as async with surge-ping and tokio
//     pub async fn ping_block(block: u16) -> Vec<(u16, bool)> {
//         let bytes = block.to_be_bytes();
//         let mut tasks = Vec::new();
//         let config = surge_ping::Config::builder().ttl(1).build();
//         let client = surge_ping::Client::new(&config).expect("Failed to build client.");
//         for le in 0..=65535 as u16 {
//             let le_bytes = le.to_be_bytes();
//             let ipv4 = Ipv4Addr::from([bytes[0], bytes[1], le_bytes[0], le_bytes[1]]);
//             tasks.push(tokio::spawn(ping(client.clone(), IpAddr::V4(ipv4))));
//         }
//         let bools = futures::future::join_all(tasks).await;
//         bools.into_iter()
//             .map(|res| res.unwrap())
//             .collect()
//     }

//     pub async fn store_block_results(
//         ip_block: u16,
//         result_vec: Vec<(u16, bool)>,
//         db: &sqlite::Connection,
//     ) -> Result<(), Box<dyn std::error::Error>> {
//         let ip = ip_block.to_be_bytes();
//         println!("Storing results of block {:?}.{:?}.x.x...", ip[0] ,ip[1]);
//         db.execute("BEGIN TRANSACTION;");
//         let tracker_reset = format!("DELETE * FROM blocks WHERE block = {:?};", ip_block);
//         db.execute(tracker_reset);

//         let results_reset = format!("DROP TABLE block_{:?};", ip_block);
//         db.execute(results_reset);

//         let create = format!("CREATE TABLE block_{:?} (ip INTEGER, res INTEGER);", ip_block);
//         db.execute(create)?;
//         for entry in result_vec {
//             let insert = format!(
//                 "INSERT INTO block_{:?} VALUES ({:?}, {:?});",
//                 ip_block, entry.0, entry.1
//             );
//             db.execute(insert)?; // exit and propagate error if insert fails
//         }

//         let progress_set = format!("INSERT INTO blocks VALUES ({:?});", ip_block);
//         db.execute(progress_set)?;
//         db.execute("COMMIT;");
//         Ok(())
//     }

//     // Query database to find completed block of highest address.
//     // TODO: Restructure database and associated functions to take advantage
//     // of indexing on ip block
//     pub fn get_job(db: &sqlite::Connection) -> u16 {
//         let query = format!("SELECT *, MAX(block) FROM blocks;");
//         let mut query_result = db.prepare(query).unwrap();
//         let mut max_u16: u16 = 0x00;
//         while let Ok(sqlite::State::Row) = query_result.next(){
//             max_u16 = query_result.read::<i64, _>("block").unwrap() as u16;
//         };
//         let bytes = max_u16.to_be_bytes();
//         println!("Resuming scan from block {}.{}.x.x", bytes[0], bytes[1]);
//         max_u16
//     }

//     // Checks head of database for missing jobs. This is an inexpensive
//     // solution to the alternative of checking all generated addresses.

//     // TODO: Restructure database and associated functions to take advantage
//     // of indexing on ip block
//     pub fn check_jobs_head(db: &sqlite::Connection, num: u16) -> Option<Vec<u16>> {
//         let mut completed: VecDeque<u16> = VecDeque::new(); // redo this with queues
//         let query = format!("SELECT * FROM blocks ORDER BY block DESC LIMIT {:?};", num);
//         for row in db
//             .prepare(query)
//             .unwrap()
//             .into_iter()
//             .map(|row| row.unwrap())
//         {
//             completed.push_front(row.read::<i64, _>("block") as u16);
//         }
//         if completed.len() == 0{
//             return None
//         }
//         println!("{:?}", completed);
//         let mut result = Vec::new();
//         let mut min = *completed.front().expect("No completed scans");
//         for num in completed {
//             println!("Comparing {:#?} to {:#?} to determine gaps in jobs", min, num);
//             if min != num {
//                 while min <= num {
//                     println!("{:#?}, {:#?}", min, num);
//                     result.push(min);
//                     min += BLOCK_SIZE;
//                 }
//             }
//             min += BLOCK_SIZE;
//         }
//         Some(result)
//     }

//     // TODO: Restructure for better parallelism
//     pub async fn assign_job(
//         block: u16,
//         db: sqlite::Connection,
//         threads: &mut Vec<tokio::task::JoinHandle<(u16, Vec<(u16, bool)>)>>,
//     ) {
//         let thread = tokio::spawn(async {
//             store_block_results(block, ping_block(block).await, db);
//         });
//     }

//     // pub async fn clean_up_job(
//     //     block: u16,
//     //     thread_result: Vec<(u16, bool)>,
//     //     db: &sqlite::Connection,
//     // ) -> Result<(), Box<dyn Error>> {
//     //     let bytes = block.to_be_bytes();
//     //     println!("Cleaning up job {}.{}.x.x", bytes[0], bytes[1]);
//     //     store_block_results(block, thread_result, db).await;
//     //     Ok(())
//     // }

//     pub async fn close_remaining_threads(
//         threads: &mut Vec<JoinHandle<(u16, Vec<(u16, bool)>)>>,
//         db: &sqlite::Connection) {
//         while threads.len() > 0 {
//             for idx in 0..threads.len() {
//                 if idx < threads.len() {
//                     clean_up_job(idx, threads.remove(idx), &db);
//                 }
//             }
//         }
//     }

//     // TODO: make this function properly, or remove it
//     pub fn test_connection() -> Result<(), Box<dyn Error>> {
//         let mut stream = std::net::TcpStream::connect("8.8.8.8:80")?;
//         Ok(())
//     }
// }   
