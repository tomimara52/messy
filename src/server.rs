use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;
use regex::{Captures, Regex};

const MAX_INACTIVITY: u64 = 5;

enum Command {
    Connect,
    Send,
    Disconnect,
}

pub struct Server {
    listener: Option<Listener>,
    receiver: Receiver<ChannelPacket>,
    clients: Vec<ClientConnection>,
    last_check: Instant,
    commands: Vec<(Regex, Command)>,
}

impl Server {
    pub fn new(addr: &str) -> Server {
        let listener = TcpListener::bind(addr).unwrap();
        let (sender, receiver) = mpsc::channel();
        let listener = Some(Listener { listener, sender });

        let commands = vec![
            (Regex::new(r"^CONNECT\s+(\w+)$").unwrap(), Command::Connect),
            (Regex::new(r"^SEND\s+([\S|\s]+)$").unwrap(), Command::Send),
            (Regex::new(r"^DISCONNECT(\s[\w|\s]*)?$").unwrap(), Command::Disconnect),
        ];

        Server {
            listener,
            receiver,
            clients: vec![],
            last_check: Instant::now(),
            commands
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
        for (r, c) in &self.commands {
            if let Some(captures) = r.captures(request) {
                return match c {
                    Command::Connect => { self.connect_handler(captures, stream, addr) },
                    Command::Send => { self.send_handler(captures, stream, addr) },
                    Command::Disconnect => { self.disconnect_handler(captures, stream, addr) },
                }
            }
        }

        Err(ServerError::InvalidRequest)
    }

    fn connect_handler(&mut self, captures: Captures, stream: TcpStream, _: SocketAddr) -> Result<(), ServerError> {
        let nick = captures[1].trim();

        let stream = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return Err(ServerError::ClientDisconnected)
        };

        self.clients.push(ClientConnection::new(nick, stream));

        self.announce_connected(nick);

        println!("{nick} connected.");
        Ok(())
    }

    fn send_handler(&mut self, captures: Captures, _: TcpStream, addr: SocketAddr) -> Result<(), ServerError> {
        let msg = captures[1].trim();

        self.send_client_message(addr, msg);

        Ok(())
    }

    fn disconnect_handler(&mut self, captures: Captures, _:TcpStream, addr: SocketAddr) -> Result<(), ServerError> {
        let pos = match self.client_index(addr) {
            Some(i) => i,
            None => return Ok(())
        };

        let mut msg = "";

        if let Some(e) = captures.get(1) {
            msg = e.as_str().trim();
        }

        self.announce_disconnected(pos, msg);

        self.clients.remove(pos);

        Ok(())
    }

    fn announce_disconnected(&mut self, index: usize, msg: &str) {
        let nick = &self.clients[index].nick;

        let msg = String::from("GOODBYE ") + nick + " " + msg + "\n";

        let mut to_delete = Vec::with_capacity(self.clients.len());
        
        for (i, client) in self.clients.iter_mut().enumerate() {
            if let Err(_) = client.write_stream(msg.as_bytes()) {
                if i != index {
                    to_delete.push(i); 
                }
            }
        }

        for i in to_delete {
            self.clients.remove(i);
        }
    }

    fn announce_connected(&mut self, nick: &str) {
        let msg = String::from("INTRODUCE ") + nick + "\n";

        let mut to_delete = Vec::with_capacity(self.clients.len());

        for (i, client) in self.clients.iter_mut().enumerate() {
            if let Err(_) = client.write_stream(msg.as_bytes()) {
                to_delete.push(i);
            }
        }

        for i in to_delete {
            self.clients.remove(i);
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

        let sender_nick = &self.clients[sender_index].nick;

        let msg = String::from("SENT ") +
            &String::from(sender_nick) + " " + msg + "\n";

        let mut to_delete = vec![];

        for (i, client) in self.clients.iter_mut().enumerate() {
            if client.addr == sender_addr {
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
        let same_address = |c: &ClientConnection| c.addr == client_addr;

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

struct ClientConnection {
    nick: String,
    stream: TcpStream,
    addr: SocketAddr
}

impl ClientConnection {
    fn new(nick: &str, stream: TcpStream) -> ClientConnection {
        let addr = stream.peer_addr().unwrap();
        let nick = String::from(nick);

        ClientConnection{ nick, stream, addr }
    }

    fn write_stream(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.stream.write(buf)
    }
}
