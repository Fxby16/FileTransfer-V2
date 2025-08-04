use eframe::{egui};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::common;
use crate::networking::client::PingResponse;
use crate::networking::{client, server};

extern crate nfd;

use nfd::Response;

#[derive(Clone, Debug)]
struct RequestDetails {
    key: u32,
    accepted_files: HashSet<String>,
}

impl RequestDetails {
    pub fn new(key: u32) -> Self {
        RequestDetails {
            key,
            accepted_files: HashSet::new(),
        }
    }
}

pub struct MyApp {
    gui_state: usize, //0 = send, 1 = receive, 2 = status
    selected_step: usize,
    selected_dest: PingResponse,
    selected_files: Vec<String>,
    responders: Arc<Mutex<HashSet<client::PingResponse>>>,
    transfer_status: Arc<Mutex<HashMap<u32, common::transfer_state::TransferState>>>,
    server_control_data: Arc<Mutex<server::ServerControlData>>,
    incoming_requests: Arc<Mutex<HashMap<u32, server::RequestData>>>,
    show_details_popup: Option<RequestDetails>,
    show_details_popup_open: bool,
    confirmed_requests: HashSet<u32>, 
    context: Arc<Mutex<Option<egui::Context>>>,
}

impl MyApp {
    pub fn new() -> Self {
        let app = Self {
            gui_state: 0,
            selected_step: 0,
            selected_dest: PingResponse::default(),
            selected_files: Vec::new(),
            responders: Arc::new(std::sync::Mutex::new(HashSet::new())),
            transfer_status: Arc::new(std::sync::Mutex::new(HashMap::new())),
            server_control_data: Arc::new(std::sync::Mutex::new(server::ServerControlData::default())),
            incoming_requests: Arc::new(std::sync::Mutex::new(HashMap::new())),
            show_details_popup: None,
            show_details_popup_open: false,
            confirmed_requests: HashSet::new(),
            context: Arc::new(std::sync::Mutex::new(None)),
        };

        app.start_threads();

        app
    }

    fn start_threads(&self) {
        let mut responders = self.responders.clone();
        let mut context = self.context.clone();
        thread::spawn(move || client::info_socket(&mut responders, &context));
    
        thread::spawn(|| server::info_socket());

        thread::spawn({
            let status = self.transfer_status.clone();
            let server_control_data = self.server_control_data.clone();
            let incoming_requests = self.incoming_requests.clone();
            let responders = self.responders.clone();
            move || server::control_connection(status, server_control_data, responders, incoming_requests)
        });

        std::thread::spawn({
            let status = Arc::clone(&self.transfer_status);
            let control_data = Arc::clone(&self.server_control_data);
            move || {
                server::data_connection(status, control_data);
            }
        });
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut ctx_lock = self.context.lock().unwrap();

        if ctx_lock.is_none() {
            *ctx_lock = Some(ctx.clone());
        } 

        drop(ctx_lock);

        egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, label) in ["Send", "Receive", "Status"].iter().enumerate() {
                    if ui.selectable_label(self.gui_state == i, *label).clicked() {
                        self.gui_state = i;
                        self.selected_step = 0;
                    }
                }
            });
        });

        if self.gui_state == 0 || self.gui_state == 1 {
            egui::SidePanel::left("sidebar").show(ctx, |ui| {
                if self.gui_state == 0 {
                    ui.vertical(|ui| {
                        for (i, label) in ["1. Select the receiver", "2. Select file"].iter().enumerate() {
                            if ui.selectable_label(self.selected_step == i, *label).clicked() {
                                self.selected_step = i;
                            }
                        }
                    });
                }

                if self.gui_state == 1 {
                    ui.vertical(|ui| {
                        for (i, label) in ["1. Incoming requests"].iter().enumerate() {
                            if ui.selectable_label(self.selected_step == i, *label).clicked() {
                                self.selected_step = i;
                            }
                        }
                    });
                }
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.gui_state{
                0 => {
                    match self.selected_step {
                        0 => {
                            let responders = self.responders.lock().unwrap();
                            for responder in responders.iter() {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.add(egui::Label::new(format!("{}", responder.addr)).wrap(true));
                                        ui.add(egui::Label::new(format!("OS: {}", responder.os)).wrap(true));
                                        ui.add(egui::Label::new(format!("Hostname: {}", responder.hostname)).wrap(true));
                                    });
                                    ui.add_space(16.0);
                                    if ui.button("Select").clicked() {
                                        self.selected_dest = responder.clone();
                                        self.selected_step = 1;
                                    }
                                });
                                ui.separator();
                            }
                        },
                        1 => {
                            ui.add(egui::Label::new(format!("Current dest: {} OS: {} Hostname: {}", self.selected_dest.addr,self.selected_dest.os,self.selected_dest.hostname)).wrap(true));
                            
                            if ui.button("Select files").clicked() {
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
                                                ui.add(egui::Label::new(file).wrap(true));
                                                if ui.button("❌").on_hover_text("Remove").clicked() {
                                                    to_remove = Some(i);
                                                }
                                            });
                                        }
                                        if let Some(i) = to_remove {
                                            self.selected_files.remove(i);
                                        }
                                    });
                            
                                ui.add_space(8.0);
                                if ui.button("Send").clicked() {
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
                        _ => {}
                    }
                },
                1 => {
                    match self.selected_step {
                        0 => {
                            ui.add(egui::Label::new("Incoming requests").wrap(true));

                            let incoming_requests = self.incoming_requests.lock().unwrap();

                            for (key, request) in incoming_requests.iter() {
                                if self.confirmed_requests.contains(key) {
                                    continue;
                                }

                                ui.horizontal(|ui| {
                                    ui.add(egui::Label::new(format!("ID: {} From: {}", key, request.from.addr)).wrap(true));
                                    if ui.button("Show details").clicked() {
                                        self.show_details_popup = Some(RequestDetails::new(*key));
                                        self.show_details_popup_open = true;
                                    }
                                });
                            }

                            drop(incoming_requests);

                            if self.show_details_popup.is_some() {
                                egui::Window::new("Request details")
                                    .open(&mut self.show_details_popup_open)
                                    .min_width(400.0)
                                    .max_width(400.0)
                                    .min_height(300.0)
                                    .max_height(300.0)
                                    .show(ctx, |ui| {
                                        if let Some(request_details) = &mut self.show_details_popup {
                                            if let Some(request) = self.incoming_requests.lock().unwrap().get_mut(&request_details.key) {
                                                ui.add(egui::Label::new(format!("ID: {}", &request_details.key)).wrap(true));
                                                ui.add(egui::Label::new(format!("From: {}", request.from.addr)).wrap(true));
                                                ui.separator();

                                                egui::ScrollArea::vertical()
                                                    .max_height(180.0)
                                                    .show(ui, |ui| {
                                                        for (name, size) in request.files.iter() {
                                                            ui.horizontal(|ui| {
                                                                ui.add(egui::Label::new(format!("{} (Size: {})", name, size)).wrap(true));
                                                                let mut checked = request_details.accepted_files.contains(name);
                                                                if ui.checkbox(&mut checked, "Accept").clicked() {
                                                                    if checked {
                                                                        request_details.accepted_files.insert(name.clone());
                                                                    } else {
                                                                        request_details.accepted_files.remove(name);
                                                                    }
                                                                }
                                                            });
                                                            ui.separator();
                                                        }
                                                    });

                                                ui.add_space(8.0);
                                                if ui.button("Send response").clicked() {
                                                    let files = request_details.accepted_files.iter().cloned().collect::<Vec<_>>();
                                                    request.accepted_files = Some(files);
                                                    self.confirmed_requests.insert(request_details.key);
                                                    self.show_details_popup = None;
                                                }
                                            }
                                        }
                                    });

                                if self.show_details_popup.is_none() {
                                    self.show_details_popup_open = false;
                                }
                            }
                        },
                        _ => {}
                    }
                }, 
                2 => {
                    //println!("Acquiring lock for transfer status - app.rs line 170");
                    let transfer_status = self.transfer_status.lock().unwrap();
                    //println!("Lock acquired for transfer status - app.rs line 172");
                    let mut status_vec: Vec<_> = transfer_status.iter().collect();
                    status_vec.sort_by_key(|(k, _)| *k);

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (id, state) in status_vec {
                            ui.horizontal(|ui| {
                                match state.ttype {
                                    common::transfer_state::TransferType::Sending => {
                                        ui.add(egui::Label::new(format!("ID: {} Status: Sending Filepath: {}, Percentage: {}", id, state.original_filepath, state.percentage)).wrap(true));
                                    },
                                    common::transfer_state::TransferType::Receiving => {
                                        ui.add(egui::Label::new(format!("ID: {} Status: Receiving Filepath: {}, Percentage: {}", id, state.dest_filepath, state.percentage)).wrap(true));
                                    },
                                    common::transfer_state::TransferType::ComputingHash => {
                                        ui.add(egui::Label::new(format!("ID: {} Status: Computing Hash Filepath: {}, Percentage: {}", id, state.original_filepath, state.percentage)).wrap(true));
                                    },
                                    common::transfer_state::TransferType::VerifyingHash => {
                                        ui.add(egui::Label::new(format!("ID: {} Status: Verifying Hash Filepath: {}, Percentage: {}", id, state.dest_filepath, state.percentage)).wrap(true));
                                    },
                                    common::transfer_state::TransferType::CompletelySent => {
                                        ui.add(egui::Label::new(format!("ID: {} Status: Completed Type: Send Filepath: {}, Percentage: {}", id, state.dest_filepath, state.percentage)).wrap(true));
                                    },
                                    common::transfer_state::TransferType::CompletelyReceived => {
                                        ui.add(egui::Label::new(format!("ID: {} Status: Completed Type: Receive Filepath: {}, Percentage: {}", id, state.original_filepath, state.percentage)).wrap(true));
                                    },
                                    common::transfer_state::TransferType::Error => {
                                        ui.add(egui::Label::new(format!("ID: {} Status: Error Filepath: {}, Percentage: {}", id, state.dest_filepath, state.percentage)).wrap(true));
                                    }
                                }
                            });
                            ui.separator();
                        }
                    });

                    //println!("Releasing lock for transfer status - app.rs line 188");
                },
                _ => {}
            }
        });
    }
}