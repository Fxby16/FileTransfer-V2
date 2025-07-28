use std::collections::{BTreeMap, HashMap};
use std::net::TcpListener;
use std::io::{Read, Write};
use std::fs::File;


use std::collections::hash_set;

use crate::common::hash::hash_file_sha256;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Default)]
pub struct ServerControlData {
    data_threads: BTreeMap<u32, (String, std::thread::JoinHandle<()>)>,
    pub counter: AtomicU32,
}

impl ServerControlData {
    pub fn next_id(&self) -> u32 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }
}

pub fn control_connection() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    println!("Server in ascolto su 127.0.0.1:8080");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Nuova connessione accettata");

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
        std::thread::spawn(move || {
            // Crea il file di destinazione
            /*let mut output_file = File::create("received_file.pdf").expect("Cannot create output file");

            const CHUNK_SIZE: usize = 64 * 1024; // 64 KB
            let mut buffer = [0u8; CHUNK_SIZE];
            let mut total_bytes = 0;

            let mut hash = [0u8; 32];

            loop {
                match stream.try_clone().unwrap().read(&mut buffer) {
                    Ok(0) => {
                        // Fine della connessione
                        println!("Connessione chiusa dal client");
                        break;
                    }
                    Ok(n) => {
                        // Scrivi i dati ricevuti nel file
                        hash.copy_from_slice(&buffer[0..32]);
                        output_file.write_all(&buffer[32..n]).expect("Cannot write to file");
                        total_bytes += n - 32;
                        println!("Ricevuti {} bytes (totale: {} bytes)", n, total_bytes);
                        break;
                    }
                    Err(e) => {
                        println!("Errore nella lettura: {}", e);
                        break;
                    }
                }
            }

            loop {
                match stream.try_clone().unwrap().read(&mut buffer) {
                    Ok(0) => {
                        // Fine della connessione
                        println!("Connessione chiusa dal client");
                        break;
                    }
                    Ok(n) => {
                        // Scrivi i dati ricevuti nel file
                        output_file.write_all(&buffer[..n]).expect("Cannot write to file");
                        total_bytes += n;
                        println!("Ricevuti {} bytes (totale: {} bytes)", n, total_bytes);
                    }
                    Err(e) => {
                        println!("Errore nella lettura: {}", e);
                        break;
                    }
                }
            }

            let received_file_hash = hash_file_sha256("received_file.pdf").unwrap();

            if received_file_hash != hash {
                println!("File corrotto");
            } else {
                println!("File ricevuto completamente: {} bytes totali", total_bytes);
            }*/
        });
    }
}

pub fn info_socket() {
    let socket = UdpSocket::bind("0.0.0.0:24934").expect("Could not bind UDP socket");
    println!("UDP socket in ascolto su 0.0.0.0:24934");

    println!("{}", socket.local_addr().unwrap().ip().to_string());

    let mut buf = [0u8; 1024];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((n, src)) => {
                println!("Ricevuto ping da {}: {} bytes", src, n);
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