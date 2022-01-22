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

    // GET /hello/warp => 200 OK with body "Hello, warp!"
    let hello = 
        warp::path!("hello" / String)
        .map(|name| format!("Hello, {}!", name));
    let space_game_pkg = 
        warp::fs::dir(args.space_game_pkg);
    let filters = 
        hello.or(space_game_pkg);
    warp::serve(filters)
        .run(args.addr)
        .await;
}
