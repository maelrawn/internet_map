mod tests {
	use internet_map::imap;
	use std::fs;

	#[test]
	fn ping_success_and_failure() {
	    let cloudflare_ip = u32::from_be_bytes([1,1,1,1]);

	    assert!(imap::ping_u32(cloudflare_ip));

	    let intel_ip = u32::from_be_bytes([192,198,149,0]);

	    assert!(!imap::ping_u32(intel_ip)); 
	}

	#[test]
	fn populate_progress_test() {
		fs::remove_file("progress_test.db");
		
		let flags = sqlite::OpenFlags::new()
			.set_create()
			.set_full_mutex()
			.set_read_write();
		let db = sqlite::Connection
			::open_with_flags("progress_test.db", flags).unwrap();
		
		let query = "CREATE TABLE blocks (block INTEGER)";
		db.execute(query).unwrap();
		
		for ip_block in 0..=100 {
			let query = format!("INSERT INTO blocks VALUES ({:?})", ip_block);
			assert_eq!(db.execute(query).unwrap(), ());
		}

		// let query = "SELECT * FROM progress";

		// for row in db
		//     .prepare(query)
		//     .unwrap()
		//     .into_iter()
		//     .map(|row| row.unwrap())
		// {
		//     println!("ip_block = {}", row.read::<i64, _>("block"));
		//     println!("status = {}", row.read::<i64, _>("status"));
		// }
	}

	#[test]

}
