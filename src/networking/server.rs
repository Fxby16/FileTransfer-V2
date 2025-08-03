use std::collections::{BTreeMap, HashMap};
use std::net::TcpListener;
use std::io::{Read, Write};
use std::fs::File;

use multiset::HashMultiSet;

use crate::common::{self, counter};
use crate::common::hash::hash_file_sha256;
use crate::networking::client::PingResponse;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use crate::networking::client;



#[derive(Default)]
pub struct ServerControlData {
    pub data_threads: BTreeMap<u32, (String, std::thread::JoinHandle<()>)>,
    pub accepted_files: Arc<Mutex<HashMap<String, HashMultiSet<String>>>>,
}

pub struct RequestData {
    pub from: PingResponse,
    pub files: Vec<(String, u64)>,
    pub accepted_files: Option<Vec<String>>,
}

impl Default for RequestData {
    fn default() -> Self {
        RequestData {
            from: PingResponse::default(),
            files: Vec::new(),
            accepted_files: None,
        }
    }
}

pub fn control_connection(status: Arc<Mutex<HashMap<u32, common::transfer_state::TransferState>>>, control_data: Arc<Mutex<ServerControlData>>, responders: Arc<Mutex<HashSet<client::PingResponse>>>, incoming_requests: Arc<Mutex<HashMap<u32, RequestData>>>) {
    let listener = TcpListener::bind("0.0.0.0:24934").unwrap();
    //println!("Server in ascolto su 127.0.0.1:24934");

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();

        /*
        request:
        ask the server if he wants to receive N files. then filename and size is specified.

        Files N\n
        File1.pdf 238974619\n
        ...
        FileN.pdf 129038745\n

        response:
        send the client the list of the files the server is willing to receive.

        Accept
        File2.pdf\n
        FileN.pdf\n
        */

        // Get peer info from ping response
        let peer_addr = stream.peer_addr().unwrap();

        //println!("Received connection from: {}", peer_addr);

        let mut peer_info: Option<PingResponse> = None;

        // Wait until the peer info is available in responders
        while peer_info.is_none() {
            //println!("Waiting for peer info for {}", peer_addr);
            let responders_guard = responders.lock().unwrap();
            for resp in responders_guard.iter() {
                if resp.addr.ip() == peer_addr.ip() { // Compare only IPv4 address
                    peer_info = Some(resp.clone());
                    break;
                }
            }
            drop(responders_guard);
            if peer_info.is_none() {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        let peer_info = peer_info.unwrap();

        //println!("Peer info found: {:?}", peer_info);

        let mut request = [0u8; 1024];

        stream.read(&mut request).expect("Cannot read request");

        let request_str = String::from_utf8_lossy(&request).into_owned();

        //println!("Received request: {}", request_str);

        let mut request_lines = request_str.lines();
        let first_line = request_lines.next().unwrap_or("").trim();
        let mut files = Vec::new();
        if first_line.starts_with("FILES") {
            let num_files: usize = first_line[6..].trim().parse().unwrap_or(0);
            for _ in 0..num_files {
                if let Some(line) = request_lines.next() {
                    let mut parts = line.rsplitn(2, ' '); // divide da destra, massimo 2 parti
                    let size_str = parts.next().unwrap_or("0");
                    let filename = parts.next().unwrap_or("").to_string();
                    let size: u64 = size_str.parse().unwrap_or(0);
                    files.push((filename, size));
                }
            }
        }

        let request_data = RequestData {
            from: peer_info.clone(),
            files: files.clone(),
            accepted_files: None,
        };

        //println!("Received request from {}: {:?}", peer_info.addr, request_data.files);

        // Store the request in incoming_requests
        let mut id = 0;
        {
            let mut incoming_requests_guard = incoming_requests.lock().unwrap();
            id = counter::get_inc();
            incoming_requests_guard.insert(id, request_data);
        }

        //println!("Request ID assigned: {}", id);

        std::thread::spawn({
            let control_data = Arc::clone(&control_data);
            let incoming_requests = Arc::clone(&incoming_requests);
            let id = id;
            move || {
                loop {
                    let mut incoming_requests_guard = incoming_requests.lock().unwrap();
                    if let Some(request_data) = incoming_requests_guard.get_mut(&id) {
                        if request_data.accepted_files.is_some() {

                            if request_data.accepted_files.as_ref().unwrap().is_empty() {
                                println!("No files accepted by the user.");
                                let response = "REJECT\n";
                                stream.write_all(response.as_bytes()).expect("Cannot write response");
                                break;
                            }

                            // Send the accepted files back to the client
                            let mut response = String::from("ACCEPT\n");
                            if let Some(accepted_files) = &request_data.accepted_files {
                                for file in accepted_files {
                                    response.push_str(&format!("{}\n", file));
                                }
                            }
                            stream.write_all(response.as_bytes()).expect("Cannot write response");

                            //println!("Accepted files sent to client: {:?}", request_data.accepted_files);

                            // Update status
                            //println!("Acquiring lock for transfer status - server.rs line 161");
                            
                            for file in request_data.accepted_files.as_ref().unwrap() {
                                //println!("Adding file {} to multiset for IP: {}", file, peer_info.addr.ip());
                                let control_data_guard = control_data.lock().unwrap();
                                if let Some(files_multiset) = control_data_guard.accepted_files.lock().unwrap().get_mut(&peer_info.addr.ip().to_string()) {
                                    files_multiset.insert(file.clone());
                                    //println!("File {} added to multiset for IP: {}", file, peer_info.addr.ip());
                                }else{
                                    control_data_guard.accepted_files.lock().unwrap().insert(peer_info.addr.ip().to_string(), HashMultiSet::from_iter(vec![file.clone()]));
                                    //println!("Created new multiset for IP: {} with file {}", peer_info.addr.ip(), file);
                                }
                            }

                            //println!("Releasing lock for transfer status - server.rs line 58");
                            break;
                        }
                    }else{
                        // If the request is not found, send a rejection response
                        let response = "REJECT\n";
                        stream.write_all(response.as_bytes()).expect("Cannot write response");
                        break;
                    }

                    drop(incoming_requests_guard);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        });
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect::<Vec<String>>().join("")
}

pub fn data_connection(status: Arc<Mutex<HashMap<u32, common::transfer_state::TransferState>>>, control_data_: Arc<Mutex<ServerControlData>>) {
    let listener = TcpListener::bind("0.0.0.0:24935").unwrap();
    
    //println!("Waiting for incoming connections on port 24935...");

    for stream in listener.incoming() {

        let from_ip = match stream.as_ref() {
            Ok(s) => s.peer_addr().unwrap().ip().to_string(),
            Err(e) => {
                println!("Error getting peer address: {}", e);
                continue;
            }
        };

        const MAX_ITERATIONS : u32 = 50;
        let mut ctr = 0;
        let mut should_skip = false;

        loop {

            if ctr >= MAX_ITERATIONS {
                should_skip = true;
                break;
            }

            //println!("Checking for accepted files for IP: {}", from_ip);

            let control_lock = control_data_.lock().unwrap();

            //println!("Acquired control lock, checking accepted files...");

            let accepted_files_lock = control_lock.accepted_files.lock().unwrap();

            //println!("Acquired accepted files lock, checking for files...");

            for (ip, files) in accepted_files_lock.iter() {
                //println!("Accepted files for {}: {:?}", ip, files);
            }

            if accepted_files_lock.contains_key(&from_ip) {
                if !accepted_files_lock.get(&from_ip).unwrap().is_empty() {
                    break;
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(100));

            ctr += 1;
        }

        //println!("Incoming connection received");

        if should_skip {
            println!("No accepted files for IP: {}, skipping connection", from_ip);
            continue;
        }
        
        let status = Arc::clone(&status);
        let control_data = Arc::clone(&control_data_);
        
        let from_ip_clone = from_ip.clone();
        let key = counter::get_inc();
        
        let thread_join_handle = std::thread::spawn(move || {
            let mut stream = stream.unwrap();

            let mut file = [0u8; 256];
            let mut file_size = [0u8; 8];
            let mut hash = [0u8; 32];

            //println!("Waiting for file name, size and hash from stream...");

            // Read the file name and hash from the stream (256 + 32 bytes)
            stream.read_exact(&mut file).expect("Cannot read file name");
            stream.read_exact(&mut file_size).expect("Cannot read file size");
            stream.read_exact(&mut hash).expect("Cannot read file hash");
            let file_name = String::from_utf8_lossy(&file).trim_end_matches('\0').to_string();

            let file_size = u64::from_le_bytes(file_size);

            //println!("Received request for file: {} with hash: {}", file_name, bytes_to_hex(&hash));

            let mut control_guard = control_data.lock().unwrap();
            let accepted_files_guard = control_guard.accepted_files.lock().unwrap().clone();

            if !accepted_files_guard.contains_key(&from_ip_clone) || !accepted_files_guard.get(&from_ip_clone).unwrap().contains(&file_name) {
                println!("File {} not accepted", file_name);

                control_guard.data_threads.remove(&key);

                stream.write_all(b"REJECT\n").expect("Cannot send rejection");

                return;
            }

            stream.write_all(b"ACCEPT\n").expect("Cannot send acceptance");

            let mut status_lock = status.lock().unwrap();
            let transfer_state = common::transfer_state::TransferState {
                ttype: common::transfer_state::TransferType::Receiving,
                original_filepath: String::new(),
                dest_filepath: file_name.clone(),
                percentage: 0.0,
                peer: stream.peer_addr().unwrap(),
            };
            
            //println!("Status key is {}", key);

            status_lock.insert(key, transfer_state);

            drop(status_lock);

            control_guard.data_threads.get_mut(&key).unwrap().0 = file_name.clone();

            let mut output_file = File::create(&file_name).expect("Cannot create output file");

            const CHUNK_SIZE: usize = 64 * 1024; // 64 KB
            let mut buffer = [0u8; CHUNK_SIZE];
            let mut total_bytes = 0;

            //println!("Starting receiving file: {}", file_name);

            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => {
                        //println!("Connessione chiusa dal client");
                        break;
                    }
                    Ok(n) => {
                        output_file.write_all(&buffer[..n]).expect("Cannot write to file");
                        total_bytes += n;
                        //println!("Ricevuti {} bytes (totale: {} bytes)", n, total_bytes);

                        //println!("Acquiring lock for transfer status - server.rs line 275");
                        let mut status_lock = status.lock().unwrap();
                        //println!("Lock acquired for transfer status - server.rs line 276");
                        if let Some(state) = status_lock.get_mut(&key) {
                            state.percentage = (total_bytes as f32 / file_size as f32) * 100.0;
                        }

                        //println!("Released lock for transfer status - server.rs line 280");
                    }
                    Err(e) => {
                        println!("Errore nella lettura: {}", e);
                        break;
                    }
                }
            }

            let received_file_hash = hash_file_sha256(&file_name).unwrap();

            if received_file_hash != hash {
                println!("File corrotto");
                println!("Expected hash: {}, received hash: {}", bytes_to_hex(&hash), bytes_to_hex(&received_file_hash));
            } else {
                println!("File {} ricevuto completamente: {} bytes totali", file_name, total_bytes);
            }

            // Remove the file from accepted_files
            control_guard.accepted_files.lock().unwrap().get_mut(&from_ip_clone).unwrap().remove(&file_name);
            control_guard.data_threads.remove(&key);

            //println!("Released lock for control data");
        });
        control_data_.lock().unwrap().data_threads.insert(key, (String::new(), thread_join_handle));
    }
}

pub fn info_socket() {
    let socket = UdpSocket::bind("0.0.0.0:24934").expect("Could not bind UDP socket");
    //println!("UDP socket in ascolto su 0.0.0.0:24934");

    //println!("{}", socket.local_addr().unwrap().ip().to_string());

    let mut buf = [0u8; 1024];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((n, src)) => {
                //println!("Ricevuto ping da {}: {} bytes", src, n);
                let response = format!(
                    "{}\n{}\n",
                    whoami::username(),
                    std::env::consts::OS
                );
                if let Err(e) = socket.send_to(response.as_bytes(), src) {
                    println!("Errore nell'invio della risposta: {}", e);
                }
            }
            Err(e) => {
                println!("Errore nella ricezione UDP: {}", e);
                break;
            }
        }
    }
}