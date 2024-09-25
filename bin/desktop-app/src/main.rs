// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use std::env;

use core::net::SocketAddr;
use openai_server::start_server;
use tauri::{generate_context, Manager};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// http bind addr
    #[arg(env, long, default_value = "127.0.0.1:1234")]
    http_bind: SocketAddr,

    /// stun server
    #[arg(env, long, default_value = "stun.l.google.com:19302")]
    stun_server: String,

    /// registry server
    #[arg(env, long, default_value = "ws://127.0.0.1:3000/ws")]
    registry_server: String,

    /// node id
    #[arg(env, long)]
    node_id: String,

    /// model id
    #[arg(env, long, default_value = "phi3")]
    model: String,

    /// model layers, layer 0 is embeding work, from 1 is for matrix jobs
    #[arg(env, long)]
    layers_from: u32,

    /// model layers, layer 0 is embeding work, from 1 is for matrix jobs
    #[arg(env, long)]
    layers_to: u32,

    /// Private key
    #[arg(env, long, default_value = "0x3bba41ade33b801bf3e42a080a699e73654eaf1775fb0afc5d65f5449e55d74b")]
    private_key: String,

    /// Contract address
    #[arg(env, long, default_value = "0x9123e2561d81ba5f77473b8dc664fa75179c841061d12264508894610b9d0b7a")]
    contract_address: String,
}

#[tokio::main]
async fn main() {
    env_logger::builder().filter_level(log::LevelFilter::Info).format_timestamp_millis().init();
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    let args = Args::parse();

    tauri::Builder::default()
        .setup(move |app| {
            // let window = app.get_window("main").unwrap();
            // window.open_devtools();
            tauri::async_runtime::spawn(async move {
                start_server(
                    &args.registry_server,
                    &args.model,
                    &args.node_id,
                    args.layers_from..args.layers_to,
                    args.http_bind,
                    &args.stun_server,
                    &args.private_key,
                    &args.contract_address,
                )
                .await;
            });
            Ok(())
        })
        .build(generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, _ev| {
            log::info!("Tauri application initialized.");
            {}
        })
}
