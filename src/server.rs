use std::net::{SocketAddr, TcpListener};
use std::io::{BufRead, BufReader};

use crate::Client;

//const CLIENT_PORT: u16 = 8043;

pub struct Server {
    listener: TcpListener,
    clients: Vec<Client>
}

impl Server {
    pub fn new(addr: &str) -> Server {
        let listener = TcpListener::bind(addr).unwrap();

        Server{ listener, clients: vec![] }
    }

    pub fn listen(&mut self) {
        let (stream, addr) = self.listener.accept().unwrap();

        let buf_reader = BufReader::new(stream.try_clone().unwrap());

        let requests = buf_reader
            .lines()
            .map(|l| l.unwrap());

        for request in requests {
            println!("Received: {:#?}", request);

            if request.starts_with("CONNECT ") {
                let nick = request.split(' ').nth(1).unwrap();
                let stream = stream.try_clone().unwrap();

                self.clients.push(
                    Client::from_stream(nick, stream));

                println!("{nick} connected.");
            } else if request.starts_with("SEND ") {
                let msg = &request[5..];

                self.send_messages(addr, msg);
            }
        }
    }

    fn send_messages(&mut self, sender_addr: SocketAddr, msg: &str) {
        let sender_nick = self.clients
            .iter()
            .take_while(|c| c.peer_addr().unwrap() == sender_addr)
            .last()
            .unwrap()
            .nick();

        let msg = String::from("SENT ") +
            &String::from(sender_nick) +
            " " + msg + "\n";

        let mut to_delete = vec![];

        for (i, client) in self.clients.iter_mut().enumerate() {
            if let Err(e) = client.write_stream(msg.as_bytes()) {
                println!("Error: {e}");
                to_delete.push(i);
            }
        }

        for i in to_delete {
            self.clients.remove(i);
        }
    }

}
