use std::net::SocketAddr;
use std::path::Path;

use axum::extract::ws::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::routing::{get, get_service};
use axum::Router;
use clap::Parser;
use futures_util::StreamExt;
use tower_http::services::ServeDir;

#[derive(Parser)]
#[clap()]
struct Args {
    #[clap(long, default_value = "../space_game")]
    space_game_pkg: String,

    #[clap(long, default_value = "127.0.0.1:8000")]
    addr: SocketAddr,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    assert!(Path::new(&args.space_game_pkg).is_dir());

    let handle_ws = get(|wsu: WebSocketUpgrade| async {
        wsu.on_upgrade(|mut ws| async move {
            while let Some(val) = ws.next().await {
                println!("Got: {:?}", val);
            }
            println!("Closed");
        })
    });
    let serve_space_game =
        get_service(ServeDir::new(&args.space_game_pkg)).handle_error(|err| async move {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unhandled internal error: {}", err),
            )
        });
    let app = Router::new()
        .route("/ws/v1", handle_ws)
        .fallback(serve_space_game);
    axum::Server::bind(&args.addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
