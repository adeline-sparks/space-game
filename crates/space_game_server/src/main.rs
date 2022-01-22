use std::path::Path;

use warp::Filter;
use clap::Parser;

#[derive(Parser)]
#[clap()]
struct Args {
    #[clap(long)]
    space_game_pkg: String,
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
        .run(([127, 0, 0, 1], 3030))
        .await;
}
