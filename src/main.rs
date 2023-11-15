use internet_map::imap;
use std::{
    sync::{Arc},
    fs::metadata,
    error::Error,
    thread::{self, available_parallelism, JoinHandle},
};

const BLOCK_SIZE: u16 = 0x0001;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let db_path: String = "internet.db".to_string();
    let db = imap::create_if_not_exists(&db_path).expect("Failed to open file.");

    // println!("Testing internet connectivity...");
    // imap::test_connection()?;

    // let mut threads = Vec::new();
    // let mut thread_count: u32 = usize::from(available_parallelism().unwrap()) as u32;

    // println!("Checking jobs head");
    // let mut curr_block: u16 = match imap::check_jobs_head(&db, thread_count) {
    //     Some(vec) => {
    //         for block in vec {
    //            threads = imap::assign_job(&client, block, threads);
    //         }
    //         imap::get_job(&db) 
    //     }
    //     None => imap::get_job(&db),
    // } + BLOCK_SIZE;

    let curr_block = u16::from_be_bytes([8, 8]);

    let batch_results = imap::batch_jobs(curr_block, 2, db_path).await.unwrap();
    for batch in batch_results {
        imap::write_results(batch.0, batch.1, &db);
    }

    // let mut completed_jobs = 0;
    // while completed_jobs < 5 {
    //     for idx in 0..threads.len() {
    //         if idx < threads.len() && threads[idx].is_finished() {
    //             imap::clean_up_job(idx, threads.remove(idx), &db);
    //             imap::assign_job(&client, curr_block, &mut threads);
    //             curr_block += BLOCK_SIZE;
    //             completed_jobs += 1;
    //         }
    //     }
    // }

    // imap::close_remaining_threads(&mut threads, &db);
    Ok(())
}
