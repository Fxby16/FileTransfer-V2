use std::env;

mod networking;
mod gui;
mod common;

fn main() {
    let options = eframe::NativeOptions::default();
    match eframe::run_native(
        "FileTransfer V2",
        options,
        Box::new(|_cc| Box::new(gui::app::MyApp::new())),
    ) {
        Ok(_) => (),
        Err(e) => eprintln!("Application error: {}", e),
    }

    /*let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Please specify 'server' or 'client' as a command line argument.");
        return;
    }

    match args[1].as_str() {
        "server" => {
            networking::server::info_socket();
        }
        "client" => {
            networking::client::info_socket();
        }
        _ => {
            println!("Unknown argument '{}'. Please specify 'server' or 'client'.", args[1]);
        }
    }*/
}
