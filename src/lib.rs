/*
Structure of database (relevant for following module):
The database has one table "blocks" which holds all unchecked triplets of
big-end octets in column "block".
When a block is checked, a table is created whose name is the integer
corresponding to those octets.
Each of these tables has columns "ip" and "result".
After this table is inserted into the database, the corresponding entry in
"blocks" is deleted. The job is finished when "blocks" contains no entries.
*/

pub mod imap {
    use ping::dgramsock::ping;
    use sqlite::{Connection, Row, Statement};
    use std::{
        collections::VecDeque,
        error::Error,
        net::{IpAddr, Ipv4Addr},
        thread::{self, spawn, JoinHandle},
        time::Duration,
    };
const BLOCK_SIZE: u32 = 0x100;

    pub fn ping_u32(address: u32) -> bool {
        let bytes = address.to_be_bytes();
        let ip_v4 = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);
        let ip = IpAddr::V4(ip_v4);
        let result = ping(
            ip,
            Some(Duration::new(1, 0)),
            None,
            None, // these nones get turned into default values
            None,
            None,
        );

        if let result = Ok()match result {
            Ok(()) => return true,
            _ => return false,
        };
    }

    pub async fn ping_block(ip_block: u32) -> Vec<(u32, bool)> {
        let bytes = ip_block.to_be_bytes();
        let mut results: Vec<(u32, bool)> = Vec::new();
        for little_end in 0..=255 as u8 {
            let address = u32::from_be_bytes([
                bytes[0], 
                bytes[1], 
                bytes[2], 
                little_end
            ]);
            let result = ping_u32(address);
            results.push((address, result));
        }
        results
    }

    // Stores the results of a block scan in the database. The operations are
    // ordered to minimize the harmful effects of any errors.
    pub fn store_block_results(
        ip_block: u32,
        result_vec: Vec<(u32, bool)>,
        db: &sqlite::Connection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Storing results of block {:?}...", ip_block.to_be_bytes());
        let tracker_reset = format!("DELETE * FROM blocks WHERE block = {:?};", ip_block);
        db.execute(tracker_reset); // delete entry indicating block is complete

        let results_reset = format!("DROP TABLE block_{:?};", ip_block);
        db.execute(results_reset); // reset block before insertion for safety

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
        db.execute(progress_set)?; // only record block if entry succeeds

        Ok(())
    }

    // Query database to find completed block of highest address.
    pub fn get_job(db: &sqlite::Connection) -> u32 {
        let query = format!("SELECT *, MAX(block) FROM blocks;");
        let mut query_result = db.prepare(query).unwrap();
        let mut max_u32: u32 = 0x00;
        while let Ok(sqlite::State::Row) = query_result.next(){

            max_u32 = query_result.read::<i64, _>("block").unwrap() as u32;
        };
        let bytes = max_u32.to_be_bytes();
        println!("Resuming scan from block {}.{}.{}.x", bytes[0], bytes[1], bytes[2]);
        max_u32
    }

    // Checks head of database for missing jobs. This is an inexpensive
    // solution to the alternative of checking all generated addresses.
    pub fn check_jobs_head(db: &sqlite::Connection, num: u32) -> Option<Vec<u32>> {
        let mut completed: VecDeque<u32> = VecDeque::new(); // redo this with queues
        let query = format!("SELECT * FROM blocks ORDER BY block DESC LIMIT {:?};", num);
        for row in db
            .prepare(query)
            .unwrap()
            .into_iter()
            .map(|row| row.unwrap())
        {
            completed.push_front(row.read::<i64, _>("block") as u32);
        }
        if completed.len() == 0{
            return None
        }
        println!("{:?}", completed);
        let mut result = Vec::new();
        let mut min = *completed.front().expect("No completed scans");
        for num in completed {
            println!("Comparing {:#?} to {:#?} to determine gaps in jobs", min, num);
            if min != num {
                while min <= num {
                    println!("{:#?}, {:#?}", min, num);
                    result.push(min);
                    min += BLOCK_SIZE;
                }
            }
            min += BLOCK_SIZE;
        }
        Some(result)
    }

    pub fn assign_job(
        curr_block: u32, 
        threads: &mut Vec<JoinHandle<(u32, Vec<(u32, bool)>)>>
    ) -> Result<(), Box<dyn Error>> {
        println!("Assigning job...");
        let thread_block = curr_block + BLOCK_SIZE;
        let thread_handle = thread::spawn(move || {
            let bytes = thread_block.to_be_bytes();
            println!("Assigning block {}.{}.{}.x", bytes[0], bytes[1], bytes[2]);
            let res = ping_block(thread_block);
            return (thread_block, res);
        });
        threads.push(thread_handle);
        Ok(())
    }

    pub fn clean_up_job(
        idx: usize, 
        thread: JoinHandle<(u32, Vec<(u32, bool)>)>,
        db: &sqlite::Connection,
    ) -> Result<(), Box<dyn Error>> {
        let thread_result = thread.join().unwrap();
        let bytes = &thread_result.0.to_be_bytes();
        println!("Cleaning up job {}.{}.{}.x", bytes[0], bytes[1], bytes[2]);
        store_block_results(thread_result.0, thread_result.1, db)?;
        Ok(())
    }

    pub fn close_remaining_threads(
        threads: &mut Vec<JoinHandle<(u32, Vec<(u32, bool)>)>>,
        db: &sqlite::Connection) {
        while threads.len() > 0 {
            for idx in 0..threads.len() {
                if idx < threads.len() {
                    clean_up_job(idx, threads.remove(idx), &db);
                }
            }
        }
    }

    pub fn test_connection() -> Result<(), Box<dyn Error>> {
        let mut stream = std::net::TcpStream::connect("8.8.8.8:80")?;
        Ok(())
    }
}
