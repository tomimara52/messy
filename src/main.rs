use std::env;

mod server;
mod client;

use server::Server;
use client::Client;

const HOST: &str = "127.0.0.1:8042";

fn main() {
    let mut args: Vec<_> = env::args().collect();
    let server_mode = is_server(&mut args);

    if server_mode {
        let mut server = Server::new(HOST);
        server.listen();
    } else {
        let mut client = Client::new(HOST, &args[1]);
        client.connect();
    }
}

fn is_server(args: &mut Vec<String>) -> bool {
    if args.len() < 2 {
        panic!("Wrong number of arguments.");
    }

    if args[1] == "server" {
        true
    } else {
        false
    }
}
