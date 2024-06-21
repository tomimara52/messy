use std::error::Error;
use std::io::{self, stdin, stdout, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::thread;
use crossterm::terminal::{enable_raw_mode, disable_raw_mode, is_raw_mode_enabled};
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug)]
pub struct Client {
    stream: TcpStream,
    nick: String,
}

impl Client {
    pub fn new(addr: &str, nick: &str) -> Client {
        let stream = TcpStream::connect(addr).expect("Host not found");
        let nick = String::from(nick);

        Client { stream, nick }
    }

    pub fn connect(&mut self) {
        let request = String::from("CONNECT ") + &self.nick + "\n";
        self.stream.write(request.as_bytes()).unwrap();
        let (tx, rx) = mpsc::channel();
        self.listen(rx);

        if let Err(e) = self.input_loop(tx) {
            if is_raw_mode_enabled().expect("Error checking if raw mode is enabled.") {
                disable_raw_mode().expect("Error disabling raw mode.");
            }

            println!("Input loop failed with: {e}.");
            std::process::exit(1);
        }
    }

    fn input_loop(&mut self, tx: Sender<String>) -> Result<(), Box<dyn Error>> {
        let mut message = String::new();

        'outer: loop {
            let mut buf: [u8; 1] = [0];
            enable_raw_mode()?;

            loop { 
                stdin().read(&mut buf)?;

                // handle pressing ENTER
                if buf[0] == 10 || buf[0] == 13 {
                    break;
                }

                // handle pressing BACKSPACE
                if buf[0] == 127 {
                    if !message.is_empty() {
                        message.pop();
                        let b = 8 as char;
                        write!(stdout(), "{b} {b}")?;
                        stdout().flush()?;
                    }
                    continue;
                }

                // handle pressing CTRL-C
                if buf[0] == 3 {
                    disable_raw_mode()?;
                    break 'outer;
                }

                // handle rest of characters
                message.push(buf[0] as char);

                tx.send(message.clone())?;

                write!(stdout(), "{}", buf[0] as char)?;
                stdout().flush()?;
            }

            disable_raw_mode()?;

            message.push_str("\r\n");

            if message.starts_with("!quit") {
                message.replace_range(
                    0..5,
                    "DISCONNECT"
                );

                self.stream.write(message.as_bytes())?;
                break;
            }

            let request = String::from("SEND ") + &message;

            self.stream.write(request.as_bytes())?;

            message.clear();
            tx.send(String::new())?;

            print!("\r\n{}> ", self.nick);
            stdout().flush()?;
        }

        println!("");
        Ok(())
    }

    fn listen(&mut self, rx: Receiver<String>) {
        let stream = self.stream.try_clone().unwrap();
        let client_nick = self.nick.clone();

        thread::spawn(move || {
            if let Err(e) = listen_function(stream, client_nick, rx) {
                if is_raw_mode_enabled().expect("Error checking if raw mode is enabled.") {
                    disable_raw_mode().expect("Error disabling raw mode.");
                }

                println!("Listener loop failed with: {e}.");
                std::process::exit(1);
            }
        });

    }
}

fn writeln(s: String) -> Result<(), io::Error> {
    write!(stdout(), "{s}\r\n")
}

fn listen_function(
    stream: TcpStream,
    client_nick: String,
    rx: Receiver<String>
) -> Result<(), Box<dyn Error>> {
    let mut message = String::new();
    
    loop {
        let buf_reader = BufReader::new(&stream);

        let requests = buf_reader.lines().map(|l| l.unwrap());

        for request in requests {
            if request.starts_with("SENT ") {
                let mut request_words = request.split(' ');
                let nick = request_words.nth(1).unwrap();
                let msg = &request[4+nick.len()+2..];

                writeln(format!("\r\x1b[K{nick}: {msg}"))?;
            } else if request.starts_with("INTRODUCE ") {
                let nick = match request.split(' ').nth(1) {
                    Some(s) => s,
                    None => continue
                };

                writeln(format!("\r\x1b[K{nick} connected to the server."))?;
            } else if request.starts_with("GOODBYE ") {
                let mut request_words = request.split(' ');

                let nick = match request_words.nth(1) {
                    Some(s) => s,
                    None => continue
                };

                let msg = match request_words.fold(String::new(), |a, e| { a + " " + e }).trim() {
                    "" => { 
                        format!("\r\x1b[K{nick} disconnected from the server.")
                    },
                    s => {
                        format!("\r\x1b[K{nick} disconnected with message: {s}.")
                    }
                };

                writeln(msg)?;

            }

            // print prompt again
            print!("{}> ", client_nick);
            stdout().flush()?;

            // get the message the user was typing
            loop {
                match rx.try_recv() {
                    Ok(s) => message = s,
                    Err(_) => break
                }
            };

            // print the message the user was typing
            print!("{}", message);
            stdout().flush()?;
        }
    }
}
