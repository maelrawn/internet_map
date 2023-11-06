use internet_map::imap;
use std::{
    fs::metadata,
    error::Error,
    thread::{self, available_parallelism, JoinHandle},
};

const BLOCK_SIZE: u32 = 0x100;

fn main() -> Result<(), Box<dyn Error>> {
    let db;
    match std::fs::metadata("internet.db") {
        Ok(data) => { 
            println!("Opening database...");
            db = sqlite::open("internet.db")?;
        }
        Err(_) => {
            println!("Creating database...");
            let flags = sqlite::OpenFlags::new().set_create().set_read_write();
            db = sqlite::open("internet.db")?;
            let query = "CREATE TABLE blocks (block INTEGER);";
            db.execute(query);
        }
    };

    // println!("Testing internet connectivity...");
    // imap::test_connection()?;

    let mut threads: Vec<JoinHandle<(u32, Vec<(u32, bool)>)>> = Vec::new();
    let mut thread_count: u32 = usize::from(available_parallelism().unwrap()) as u32;

    println!("Checking jobs head");
    let mut curr_block: u32 = match imap::check_jobs_head(&db, thread_count) {
        Some(vec) => {
            for block in vec {
                imap::assign_job(block, &mut threads);
            }
            imap::get_job(&db) 
        }
        None => imap::get_job(&db),
    } + BLOCK_SIZE;

    curr_block = u32::from_be_bytes([127, 0, 0, 0]);

    for thread in 0..thread_count {
        imap::assign_job(curr_block, &mut threads);
        curr_block += BLOCK_SIZE;
    }

    let mut completed_jobs = 0;
    while completed_jobs < 5 {
        for idx in 0..threads.len() {
            if idx < threads.len() && threads[idx].is_finished() {
                imap::clean_up_job(idx, threads.remove(idx), &db);
                imap::assign_job(curr_block, &mut threads);
                curr_block += BLOCK_SIZE;
                completed_jobs += 1;
            }
        }
    }

    imap::close_remaining_threads(&mut threads, &db);
    Ok(())
}
