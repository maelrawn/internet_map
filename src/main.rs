mod net;
use crate::net::net::Block;
use crate::net::*;
use std::{
        time::{Instant, Duration},
        error::Error,
        collections::VecDeque,
    };
// use futures::future::join_all;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    let mut blocks = Vec::new();
    for byte in 0..=255 {
        blocks.push(Block::new([8,8,byte]));
        blocks[byte as usize].process().await;
        let dur = Instant::now().duration_since(start);
        let num = blocks.len();
        println!("{:?} blocks took {:?}ms", num, dur.as_millis());
    }
    Ok(())
}