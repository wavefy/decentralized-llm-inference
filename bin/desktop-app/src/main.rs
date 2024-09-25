// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use std::env;
use utils::random_node_id;

use core::net::SocketAddr;
use openai_server::start_control_server;
use tauri::generate_context;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// status bind addr
    #[arg(env, long, default_value = "127.0.0.1:18889")]
    control_bind: SocketAddr,

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
    node_id: Option<String>,
}

#[tokio::main]
async fn main() {
    env_logger::builder().filter_level(log::LevelFilter::Info).format_timestamp_millis().init();
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    let args = Args::parse();

    let node_id = args.node_id.unwrap_or_else(random_node_id);

    tauri::Builder::default()
        .setup(move |_app| {
            // let window = app.get_window("main").unwrap();
            // window.open_devtools();
            tauri::async_runtime::spawn(async move {
                start_control_server(args.control_bind, &args.registry_server, &node_id, args.http_bind, &args.stun_server).await;
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
