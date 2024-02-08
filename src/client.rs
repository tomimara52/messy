use std::io::{self, stdin, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread;

#[derive(Debug)]
pub struct Client {
    stream: TcpStream,
    nick: String,
}

impl Client {
    pub fn new(addr: &str, nick: &str) -> Client {
        let stream = TcpStream::connect(addr).unwrap();

        let nick = String::from(nick);

        Client { stream, nick }
    }

    pub fn from_stream(nick: &str, stream: TcpStream) -> Client {
        let nick = String::from(nick);

        Client { nick, stream }
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

    pub fn peer_addr(&self) -> Result<SocketAddr, io::Error> {
        self.stream.peer_addr()
    }

    fn input_loop(&mut self) {
        let mut message = String::new();

        while message.trim() != "!quit" {
            message.clear();

            stdin().read_line(&mut message).unwrap();

            let request = String::from("SEND ") + &message;

            self.stream.write(request.as_bytes()).unwrap();
        }
    }

    fn listen(&mut self) {
        let stream = self.stream.try_clone().unwrap();

        thread::spawn(move || loop {
            let buf_reader = BufReader::new(&stream);

            let requests = buf_reader.lines().map(|l| l.unwrap());

            for request in requests {
                if request.starts_with("SENT ") {
                    let mut request_words = request.split(' ');
                    let nick = request_words.nth(1).unwrap();
                    let msg = &request[4+nick.len()+2..];

                    println!("{nick}: {msg}");
                } else if request.starts_with("INTRODUCE ") {
                    let nick = match request.split(' ').nth(1) {
                        Some(s) => s,
                        None => continue
                    };

                    println!("{nick} connected to the server.");
                }
            }
        });
    }
}
