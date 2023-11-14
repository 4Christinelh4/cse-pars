use pars_libs::{Remote, RemoteCommand, parse_line};
// use std::error::Error;
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
mod tcps;

// cargo run pars -J 1 -e Never
// cargo run pars -J 1 -e Lazy | Eager
// cargo run pars -r localhost:12346/1

const PORT: u16 = 4444;

// terminal mode = 0: Never, 1: Lazy, 2: Eager
// Never by default
// when one worker execute the last task, the main needs to send message to ALL
// worker to let them quit
fn main() {
    let config_args: Vec<String> = env::args().collect(); 
    let mut pool = Pool::new();
    
    // -r localhost:12346/2  -e
    match config_args[2].as_str() {
        "-r" => {
            println!("start init remote");

            // lifetime!!!
            pool.remote_init(& config_args); 

            println!("server on the remote, pool is {:?}", pool);
            let remote_ip: String = String::from(pool.get_remote_ip());
            let n_workers = pool.get_workers();
            let remote_port = pool.get_remote_port();

            println!("remote_ip = {remote_ip}, remote_port = {remote_port}");
    
            // start the listening service on remote
            thread::spawn(move || {
                executor::executor_helpers::wakeup_remote(remote_port
                            , remote_ip
                            , n_workers);
            });
        }
        "-J" => {
            // println!("start init local");
            // pool.init();
            pool.init(&config_args[2..]);
            // println!("server on local, pool is {:?}", pool);
        }
        _ => {

        }
    };

    // for the remote only
    // pars --server-remote [num_worker] [mode]
    if &config_args[1] == &String::from("--server-remote") {
        println!("server is listening...");
        let n_workers: i32;
        let mode = 0;
        match config_args[2].as_str().parse::<i32>() {
            Ok(n) => n_workers = n,
            Err(_) => std::process::exit(1),
        }

        let (send_cmdx, recv_cmdx)
        :(Sender<Vec<Vec<String>>>, Receiver<Vec<Vec<String>>>) = unbounded();

        let (send_resp, recv_resp): (Sender<String>, Receiver<String>) = unbounded(); 

        workers_run(n_workers, mode, recv_cmdx, send_resp);
        let addr_server: SocketAddr = format!("127.0.0.1:{}", /* pool.get_remote_ip(), */ &PORT.to_string())
                                .parse().expect("SocketAddr Error");

        tcps::tcp_helpers::init_serv(addr_server, send_cmdx, recv_resp);

        // create workers that takes commands from the channel
        return;
    }

    // local: client
    if config_args[2] == String::from("-r")  {
        println!("client: start trying to connect");
        sleep(Duration::from_secs(1));
        let addr_connect: SocketAddr = format!("{}:{}", pool.get_remote_ip(), &PORT.to_string())
                                        .parse().expect("SocketAddr Error");

        tcps::tcp_helpers::connect_serv(addr_connect);
    } else if config_args[2] == String::from("-J") {
        execute_local(&pool);
    }
}

fn execute_local(pool: & Pool) {

    let (send_cmdx, recv_cmdx)
    :(Sender<Vec<Vec<String>>>, Receiver<Vec<Vec<String>>>)
     = unbounded();

    let receiver = Arc::new(Mutex::new(recv_cmdx));

    // this is the condvar that the main thread will wait on
    // let signal_all_finish = Arc::new((Mutex::new(false), Condvar::new()));

    let mut workers : Vec<executor::executor_helpers::Worker> = Vec::new();
    // let mut pool = Pool::new();
    // pool.init(&config_args);

    // // start all the workers, they are waiting fot tasks
    let n_workers = pool.get_workers();
    let mode = pool.get_mode();
    // let (main_lock, cvar1) = &*signal_all_finish;

    // println!("pool mode = {mode}");
    
    for i in 0..n_workers {
        workers.push(executor::executor_helpers::Worker::new(i+1,  mode) );
    }

    let flag_to_run = Arc::new(Mutex::new(true));
    for each_worker in workers.iter() {
        each_worker.execute(Arc::clone(&flag_to_run),Arc::clone(&receiver));
    }
    
    // start sending tasks
    reader(&send_cmdx);

    // let num_task = Arc::new(Mutex::new(total_tasks));
    // wait until received notification that all tasks are finished
    // while !num_task.lock().unwrap().eq(&0) {}

    // println!("[MAIN] All tasks are finished");
    // notify all threads
    // drop(send_cmdx);
}


// TODO: replace execute pool then
// It's only for remove now!!!!
// also send the result of execute to the sender
// start all the workers, they are waiting fot tasks
fn workers_run(n_workers: i32, mode: i32, recv_cmdx: Receiver<Vec<Vec<String>>>, tx_resp: Sender<String> ) {
    let receiver = Arc::new(Mutex::new(recv_cmdx));
    let sender_worker  = Arc::new(Mutex::new(tx_resp));

    let mut workers : Vec<executor::executor_helpers::Worker> = Vec::new();

    // let (main_lock, cvar1) = &*signal_all_finish;
    println!("workers = {n_workers}, mode = {mode}");
    
    for i in 0..n_workers {
        workers.push(executor::executor_helpers::Worker::new(i+1,  mode) );
    }

    let flag_to_run = Arc::new(Mutex::new(true));
    for each_worker in workers.iter() {
        each_worker.execute_remoteConnHandler(Arc::clone(&flag_to_run)
        , Arc::clone(&receiver), Arc::clone(&sender_worker) );
    }
}

#[derive(Debug,  Clone, Copy)]
struct Pool<'a> {
    n_workers: i32,
    terminal_mode: i32,
    remote_port: u16,
    remote_ip: &'a str,
}

impl<'a> Pool<'a> {
    fn new() -> Self {
       Pool { n_workers: (0), terminal_mode: (0), remote_port: (0), remote_ip: "", }
    }

    // -r localhost:12346/2 -e lazy
    fn remote_init(&mut self, args: &'a Vec<String> ) {
        println!("conf = {:?}", args);

        let split1: Vec<&str> = args[3].as_str().split(':').collect();
        self.remote_ip = split1[0];

        let split2: Vec<&str> = split1[1].split('/').collect();
        match &split2[0].parse::<u16>() {
            Ok(n) => self.remote_port = *n,
            Err(_) => std::process::exit(1),
        }

        match &split2[1].parse::<i32>() {
            Ok(n) => self.n_workers = *n,
            Err(_) => std::process::exit(1),
        }

        // match num_workers_string.parse::<i32>() {
        //     Ok(number) => self.n_workers = number as usize, 
        //     Err(e) => {
        //         // Handle the case where the conversion fails
        //         eprintln!("Error parsing string: {}", e);
        //     }
        // }
    }

    fn get_remote_port(&self) -> u16 {
        self.remote_port
    }

    fn get_remote_ip(&self) -> &str  {
        self.remote_ip
    }

    fn init(&mut self, conf: & [String]) {
        // let mut mode_map: collections::HashMap<&str, i32> = collections::HashMap::new();
        // mode_map.insert("never", 0);
        // mode_map.insert("lazy", 1);
        // mode_map.insert("eager", 2);

        for i in 0..conf.len() {
            match conf[i].as_str() {
                "-J" => {
                    //  self.n_workers
                    self.n_workers = conf[i+1].parse().expect("Error: num workers");
                }
                "-e" | "--halt" => {
                    // self.terminal_mode = * mode_map.get(conf[i+1].as_str().to_lowercase().as_str())
                    // .expect("Error: unexpected argument in mode");
                    match conf[i+1].to_lowercase().as_str() {
                        "lazy" => self.terminal_mode = 1,
                        "eager" => self.terminal_mode = 2,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    fn get_workers(self) -> i32 {
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
    let _ = drop(sender);
}
