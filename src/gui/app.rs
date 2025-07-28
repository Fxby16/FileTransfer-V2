use eframe::{egui};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::common;
use crate::networking::client::PingResponse;
use crate::networking::{client, server};

extern crate nfd;

use nfd::Response;

pub struct MyApp {
    gui_state: usize, //0 = send, 1 = receive
    selected_step: usize,
    selected_dest: PingResponse,
    selected_files: Vec<String>,
    responders: Arc<Mutex<HashSet<client::PingResponse>>>,
    transfer_status: Arc<Mutex<HashMap<u32, common::transfer_state::TransferState>>>
}

impl MyApp {
    pub fn new() -> Self {
        let app = Self {
            gui_state: 0,
            selected_step: 0,
            selected_dest: PingResponse::default(),
            selected_files: Vec::new(),
            responders: Arc::new(std::sync::Mutex::new(HashSet::new())),
            transfer_status: Arc::new(std::sync::Mutex::new(HashMap::new()))
        };

        app.start_threads();

        app
    }

    fn start_threads(&self) {
        let mut responders = self.responders.clone();
        thread::spawn(move || client::info_socket(&mut responders));
    
        thread::spawn(|| server::info_socket());

        println!("Started all threads");
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, label) in ["Invio", "Ricezione"].iter().enumerate() {
                    if ui.selectable_label(self.gui_state == i, *label).clicked() {
                        self.gui_state = i;
                        self.selected_step = 0;
                    }
                }
            });
        });

        egui::SidePanel::left("sidebar").show(ctx, |ui| {
            if self.gui_state == 0 {
                ui.vertical(|ui| {
                    for (i, label) in ["1. Seleziona destinatario", "2. Seleziona file", "3. Stato"].iter().enumerate() {
                        if ui.selectable_label(self.selected_step == i, *label).clicked() {
                            self.selected_step = i;
                        }
                    }
                });
            }

            if self.gui_state == 1 {
                ui.vertical(|ui| {
                    for (i, label) in ["1. Richieste in entrata", "2. Stato"].iter().enumerate() {
                        if ui.selectable_label(self.selected_step == i, *label).clicked() {
                            self.selected_step = i;
                        }
                    }
                });
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.gui_state == 0 {
                match self.selected_step {
                    0 => {
                        let responders = self.responders.lock().unwrap();
                        for responder in responders.iter() {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(format!("{}", responder.addr));
                                    ui.label(format!("OS: {}", responder.os));
                                    ui.label(format!("Hostname: {}", responder.hostname));
                                });
                                ui.add_space(16.0);
                                if ui.button("Seleziona").clicked() {
                                    self.selected_dest = responder.clone();
                                    self.selected_step = 1;
                                }
                            });
                            ui.separator();
                        }
                    },
                    1 => {
                        ui.label(format!("Current dest: {} OS: {} Hostname: {}", self.selected_dest.addr,self.selected_dest.os,self.selected_dest.hostname));
                        
                        if ui.button("Seleziona file").clicked() {
                            let result = nfd::dialog_multiple().open().unwrap_or_else(|e| {
                                panic!("{}", e);
                            });

                            match result {
                                Response::Okay(file_path) => self.selected_files = vec![file_path],
                                Response::OkayMultiple(files) => self.selected_files = files,
                                Response::Cancel => println!("User canceled"),
                            }
                        }

                        ui.vertical(|ui| {
                            // Calcola lo spazio disponibile per la lista, lasciando spazio per il bottone
                            let available_height = ui.available_height() - 40.0; // 40px circa per il bottone
                        
                            egui::ScrollArea::both() // verticale e orizzontale
                                .max_height(available_height)
                                .show(ui, |ui| {
                                    let mut to_remove = None;
                                    for (i, file) in self.selected_files.iter().enumerate() {
                                        ui.horizontal(|ui| {
                                            ui.label(file);
                                            if ui.button("❌").on_hover_text("Rimuovi").clicked() {
                                                to_remove = Some(i);
                                            }
                                        });
                                    }
                                    if let Some(i) = to_remove {
                                        self.selected_files.remove(i);
                                    }
                                });
                        
                            ui.add_space(8.0);
                            if ui.button("Invia").clicked() {
                                let dest = self.selected_dest.addr.clone();
                                let files = self.selected_files.clone();
                                let status = self.transfer_status.clone();

                                thread::spawn(move || {
                                    client::control_connection(dest, files, status);
                                });

                                self.selected_step = 2;
                            }
                        });
                    },
                    2 => {
                        let transfer_status = self.transfer_status.lock().unwrap();
                        let mut status_vec: Vec<_> = transfer_status.iter().collect();
                        status_vec.sort_by_key(|(k, _)| *k);

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (id, state) in status_vec {
                                ui.horizontal(|ui| {
                                    match state.ttype {
                                        common::transfer_state::TransferType::Sending => {
                                            ui.label(format!("ID: {} Status: Sending Filepath: {}, Percentage: {}", id, state.original_filepath, state.percentage));
                                        },
                                        common::transfer_state::TransferType::Receiving => {
                                            ui.label(format!("ID: {} Status: Receiving Filepath: {}, Percentage: {}", id, state.dest_filepath, state.percentage));
                                        }
                                    }
                                });
                                ui.separator();
                            }
                        });
                    },
                    _ => {}
                }
            }

            if self.gui_state == 1 {
                match self.selected_step {
                    0 => {
                        ui.label("Richieste in entrata");
                    },
                    1 => {
                        ui.label("Stato");
                    },
                    _ => {}
                }
            }
        });
    }
}