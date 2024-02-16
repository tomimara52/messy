use std::env;

mod client;
mod server;

use client::Client;
use server::Server;

const PORT: &str = "8042";

fn main() {
    let mut args: Vec<_> = env::args().collect();
    let server_mode = is_server(&mut args);

    if server_mode {
        let host = local_ip_address::local_ip().unwrap().to_string()
            + ":"
            + PORT;
        let mut server = Server::new(&host);
        server.start();
    } else {
        let nick = match args.get(1) {
            Some(n) => n,
            None => {
                println!("You have to provide a nickname.");
                std::process::exit(1);
            }
        };

        let host = match args.get(2) {
            Some(n) => n,
            None => {
                println!("You have to provide a host ip.");
                std::process::exit(1);
            }
        };

        let host = host.to_string() + ":" + PORT;

        let mut client = Client::new(&host, nick);
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
