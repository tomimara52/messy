use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::Client;

//const CLIENT_PORT: u16 = 8043;

pub struct Server {
    listener: TcpListener,
    clients: Arc<Mutex<Vec<Client>>>,
}

impl Server {
    pub fn new(addr: &str) -> Server {
        let listener = TcpListener::bind(addr).unwrap();

        Server {
            listener,
            clients: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn listen(&mut self) {
        loop {
            let (stream, addr) = self.listener.accept().unwrap();
            let clients = Arc::clone(&self.clients);

            thread::spawn(move || {
                let buf_reader = BufReader::new(stream.try_clone().unwrap());

                let requests = buf_reader.lines().map(|l| l.unwrap());

                for request in requests {
                    println!("Received: {:#?}", request);

                    if request.starts_with("CONNECT ") {
                        let nick = request.split(' ').nth(1).unwrap();
                        let stream = stream.try_clone().unwrap();

                        clients
                            .lock()
                            .unwrap()
                            .push(Client::from_stream(nick, stream));

                        println!("{nick} connected.");
                    } else if request.starts_with("SEND ") {
                        let msg = &request[5..];

                        send_messages(&mut clients.lock().unwrap(), addr, msg);
                    }
                }
            });
        }
    }
}

fn send_messages(clients: &mut Vec<Client>, sender_addr: SocketAddr, msg: &str) {
    let same_address = |c: &Client| c.peer_addr().unwrap() == sender_addr;
    let sender_index = match clients.iter().position(same_address) {
        None => {
            println!("Unrecognized client");
            return;
        }
        Some(i) => i,
    };

    let sender_nick = clients[sender_index].nick();

    let msg = String::from("SENT ") + &String::from(sender_nick) + " " + msg + "\n";

    let mut to_delete = vec![];

    for (i, client) in clients.iter_mut().enumerate() {
        if same_address(client) {
            continue;
        }

        if let Err(e) = client.write_stream(msg.as_bytes()) {
            println!("Error: {e}");
            to_delete.push(i);
        }
    }

    for i in to_delete {
        clients.remove(i);
    }
}
