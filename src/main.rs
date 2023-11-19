use std::thread::{sleep};
use std::thread;
use std::time::Duration;
use std::{io, process, env};
use std::net::{TcpStream, ToSocketAddrs, SocketAddr};

use std::sync::{Arc, Mutex};

use crossbeam::channel::unbounded;
use crossbeam::channel::{Sender, Receiver};
mod executor;
mod tcps;
mod reader;

// cargo run -- -J 1 -e Never
// cargo run -- -J 1 -e Lazy | Eager
// cargo run -- -r localhost:12346/1

const PORT: u16 = 4400;

// terminal mode = 0: Never, 1: Lazy, 2: Eager
// Never by default
// when one worker execute the last task, the main needs to send message to ALL
// worker to let them quit
fn main() {
    let mut config_args: Vec<String> = env::args().collect(); 
   // println!("args = {:?}", config_args);

    let mut pool = Pool::new();
    
    // -r localhost:12346/2  -e
    match config_args[1].as_str() {
        "-r" => {

            // lifetime!!!
            // pool.remote_init(& config_args); 
            let all_pools = init_pools(&mut config_args);
            for p in all_pools.into_iter() {
                // println!("server on the remote, pool is {:?}", pool);
                let remote_ip: String = String::from(p.get_remote_ip());
                let n_workers = p.get_workers();
                let mode = p.get_mode();
                let remote_port = p.get_remote_port();

                println!("remote_ip = {remote_ip}, remote_port = {remote_port}, n_worker = {n_workers}, mode = {mode}");

                thread::spawn(move || {
                    executor::executor_helpers::wakeup_remote(remote_port
                                , remote_ip
                                , n_workers, mode);                
                });
            }

            println!("server: start");
            let addr_server: SocketAddr = format!("127.0.0.1:{}", &PORT.to_string())
                                    .parse().expect("SocketAddr Error");

            let (send_cmdx, recv_cmdx)
                    :(Sender<String>, Receiver<String>) = unbounded();

            // let (send_resp, recv_resp): (Sender<String>, Receiver<String>) = unbounded(); 
    
            // println!("init_serv");

            // thread who read input: takes [send_cmdx]
            // every thread that for a stream: takes[recv_cmdx ]
            tcps::tcp_helpers::init_serv(addr_server, send_cmdx, recv_cmdx);
        }
        "-J" => {
            pool.init(&config_args[1..]);
            let (send_cmdx, recv_cmdx)
            :(Sender<Vec<Vec<String>>>, Receiver<Vec<Vec<String>>>)
             = unbounded();

            let n_workers = pool.get_workers();
            let mode = pool.get_mode();
        
            execute_local(n_workers, mode, recv_cmdx, send_cmdx);
        }
        _ => {}
    };


    // for the remote only
    // 6991 ./target/debug/pars --client-remote [num_worker] [mode]
    if &config_args[1] == &String::from("--client-remote") {
        let n_workers: i32;
        let mode: i32;
        
        match config_args[2].as_str().parse::<i32>() {
            Ok(n) => n_workers = n,
            Err(_) => std::process::exit(1),
        }

        match config_args[3].as_str().parse::<i32>() {
            Ok(n) => mode = n,
            Err(_) => std::process::exit(1),
        }

        sleep(Duration::from_secs(2));
    
        let addr_connect: SocketAddr = format!("{}:{}", "127.0.0.1", &PORT.to_string())
                                        .parse().expect("SocketAddr Error");

        if let Some(mut client_stream) = tcps::tcp_helpers::connect_serv(addr_connect) {
            println!("[REMOTE]: client connected, start execute_remote");

            let client_stream_clone = client_stream.try_clone().expect("TryClone failed...");

            let (send_cmdx, recv_cmdx)
                :(Sender<Vec<Vec<String>>>, Receiver<Vec<Vec<String>>>) = unbounded();

            execute_remote(n_workers, mode, recv_cmdx, client_stream_clone );

            // client_stream: receive from server
            println!("start client_runner");
            tcps::tcp_helpers::client_runner(&mut client_stream, send_cmdx);
        }
    }

    // local: server is now running 
}

fn execute_local(n_workers: i32, mode: i32, recv_cmdx: Receiver<Vec<Vec<String>>>
    , send_cmdx: Sender<Vec<Vec<String>>>) {
    
    let receiver = Arc::new(Mutex::new(recv_cmdx));
    let placeholder  = Arc::new(Mutex::new(None));

    // this is the condvar that the main thread will wait on
    // let signal_all_finish = Arc::new((Mutex::new(false), Condvar::new()));

    let mut workers : Vec<executor::executor_helpers::Worker> = Vec::new();

    for i in 0..n_workers {
        workers.push(executor::executor_helpers::Worker::new(i+1, mode) );
    }

    // let sender_worker_place  = Arc::new(Mutex::new(None));

    let flag_to_run = Arc::new(Mutex::new(true));
    for each_worker in workers.iter_mut() {
        each_worker.execute(Arc::clone(&flag_to_run), Arc::clone(&receiver)
        , true, Arc::clone(&placeholder));
    }
    
    // start sending tasks
    reader::pars_reader::reader_local(&send_cmdx);
    let _ = drop(send_cmdx);
    
    // println!("[MAIN] All tasks are finished");
    for each_worker in workers.iter_mut() {
        if let Some(t) = each_worker.runner.take() {
            t.join().unwrap();
        }
    }
}

// this is running remotly
// start all the workers, they are waiting fot tasks
// send the result to server directly
fn execute_remote(n_workers: i32, mode: i32, recv_cmdx: Receiver<Vec<Vec<String>>>, stream_to_local: TcpStream ) {
    let receiver = Arc::new(Mutex::new(recv_cmdx));
    let stream_arc_ = Arc::new(Mutex::new(Some(stream_to_local)));

    let mut workers : Vec<executor::executor_helpers::Worker> = Vec::new();

    // let (main_lock, cvar1) = &*signal_all_finish;
    println!("workers = {n_workers}, mode = {mode}");
    
    for i in 0..n_workers {
        workers.push(executor::executor_helpers::Worker::new(i+1,  mode) );
    }

    let flag_to_run = Arc::new(Mutex::new(true));

    for each_worker in workers.iter_mut () {
        each_worker.execute(Arc::clone(&flag_to_run)
        , Arc::clone(&receiver), false,  Arc::clone(&stream_arc_) );
    }

    println!("end of executing remote init ");
}

#[derive(Debug,  Clone, Copy)]
struct Pool<'a> {
    n_workers: i32,
    terminal_mode: i32,
    remote_port: u16,
    remote_ip: &'a str,
}

fn init_pools(conf: & mut Vec<String> ) -> Vec<Pool> {
    let mut pools: Vec<Pool> = Vec::new();
    
    for i in 0..conf.len() {
        match conf[i].as_str() {
            "-r" => {
                let mut p = Pool::new();
                p.remote_init(i, conf);

                pools.push(p);
            }
            _ => {}
        }
    }

    pools
}

impl<'a> Pool<'a> {
    fn new() -> Self {
       Pool { n_workers: (0), terminal_mode: (0), remote_port: (0), remote_ip: "", }
    }

    // pars -r localhost:12346/2 -e lazy -r localhost:12367/3 -e never -r localhost:12300/2 -e eager
    fn parse_url(&mut self, url: &'a String) {
        let split1: Vec<&str> = url.as_str().split(':').collect();

        if split1[0] == "localhost" {
            self.remote_ip = "127.0.0.1";
        } else {
            self.remote_ip = split1[0];
        }
        
        let split2: Vec<&str> = split1[1].split('/').collect();
        match &split2[0].parse::<u16>() {
            Ok(n) => self.remote_port = *n,
            Err(_) => std::process::exit(1),
        }

        match &split2[1].parse::<i32>() {
            Ok(n) => self.n_workers = *n,
            Err(_) => std::process::exit(1),
        }
    }

    fn parse_mode(&mut self, conf: &str) {
        match conf.to_lowercase().as_str() {
            "lazy" => self.terminal_mode = 1,
            "eager" => self.terminal_mode = 2,
            "never" => self.terminal_mode = 0,
            _ => {
                println!("Error in termination mode");
                std::process::exit(1);
            }
        }
    }

    fn check_arg(& self, idx: usize, total_len: usize ) {
        if idx+1 == total_len {
            println!("Error in argument");
            std::process::exit(1);
        }
    }

    fn remote_init(&mut self, idx: usize, conf: &'a Vec<String> ) {
        let mut cnt = 0;
        for i in idx..conf.len() {
            match  conf[i].as_str() {
                "-r" | "--remote" => {
                    self.check_arg(i, conf.len());
                    if cnt == 1 { return ; }
                    cnt = 1;

                    let url_ = & conf[i+1];
                    self.parse_url(url_);
                }
                "-e" | "--halt" => {
                    self.check_arg(i, conf.len());
                    self.parse_mode(conf[i+1].as_str());
                }
                _ => {}
            }
        }
    }

    fn get_remote_port(&self) -> u16 {
        self.remote_port
    }

    fn get_remote_ip(&self) -> &str  {
        self.remote_ip
    }

    fn init(&mut self, conf: & [String]) {

        for i in 0..conf.len() {
            match conf[i].as_str() {
                "-J" => {
                    self.check_arg(i, conf.len());
                    //  self.n_workers
                    self.n_workers = conf[i+1].parse().expect("Error: num workers");
                }
                "-e" | "--halt" => {
                    self.check_arg(i, conf.len());
                    // self.terminal_mode = * mode_map.get(conf[i+1].as_str().to_lowercase().as_str())
                    // .expect("Error: unexpected argument in mode");
                    self.parse_mode(conf[i+1].as_str());
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
