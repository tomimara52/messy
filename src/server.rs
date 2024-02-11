use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

use crate::Client;

const MAX_INACTIVITY: u64 = 5;

pub struct Server {
    listener: Option<Listener>,
    receiver: Receiver<ChannelPacket>,
    clients: Vec<Client>,
    last_check: Instant
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
            last_check: Instant::now()
        }
    }

    pub fn start(&mut self) {
        let listener = match self.listener.take() {
            Some(l) => l,
            None => {
                println!("Error: server was already set up to listen.");
                return;
            }
        };

        thread::spawn(move || {
            listener.listen();
        });

        self.receive_requests();
    }

    fn receive_requests(&mut self) {
        loop {
            let rec = self.receiver.try_recv();
            
            let ChannelPacket { request, stream, addr } = match rec {
                Ok(t) => t,
                Err(_) => {
                    if self.last_check.elapsed().as_secs() > MAX_INACTIVITY
                        && self.clients.len() == 0 {
                        println!("Bye");
                        break;
                    } else {
                        continue;
                    }
                }
            };

            println!("Request: {:#?}", request);
            println!("Stream: {:#?}", stream);

            match self.process_request(&request, stream, addr) {
                Err(ServerError::InvalidRequest) => {
                    println!("Invalid request received");
                },
                Err(ServerError::ClientDisconnected) => {
                    println!("Client disconnected");
                },
                _ => {}
            }

            self.last_check = Instant::now();

        }
    }

    fn process_request(
        &mut self,
        request: &str,
        stream: TcpStream,
        addr: SocketAddr
    ) -> Result<(), ServerError> {
        if request.starts_with("CONNECT ") {
            let nick = match request.split(' ').nth(1) {
                Some(s) => s,
                None => return Err(ServerError::InvalidRequest)
            };
            
            let stream = match stream.try_clone() {
                Ok(s) => s,
                Err(_) => return Err(ServerError::ClientDisconnected)
            };

            self.clients.push(Client::from_stream(nick, stream));

            self.announce_connected(nick);

            println!("{nick} connected.");
        } else if request.starts_with("SEND ") {
            if request.len() < 6 {
                return Err(ServerError::InvalidRequest);
            }
            
            let msg = &request[5..];

            self.send_client_message(addr, msg);
        } else if request.starts_with("DISCONNECT") {
            let pos = match self
                .clients
                .iter()
                .position(|c: &Client| c.peer_addr() == addr) {
                    Some(i) => i,
                    None => return Ok(())
            };

            let mut msg = "";

            if request.len() > 11 {
                msg = &request[11..];
            }

            self.announce_disconnected(addr, msg);

            self.clients.remove(pos);
        } else {
            return Err(ServerError::InvalidRequest);
        }

        Ok(())
    }

    fn announce_disconnected(&mut self, addr: SocketAddr, msg: &str) {
        let index = match self.client_index(addr) {
            Some(i) => i,
            None => return
        };

        let nick = self.clients[index].nick();

        let msg = String::from("GOODBYE ") + nick + " " + msg + "\n";

        for client in self.clients.iter_mut() {
            client.write_stream(msg.as_bytes()).unwrap();
        }
    }

    fn announce_connected(&mut self, nick: &str) {
        let msg = String::from("INTRODUCE ") + nick + "\n";

        for client in self.clients.iter_mut() {
            client.write_stream(msg.as_bytes()).unwrap();
        }
    }

    fn send_client_message(&mut self, sender_addr: SocketAddr, msg: &str) {
        let sender_index = match self.client_index(sender_addr) {
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
            if client.peer_addr() == sender_addr {
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

    fn client_index(&self, client_addr: SocketAddr) -> Option<usize> {
        let same_address = |c: &Client| c.peer_addr() == client_addr;

        self.clients.iter().position(same_address)
    }
}



struct Listener {
    listener: TcpListener,
    sender: Sender<ChannelPacket>
}

impl Listener {
    fn listen(&self) {
        loop {
            let (stream, addr) = self.listener.accept().unwrap();
            let sender = self.sender.clone();

            thread::spawn(move || {
                let buf_reader = BufReader::new(stream.try_clone().unwrap());

                let requests = buf_reader.lines();

                for request in requests {
                    let request = match request {
                        Ok(r) => r,
                        Err(_) => break
                    };

                    let stream = stream.try_clone().unwrap();

                    let packet = ChannelPacket {
                        request,
                        stream,
                        addr
                    };

                    sender.send(packet).unwrap();
                }

                let packet = ChannelPacket {
                    request: String::from("DISCONNECT"),
                    stream,
                    addr
                };

                sender.send(packet).ok();
            });
        }
    }
}

struct ChannelPacket {
    request: String,
    stream: TcpStream,
    addr: SocketAddr
}

enum ServerError {
    ClientDisconnected,
    InvalidRequest,
}
