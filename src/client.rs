use std::io::{self, stdin, stdout, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread;
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug)]
pub struct Client {
    stream: TcpStream,
    nick: String,
    addr: SocketAddr
}

impl Client {
    pub fn new(addr: &str, nick: &str) -> Client {
        let stream = TcpStream::connect(addr).expect("Host not found");
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
        let (tx, rx) = mpsc::channel();
        self.listen(rx);
        self.input_loop(tx);
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

    fn input_loop(&mut self, tx: Sender<String>) {
        let mut message = String::new();

        loop {
            message.clear();
            tx.send(String::new()).unwrap();

            print!("\r\n{}> ", self.nick);
            stdout().flush().unwrap();

            let mut buf: [u8; 1] = [0];
            enable_raw_mode().unwrap();

            loop { 
                stdin().read(&mut buf).unwrap();

                if buf[0] == 10 || buf[0] == 13 {
                    break;
                }

                if buf[0] == 127 {
                    if !message.is_empty() {
                        message.pop();
                        let b = 8 as char;
                        write!(stdout(), "{b} {b}").unwrap();
                        stdout().flush().unwrap();
                    }
                    continue;
                }

                message.push(buf[0] as char);

                tx.send(message.clone()).unwrap();

                write!(stdout(), "{}", buf[0] as char).unwrap();
                stdout().flush().unwrap();
            }

            disable_raw_mode().unwrap();

            message.push_str("\r\n");

            if message.starts_with("!quit") {
                message.replace_range(
                    0..5,
                    "DISCONNECT"
                );

                self.stream.write(message.as_bytes()).unwrap();
                break;
            }

            let request = String::from("SEND ") + &message;

            self.stream.write(request.as_bytes()).unwrap();
        }

        println!("");

    }

    fn listen(&mut self, rx: Receiver<String>) {
        let stream = self.stream.try_clone().unwrap();
        let client_nick = self.nick.clone();
        let mut message = String::new();

        thread::spawn(move || loop {
            let buf_reader = BufReader::new(&stream);

            let requests = buf_reader.lines().map(|l| l.unwrap());

            for request in requests {
                if request.starts_with("SENT ") {
                    let mut request_words = request.split(' ');
                    let nick = request_words.nth(1).unwrap();
                    let msg = &request[4+nick.len()+2..];

                    writeln(format!("\r\x1b[K{nick}: {msg}"));
                    print!("{}> ", client_nick);
                    stdout().flush().unwrap();
                } else if request.starts_with("INTRODUCE ") {
                    let nick = match request.split(' ').nth(1) {
                        Some(s) => s,
                        None => continue
                    };

                    writeln(format!("\r\x1b[K{nick} connected to the server."));

                    if nick != client_nick {
                        print!("{}> ", client_nick);
                        stdout().flush().unwrap();
                    }
                } else if request.starts_with("GOODBYE ") {
                    let mut request_words = request.split(' ');
                    
                    let nick = match request_words.nth(1) {
                        Some(s) => s,
                        None => continue
                    };

                    let msg = match request_words.next() {
                        Some(s) if s != "" => {
                            format!("\r\x1b[K{nick} disconnected with message: {s}.")
                        },
                        None|Some(_) => { 
                            format!("\r\x1b[K{nick} disconnected from the server.")
                        }
                    };

                    writeln(msg);

                    print!("{}> ", client_nick);
                    stdout().flush().unwrap();
                }

                loop {
                    match rx.try_recv() {
                        Ok(s) => message = s,
                        Err(_) => break
                    }
                };

                print!("{}", message);
                stdout().flush().unwrap();
            }
        });

        thread::sleep(std::time::Duration::from_millis(250));
    }
}

fn writeln(s: String) {
    write!(stdout(), "{s}\r\n").unwrap();
}
