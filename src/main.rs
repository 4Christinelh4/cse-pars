use executor::Executor::Worker;
use pars_libs::{Remote, RemoteCommand, parse_line};
use core::num;
use std::error::Error;
use std::io::{Write, Read};

use std::thread::{sleep};
use std::thread;
use std::time::Duration;
use std::{fs, io, process, collections, env};
use std::net::{TcpListener, TcpStream, ToSocketAddrs, SocketAddr};

use std::sync::{Arc, Mutex};

use crossbeam::channel::unbounded;
use crossbeam::channel::{Sender, Receiver};
mod executor;

// pars -J 1 -e Never
// pars -J 1 -e Lazy | Eager
// cargo run pars -r localhost:12346/1


// terminal mode = 0: Never, 1: Lazy, 2: Eager
// Never by default
// when one worker execute the last task, the main needs to send message to ALL
// worker to let them quit
fn main() {

    let config_args: Vec<String> = env::args().collect();  

    /* 
        cargo run pars server 4
        config_args = ["target/debug/pars", "pars", "server", "4"]
     */
    // println!("config_args = {:?}", config_args);


    
    if &config_args[2] == &String::from("-r")   {
        // start the same binary on remote
        println!("start init remote");

        let mut pool = Pool::new();
        pool.remote_init(&config_args[3]); 

        println!("server on the remote, pool is {:?}", pool);

        // start the listening service on remote
        thread::spawn(move || {
            executor::Executor::wakeup_remote(12346, String::from("127.0.0.1")
            , pool.get_workers(), 4445);
        });
    } 

    // for the remote only
    // pars --server-remote [num_worker] [mode]
    if &config_args[1] == & String::from("--server-remote") {
        println!("DEBUG: listening...");

        // init pool in server 

        let mut serv_pool = Pool::new();
        serv_pool.remote_init(&config_args[2]);

        println!("server: num worker = {}", &serv_pool.get_workers());

        let (send_cmdx, recv_cmdx)
        :(Sender<Vec<Vec<String>>>, Receiver<Vec<Vec<String>>>)
        = unbounded();

        let (send_resp, recv_resp): (Sender<String>, Receiver<String>) = unbounded(); 

        workers_run(&serv_pool, recv_cmdx, send_resp);

        init_serv(&send_cmdx, recv_resp);

        // create workers that takes commands from the channel
        
        return;
    }

    // local: client
    // client side
    println!("start try to connect");
    sleep(Duration::from_secs(2));

    if &config_args[2] == &String::from("-r")  {
        
        let mut client_stream;

        loop {
            match TcpStream::connect("127.0.0.1:4445") {
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
        return;
    }
}


fn init_serv(sender: & Sender<Vec<Vec<String>>>, rx_resp:  Receiver<String>) {


    println!("init_serv on remote");
    let addrs = [
        SocketAddr::from(([127, 0, 0, 1], 4445))
    ];

    let listener = TcpListener::bind(&addrs[..]).expect("Failed to bind to address");
    
    println!("Server listening on {:?}", addrs);

    // main: listen and send to channel 
    // workers: execute
    // loop listen
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // clone sender
                thread::scope( |s| {
                    s.spawn(||{
                        handle_connection(stream, &sender, &rx_resp);
                    });
                    
                });
            }

            Err(e) => {
                eprintln!("Error accepting client connection: {}", e);
            }
        }
    }
}

fn handle_connection(mut stream: TcpStream, sender: &Sender<Vec<Vec<String>>>, rx_resp: & Receiver<String>) {
    // use a thread to get string from receiver, and write to the stream
    // let rx_resp_arc = Arc::new(rx_resp);
    // let rx_resp_ = Arc::clone(&rx_resp_arc);

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
                eprintln!("Error reading from client: {}", e);
            }
        }

    }

}

// fn execute_local(pool: & Pool) {

//     let (send_cmdx, recv_cmdx)
//     :(Sender<Vec<Vec<String>>>, Receiver<Vec<Vec<String>>>)
//      = unbounded();

//     let receiver = Arc::new(Mutex::new(recv_cmdx));

//     // this is the condvar that the main thread will wait on
//     // let signal_all_finish = Arc::new((Mutex::new(false), Condvar::new()));

//     let mut workers : Vec<executor::Executor::Worker> = Vec::new();
//     // let mut pool = Pool::new();
//     // pool.init(&config_args);

//     // // start all the workers, they are waiting fot tasks
//     let n_workers = pool.get_workers();
//     let mode = pool.get_mode();
//     // let (main_lock, cvar1) = &*signal_all_finish;

//     println!("pool mode = {mode}");
    
//     for i in 0..n_workers {
//         workers.push(executor::Executor::Worker::new(i+1,  mode) );
//     }

//     let flag_to_run = Arc::new(Mutex::new(true));
//     for each_worker in workers.iter() {
//         each_worker.execute(Arc::clone(&flag_to_run),Arc::clone(&receiver));
//     }
    
//     // start sending tasks
//     println!("start reader");
//     reader(&send_cmdx);

//     // let num_task = Arc::new(Mutex::new(total_tasks));
//     // wait until received notification that all tasks are finished
//     // while !num_task.lock().unwrap().eq(&0) {}

//     println!("[MAIN] All tasks are finished");
//     // notify all threads
//     // drop(send_cmdx);
// }

// TODO: replace execute pool then
// It's only for remove now!!!!
// also send the result of execute to the sender
fn workers_run(pool: & Pool, recv_cmdx: Receiver<Vec<Vec<String>>>, tx_resp: Sender<String> ) {
    let receiver = Arc::new(Mutex::new(recv_cmdx));
    let sender_worker  = Arc::new(Mutex::new(tx_resp));



    let mut workers : Vec<executor::Executor::Worker> = Vec::new();
    // let mut pool = Pool::new();
    // pool.init(&config_args);

    // // start all the workers, they are waiting fot tasks
    let n_workers = pool.get_workers();
    let mode = pool.get_mode();
    // let (main_lock, cvar1) = &*signal_all_finish;
    println!("pool mode = {mode}");
    
    for i in 0..n_workers {
        workers.push(executor::Executor::Worker::new(i+1,  mode) );
    }

    let flag_to_run = Arc::new(Mutex::new(true));
    for each_worker in workers.iter() {
        each_worker.execute_remoteConnHandler(Arc::clone(&flag_to_run), Arc::clone(&receiver), Arc::clone(&sender_worker) );
    }
}


#[derive(Debug, Copy, Clone)]
struct Pool {
    n_workers: usize,
    terminal_mode: i32,
}

impl Pool {
    fn new() -> Self {
       Pool { n_workers: (0), terminal_mode: (0) }
    }

    fn remote_init(&mut self, num_workers_string: &String) {
        // println!("conf = {:?}", conf);

        match num_workers_string.parse::<i32>() {
            Ok(number) => self.n_workers = number as usize, 
            Err(e) => {
                // Handle the case where the conversion fails
                eprintln!("Error parsing string: {}", e);
            }
        }
    }

    fn init(&mut self, conf: & Vec<String>) {
        let mut mode_map: collections::HashMap<&str, i32> = collections::HashMap::new();
        mode_map.insert("never", 0);
        mode_map.insert("lazy", 1);
        mode_map.insert("eager", 2);

        for i in 0..conf.len() {
            match conf[i].as_str() {
                "-J" => {
                    //  self.n_workers
                    self.n_workers = conf[i+1].parse().expect("Error: num workers");
                }
                "-e" | "--halt" => {
                    self.terminal_mode = * mode_map.get(conf[i+1].as_str().to_lowercase().as_str())
                    .expect("Error: unexpected argument in mode");
                }
                _ => {}
            }
        }
    }

    fn get_workers(self) -> usize {
        self.n_workers
    }

    fn get_mode(self) -> i32 {
        self.terminal_mode
    }
}

fn reader(sender: & Sender<Vec<Vec<String>>>) {
    let mut read_input: String = String::new();

    loop {

        read_input.clear();

        let _ = io::stdin().read_line(& mut read_input);

        if read_input.trim().is_empty() {
            break;
        }
        // commands.push(String::from(read_input.trim()));
        let new_cmd_opt = parse_line(&read_input.as_str());
        if let Some(next_cmds) = new_cmd_opt{
            // // println!("cmd is {:?}", next_cmds);
            // executor::Executor::execute_command(&next_cmds);
            // println!("command {:?} is pushed", & next_cmds);
            let _ = sender.send(next_cmds);
        }            
    }


    drop(sender);
}


// one thread for read and one for write
fn client_handler(mut client_stream: TcpStream) {
    let mut read_input: String = String::new();

    // thread to read from the stream 
    let stream_arc = Arc::new(Mutex::new(client_stream));
    let mut stream_read_from_server = Arc::clone(&stream_arc);

    let thr = thread::spawn (move || {
        
        loop {
            let mut buffer_1 = [0; 512];
            let size: usize = match stream_read_from_server.lock().unwrap().read(&mut buffer_1 ) {
                Ok(l) => l,
                Err(e) => 0,
            };

            if size >0 {
                // Convert the received bytes to a String
                let response = String::from_utf8_lossy(&buffer_1[0..size]);
                println!("Response from server:\n{}", response);               
            }            
        }
    });
    

    loop {
        read_input.clear();
        let _ = io::stdin().read_line(& mut read_input);

        if read_input.trim().is_empty() {
            break;
        }

        println!("cmd = {}", read_input);

        let mut stream_write_ = stream_arc.lock().unwrap();

        stream_write_.write_all(read_input.as_bytes()).unwrap();
        drop(stream_write_)    ;
        println!("finish send cmd");
    }

    // thr.join().unwrap();
}


