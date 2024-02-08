use std::io::{self, stdin, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread;

#[derive(Debug)]
pub struct Client {
    stream: TcpStream,
    nick: String,
    addr: SocketAddr
}

impl Client {
    pub fn new(addr: &str, nick: &str) -> Client {
        let stream = TcpStream::connect(addr).unwrap();
        let nick = String::from(nick);
        let addr = stream.peer_addr().unwrap();

        Client { stream, nick, addr }
    }

    pub fn from_stream(nick: &str, stream: TcpStream) -> Client {
        let nick = String::from(nick);
        let addr = stream.peer_addr().unwrap();

        Client { stream, nick, addr }
    }

    pub fn connect(&mut self) {
        let request = String::from("CONNECT ") + &self.nick + "\n";
        self.stream.write(request.as_bytes()).unwrap();
        self.listen();
        self.input_loop();
    }

    pub fn nick(&self) -> &String {
        &self.nick
    }

    pub fn write_stream(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.stream.write(buf)
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.addr
    }

    fn input_loop(&mut self) {
        let mut message = String::new();

        while message.trim() != "!quit" {
            message.clear();

            print!("{}> ", self.nick);
            io::stdout().flush().unwrap();

            stdin().read_line(&mut message).unwrap();

            let request = String::from("SEND ") + &message;

            self.stream.write(request.as_bytes()).unwrap();
        }
    }

    fn listen(&mut self) {
        let stream = self.stream.try_clone().unwrap();
        let client_nick = self.nick.clone();

        thread::spawn(move || loop {
            let buf_reader = BufReader::new(&stream);

            let requests = buf_reader.lines().map(|l| l.unwrap());

            for request in requests {
                if request.starts_with("SENT ") {
                    let mut request_words = request.split(' ');
                    let nick = request_words.nth(1).unwrap();
                    let msg = &request[4+nick.len()+2..];

                    println!("\r\x1b[K{nick}: {msg}");
                    print!("{}> ", client_nick);
                    io::stdout().flush().unwrap();
                } else if request.starts_with("INTRODUCE ") {
                    let nick = match request.split(' ').nth(1) {
                        Some(s) => s,
                        None => continue
                    };

                    println!("\r\x1b[K{nick} connected to the server.");

                    if nick != client_nick {
                        print!("{}> ", client_nick);
                        io::stdout().flush().unwrap();
                    }
                }
            }
        });

        thread::sleep(std::time::Duration::from_millis(250));
    }
}
