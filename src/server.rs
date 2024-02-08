use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::Client;

//const CLIENT_PORT: u16 = 8043;

pub struct Server {
    listener: Option<Listener>,
    receiver: Receiver<(String, TcpStream)>,
    clients: Vec<Client>
}

impl Server {
    pub fn new(addr: &str) -> Server {
        let listener = TcpListener::bind(addr).unwrap();
        let (sender, receiver) = mpsc::channel();
        let listener = Some(Listener { listener, sender });

        Server {
            listener,
            receiver,
            clients: vec![],
        }
    }

    pub fn listen(&mut self) {
        let listener = self.listener.take().unwrap();

        thread::spawn(move || {
            listener.listen();
        });
    }

    pub fn handle_requests(&mut self) {
        loop {
            let (request, stream) = self.receiver.recv().unwrap();

            println!("Request: {:#?}", request);
            println!("Stream: {:#?}", stream);

            if request.starts_with("CONNECT ") {
                let nick = request.split(' ').nth(1).unwrap();
                let stream = stream.try_clone().unwrap();

                self.clients.push(Client::from_stream(nick, stream));

                self.announce_connected(nick);

                println!("{nick} connected.");
            } else if request.starts_with("SEND ") {
                let msg = &request[5..];
                let addr = stream.peer_addr().unwrap();

                self.send_messages(addr, msg);
            }
        }
    }

    fn announce_connected(&mut self, nick: &str) {
        let msg = String::from("INTRODUCE ") + nick + "\n";

        for client in self.clients.iter_mut() {
            client.write_stream(msg.as_bytes()).unwrap();
        }
    }

    fn send_messages(&mut self, sender_addr: SocketAddr, msg: &str) {
        let same_address = |c: &Client| c.peer_addr().unwrap() == sender_addr;
        let sender_index = match self.clients.iter().position(same_address) {
            None => {
                println!("Unrecognized client");
                return;
            }
            Some(i) => i,
        };

        let sender_nick = self.clients[sender_index].nick();

        let msg = String::from("SENT ") +
            &String::from(sender_nick) + " " + msg + "\n";

        let mut to_delete = vec![];

        for (i, client) in self.clients.iter_mut().enumerate() {
            if same_address(client) {
                continue;
            }

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



struct Listener {
    listener: TcpListener,
    sender: Sender<(String, TcpStream)>
}

impl Listener {
    fn listen(&self) {
        loop {
            let (stream, _) = self.listener.accept().unwrap();
            let sender = self.sender.clone();

            thread::spawn(move || {
                let buf_reader = BufReader::new(stream.try_clone().unwrap());

                let requests = buf_reader.lines().map(|l| l.unwrap());

                for request in requests {
                    let stream = stream.try_clone().unwrap();
                    sender.send((request, stream)).unwrap();
                }
            });
        }
    }
}
