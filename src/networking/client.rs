use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::io::{Read, Write};
use std::fs::File;
use std::sync::{Arc, Mutex};

use crate::common;
use crate::common::hash::hash_file_sha256;
use std::net::{UdpSocket};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct PingResponse {
    pub addr: std::net::SocketAddr,
    pub os: String,
    pub hostname: String,
}

impl Default for PingResponse {
    fn default() -> Self {
        PingResponse {
            addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
            os: String::new(),
            hostname: String::new(),
        }
    }
}

impl PingResponse {
    pub fn new(addr: std::net::SocketAddr, os: String, hostname: String) -> Self {
        Self {
            addr,
            os, 
            hostname
        }
    }
}

/*pub fn data_connection() {
    let mut stream = TcpStream::connect("127.0.0.1:8080").unwrap();

    const CHUNK_SIZE: usize = 64 * 1024; // 64 KB
    let mut file = File::open("test.pdf").expect("Cannot open file");

    let hash = hash_file_sha256("test.pdf").unwrap();

    stream.write_all(&hash).unwrap();

    let mut buffer = [0u8; CHUNK_SIZE];

    loop {
        let n = file.read(&mut buffer).expect("Cannot read");
        if n == 0 {
            break;
        }

        stream.write_all(&buffer[..n]).unwrap();
    }
}*/

pub fn control_connection(dest: std::net::SocketAddr, files: Vec<String>, status: Arc<Mutex<HashMap<u32, common::transfer_state::TransferState>>>) {
    let mut status_lock = status.lock().unwrap();
    let next_key = status_lock.keys().max().map_or(0, |k| k + 1);
    let mut tmp = common::transfer_state::TransferState::default();
    tmp.ttype = common::transfer_state::TransferType::Sending;
    tmp.percentage = 50.0;
    tmp.original_filepath = "/home/fabio/test".to_string();
    status_lock.insert(next_key, tmp);

    let next_key2 = status_lock.keys().max().map_or(0, |k| k + 1);
    let mut tmp2 = common::transfer_state::TransferState::default();
    tmp2.ttype = common::transfer_state::TransferType::Receiving;
    tmp2.percentage = 30.0;
    tmp2.dest_filepath = "/home/fabio/test2".to_string();
    status_lock.insert(next_key2, tmp2);
}

pub fn info_socket(responders_list : &mut Arc<Mutex<HashSet<PingResponse>>>) {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Could not bind UDP socket");

    println!("{}", socket.local_addr().unwrap().ip().to_string());

    socket.set_broadcast(true).expect("set_broadcast call failed");
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set_read_timeout call failed");
    
    println!("{}", socket.broadcast().unwrap());

    loop {
        //let broadcast_addr = "255.255.255.255:24934";
        let broadcast_addr = "192.168.1.45:24934";
        let ping_message = b"ping";
        let mut responders = HashSet::new();

        // Loop to send ping, receive responses, and repeat after a delay
        loop {
            responders.clear();

            socket.send_to(ping_message, broadcast_addr).expect("Failed to send ping");

            println!("Sent broadcast message");

            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(2) {
                let mut buf = [0u8; 1024];
                if let Ok((amt, src)) = socket.recv_from(&mut buf) {
                    let text = String::from_utf8(buf[..amt].to_vec()).unwrap();
                    let mut lines = text.lines().filter(|l| !l.trim().is_empty());let hostname = lines.next().unwrap_or("").to_string();
                    let os = lines.next().unwrap_or("").to_string();
                    responders.insert(PingResponse::new(src, os, hostname));
                }
            }

            //println!("Responders: {:?}", responders);

            *responders_list.lock().unwrap() = responders.clone();

            // Wait a few seconds before next broadcast
            std::thread::sleep(Duration::from_secs(3));
        }
    }
    
}