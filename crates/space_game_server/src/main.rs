use std::{path::Path, net::SocketAddr};

use warp::Filter;
use clap::Parser;

#[derive(Parser)]
#[clap()]
struct Args {
    #[clap(long)]
    space_game_pkg: String,

    #[clap(long, default_value="127.0.0.1:3030")]
    addr: SocketAddr,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    assert!(Path::new(&args.space_game_pkg).is_dir());

    let ws =
        warp::path!("ws" / "v1")
        .and(warp::addr::remote())
        .and(warp::ws::ws())
        .map(|addr, ws: warp::ws::Ws| ws.on_upgrade(move |socket| async move {
            println!("Got connection: {:?}", addr);
            socket.close().await.unwrap();
        }));
    let space_game_pkg = 
        warp::fs::dir(args.space_game_pkg);
    let filters = 
        space_game_pkg
        .or(ws);
    warp::serve(filters)
        .run(args.addr)
        .await;
}
