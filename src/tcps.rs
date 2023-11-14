pub mod tcp_helpers {
    use std::net::{TcpListener, TcpStream, SocketAddr};
    use std::{io, thread, time::Duration};
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};
    use pars_libs::parse_line;
    // use crossbeam::channel::unbounded;
    use crossbeam::channel::{Sender, Receiver};

    pub fn init_serv(server_addr: SocketAddr, sender: Sender<Vec<Vec<String>>>
        , rx_resp: Receiver<String>) {

        // let share_sender = Arc::new(Mutex::new(sender) );
        println!("init_serv on remote");
        // let addrs = [
        //     SocketAddr::from(([127, 0, 0, 1], PORT))
        // ];
    
        // let listener = TcpListener::bind(&addrs[..]).expect("Failed to bind to address");
        let listener = TcpListener::bind(&server_addr).expect("Failed to bind to address");
        println!("Server listening on {:?}", server_addr);
    
        // main: listen and send to channel 
        // workers: execute
        // loop listen
        for stream in listener.incoming() {
            thread::scope( |s| {
                s.spawn( ||{
                    handle_connection(stream.unwrap(), &sender, &rx_resp);
                });
            });
        }
    }

    pub fn connect_serv(remote_addr: SocketAddr) {
        let mut client_stream;

        loop {
            match TcpStream::connect(remote_addr) {
                Ok(stream) => {
                    client_stream = stream;
                    break;
                }
                Err(_) => {
                    println!("not connected, please retry");
                }
            }
        }

        println!("connected to server, start to send cmds");
        client_stream.set_nonblocking(true).expect("set_nonblocking call failed");

        // client_stream.set_read_timeout(Some(Duration::from_secs(1))).expect("set_read_timeout call failed");
        // client_stream.set_write_timeout(Some(Duration::from_secs(1))).expect("set_write_timeout call failed");
        // read input + send to remote
        client_handler(client_stream);
    }

    fn handle_connection(mut stream: TcpStream, sender: &Sender<Vec<Vec<String>>>
        , rx_resp: & Receiver<String>) {

        // use a thread to get string from receiver, and write to the stream
        // let rx_resp_arc = Arc::new(rx_resp);
        // let rx_resp_ = Arc::clone(&rx_resp_arc);
        stream.set_read_timeout(Some(Duration::from_secs(1))).expect("set_read_timeout call failed");
        loop {
            loop {
                match rx_resp.recv_timeout(Duration::from_millis(150)) {
                    Ok(tmp) => { 
                        println!("receive from worker: {tmp}");
                        stream.write_all(tmp.as_bytes()).unwrap();
                    },
                    Err(_) => {
                        break;
                    },
                };
            }

            let mut buffer = [0; 512];
            // Read data from the stream
            match & stream.read(&mut buffer) {
                Ok(size) => {
                    // Echo the received data back to the client
                    // stream.write_all(b"this is RESPONSE from the server").unwrap(); 
                    println!("Server: size = {size}");
                    if let Ok(buf_to_str) = std::str::from_utf8(&buffer[0..*size]) {
                        if let Some(next_cmds) = parse_line(&buf_to_str){
                            // // println!("cmd is {:?}", next_cmds);
                            // executor::Executor::execute_command(&next_cmds);
                            // println!("command {:?} is pushed", & next_cmds);

                            println!("cmd to send = {:?}", next_cmds);
                            let _ = sender.send(next_cmds);
                        }
                    } 
                }
                Err(e) => {
                    // eprintln!("Error reading from client: {}", e);
                }
            }
        }
    }

    // one thread for read and one for write
    fn client_handler(client_stream: TcpStream) {
        let mut read_input: String = String::new();

        // thread to read from the stream 
        let stream_arc = Arc::new(Mutex::new(client_stream));
        let stream_read_from_server = Arc::clone(&stream_arc);

        thread::spawn (move || {
            
            loop {
                let mut buffer_1 = [0; 512];
                let size: usize = match stream_read_from_server.lock().unwrap().read(&mut buffer_1 ) {
                    Ok(l) => l,
                    Err(_) => 0,
                };

                if size > 0 {
                    // Convert the received bytes to a String
                    let response = String::from_utf8_lossy(&buffer_1[0..size]);
                    println!("{response}");               
                }            
            }
        });
        
        loop {
            read_input.clear();
            let _ = io::stdin().read_line(& mut read_input);

            if read_input.trim().is_empty() {
                break;
            }

            print!("cmd = {}", read_input);

            let mut stream_write_ = stream_arc.lock().unwrap();
            stream_write_.write_all(read_input.as_bytes()).unwrap();
            drop(stream_write_)    ;
            println!("finish send cmd");
        }
        // thr.join().unwrap();
    }
}