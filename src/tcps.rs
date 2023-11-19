pub mod tcp_helpers {
    use std::net::{TcpListener, Shutdown, TcpStream, SocketAddr};
    use std::{io, thread, time::Duration};
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};
    use pars_libs::parse_line;
    use crossbeam::channel::{Sender, Receiver};
    use crate::reader;
    
    pub fn init_serv(server_addr: SocketAddr, cmd_sender: Sender<String>
            , cmd_receiver: Receiver<String>) {

        // let share_sender = Arc::new(Mutex::new(sender) );
        println!("init_serv locally ");
        // let addrs = [
        //     SocketAddr::from(([127, 0, 0, 1], PORT))
        // ];
    
        // let listener = TcpListener::bind(&addrs[..]).expect("Failed to bind to address");
        let listener = TcpListener::bind(&server_addr).expect("Failed to bind to address");
        println!("Server listening on {:?}", server_addr);

        let t_reader = thread::spawn(move || {
            reader::pars_reader:: reader_remote_mode(cmd_sender);
        });
        

        let rx_resp_arc = Arc::new(cmd_receiver);
        
        // main: listen and send to channel 
        // workers: execute
        // loop listen
        for stream in listener.incoming() {
            thread::scope( |s| {
                println!("connection received");
                let rx_resp_ = Arc::clone(&rx_resp_arc);
                s.spawn( || {
                    // read from stdin, send to the stream to the client
                    // different threads, different streams
                    handle_connection(stream.unwrap(), rx_resp_ );
                });
            });
        }
        
        t_reader.join().unwrap();
        println!("server finished") ;
    }

    // stream: connect with the client,  only send what received from rx_resp to the client
    // the function is also responsible to close the stream
    // multiple connection-> add Mutex on rx_resp (part 2.3)
    fn handle_connection(mut stream: TcpStream, rx_resp: Arc::<Receiver<String>>) {
        // use a thread to get string from receiver, and write to the stream
        stream.set_read_timeout(Some(Duration::from_millis(500))).expect("set_read_timeout call failed");
        // also need to read from the stream
        let mut stop_sending_cmds = false;
        loop {
            while ! stop_sending_cmds {
                match rx_resp.recv_timeout(Duration::from_millis(500)) {
                    Ok(tmp) => { 
                        // println!("receive from main thread: {:?}", tmp);
                        if tmp == "quit" {
                            println!("handler: quit received");
                            stop_sending_cmds = true;
                            break;
                        }
                        stream.write_all(tmp.as_bytes()).unwrap();
                        // println!("finish write");
                    },
                    Err(_) => break,
                };
            }


            let mut buffer = [0; 512];
            // Read data from the stream
            match & stream.read(&mut buffer) {
                Ok(size) => {
                    // Echo the received data back to the client
                    // stream.write_all(b"this is RESPONSE from the server").unwrap(); 
                    let size_ = *size;

                    if size_ > 0 {
                        if let Ok(buf_to_str) = std::str::from_utf8(&buffer[0..size_]) {
                            print!("[REMOTE]: {buf_to_str}");
                        }                         
                    }
                }
                Err(_) => {
                    // eprintln!("Error reading from client: {}", e);
                }
            }
        }
    }

    // remote  client connect to server 
    pub fn connect_serv(remote_addr: SocketAddr) ->  Option<TcpStream>{
        // loop {
        match TcpStream::connect(remote_addr) {
            Ok(stream) => {
                println!("connected to the server!");
                stream.set_read_timeout(Some(Duration::from_millis(500)))
                        .expect("set_read_timeout call failed");
                
                Some(stream)
            }
            Err(_) => {
                println!("not connected, please retry");
                None
            }
        }
        // }

        // client_stream.set_nonblocking(true).expect("set_nonblocking call failed");

        // the stream will read every command from the server, send to the worker, every worker will send the result back

        
        // client_stream.set_write_timeout(Some(Duration::from_secs(1))).expect("set_write_timeout call failed");
        // read input + send to remote
        // client_handler(client_stream);
    }

    // this is running on client side, it dispatch all commands received from the stream
    pub fn client_runner(client_stream: &mut TcpStream, sender: Sender<Vec<Vec<String>>>) {
        loop {
            let mut buffer = [0; 512];
            match &client_stream.read(&mut buffer) {
                Ok(size) => {
                    // println!("client reveive command, size = {size}");
                    if let Ok(buf_to_str) = std::str::from_utf8(&buffer[0..*size]) {
                        if let Some(next_cmds) = parse_line(&buf_to_str){
                            // println!("cmd is {:?}", next_cmds);
                            // send cmd to workers 
                            let _ = sender.send(next_cmds);
                        }
                    }
                }
                Err(_) =>  {}
            }
        }
    }       

}