use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::io::{BufRead, Read, Write};
use std::fs::File;
use std::sync::{Arc, Mutex};

use local_ip_address::local_ip;
use sha2::digest::typenum::ToInt;

use crate::common::{self, counter};
use crate::common::hash::hash_file_sha256;
use std::net::{UdpSocket};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
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

pub fn control_connection(mut dest: std::net::SocketAddr, files: Vec<String>, status: Arc<Mutex<HashMap<u32, common::transfer_state::TransferState>>>) {
    dest.set_port(24934);

    let mut stream = TcpStream::connect(dest).expect("Could not connect to control server");
    
    let req_message = format!(
        "FILES {}\n{}\n",
        files.len(),
        files.iter()
            .map(|f| {
                let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
                format!("{} {}", std::path::Path::new(f).file_name().unwrap().to_string_lossy(), size)
            })
            .collect::<Vec<_>>()
            .join("\n")
    );

    //println!("Sending request: {}", req_message);

    stream.write_all(req_message.as_bytes()).expect("Failed to send request");

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("Failed to read response");

    //println!("Received response: {}", response);

    if response.starts_with("ACCEPT"){
        let accepted_files: Vec<String> = response.lines().skip(1).map(|line| line.to_string()).collect();

        if accepted_files.is_empty() {
            println!("No files accepted by the server.");
            return;
        }

        //println!("Accepted files: {:?}", accepted_files);

        for file in accepted_files {
            if let Some(original_path) = files.iter().find(|f| {
                std::path::Path::new(f).file_name().map(|n| n.to_string_lossy()) == Some(file.clone().into())
            }) {
                let next_key = counter::get_inc();
                let mut tmp = common::transfer_state::TransferState::default();
                tmp.ttype = common::transfer_state::TransferType::Sending;
                tmp.percentage = 0.0;
                tmp.original_filepath = original_path.clone();
                tmp.peer = dest.clone();
                //println!("Acquiring lock for transfer status - client.rs line 124");
                let mut status_lock = status.lock().unwrap();
                //println!("Lock acquired for transfer status - client.rs line `25`");
                status_lock.insert(next_key, tmp);

                //println!("Spawning thread for file: {}", original_path);

                std::thread::spawn({
                    let dest = dest.clone();
                    let file = original_path.clone();
                    let status = Arc::clone(&status);
                    move || {
                        data_connection(next_key, dest, file, status);
                    }
                });
                
                //println!("File {} accepted for sending", file);

                //println!("Releasing lock for transfer status - app.rs line 173");
            } else {
                println!("File {} not found in request list", file);
            }
        }
    }else if response.starts_with("REJECT") {
        println!("Server rejected the request: {}", response);
    } else {
        println!("Unexpected response from server: {}", response);
    }
}

pub fn data_connection(key: u32, mut dest: std::net::SocketAddr, file_str: String, status: Arc<Mutex<HashMap<u32, common::transfer_state::TransferState>>>) {
    dest.set_port(24935);
    
    let mut stream;

    loop {
        match TcpStream::connect(dest) {
            Ok(s) => {
                stream = s;
                println!("Connected to {}", dest);
                break;
            },
            Err(e) => {
                println!("Failed to connect to {}: {}", dest, e);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    const CHUNK_SIZE: usize = 64 * 1024; // 64 KB
    let mut file = File::open(&file_str).expect("Cannot open file");
    
    let mut filename_bytes = [0u8; 256];
    let filename_str = std::path::Path::new(&file_str)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let bytes = filename_str.as_bytes();
    let len = bytes.len().min(256);
    filename_bytes[..len].copy_from_slice(&bytes[..len]);
    stream.write_all(&filename_bytes).unwrap();

    let file_size = file.metadata().unwrap().len();
    stream.write_all(&file_size.to_le_bytes()).unwrap();

    let hash = hash_file_sha256(&file_str).unwrap();
    stream.write_all(&hash).unwrap();

    //println!("Sent file name and hash for {}", file_str);

    let mut response = String::new();
    let mut reader = std::io::BufReader::new(&stream);
    reader.read_line(&mut response).expect("Failed to read response");
    
    //println!("Received response: {}", response);

    if response.starts_with("ACCEPT") {
        println!("File {} accepted for sending", file_str);
    } else {
        println!("File {} rejected: {}", file_str, response);
        return;
    }

    let mut buffer = [0u8; CHUNK_SIZE];
    let mut total_bytes = 0;

    println!("Starting file transfer for {}", file_str);

    loop {
        let n = file.read(&mut buffer).expect("Cannot read");
        if n == 0 {
            break;
        }

        total_bytes += n;

        stream.write_all(&buffer[..n]).unwrap();

        //println!("Acquiring lock for transfer status - client.rs line 209");
        let mut status_lock = status.lock().unwrap();
        //println!("Lock acquired for transfer status - client.rs line 210");
        if let Some(state) = status_lock.get_mut(&key) {
            state.percentage = (total_bytes as f32 / file_size as f32) * 100.0;
        }

        //println!("Released lock for transfer status - client.rs line 58");
    }
}

pub fn info_socket(responders_list : &mut Arc<Mutex<HashSet<PingResponse>>>) {
    let socket = UdpSocket::bind("0.0.0.0:24936").expect("Could not bind UDP socket");

    //println!("{}", socket.local_addr().unwrap().ip().to_string());

    socket.set_broadcast(true).expect("set_broadcast call failed");
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set_read_timeout call failed");
    
    //println!("{}", socket.broadcast().unwrap());

    let local_ip = local_ip().unwrap();

    //println!("This is my local IP address: {:?}", local_ip);

    loop {
        let broadcast_addr = "255.255.255.255:24934";
        //let broadcast_addr = "192.168.1.9:24934";
        let ping_message = b"ping";
        let mut responders = HashSet::new();

        // Loop to send ping, receive responses, and repeat after a delay
        loop {
            responders.clear();

            socket.send_to(ping_message, broadcast_addr).expect("Failed to send ping");

            //println!("Sent broadcast message");

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

            responders.retain(|r| r.addr.ip() != local_ip);

            //println!("Responders: {:?}", responders);

            *responders_list.lock().unwrap() = responders.clone();

            // Wait a few seconds before next broadcast
            std::thread::sleep(Duration::from_secs(3));
        }
    }
    
}