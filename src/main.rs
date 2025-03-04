use pars_libs::{Remote, RemoteCommand, parse_line};
// use std::error::Error;
use std::io::{Write, Read};

use std::thread::{sleep};
use std::thread;
use std::time::Duration;
use std::{io, process, env};
use std::net::{TcpListener, TcpStream, ToSocketAddrs, SocketAddr};

use std::sync::{Arc, Mutex};

use crossbeam::channel::unbounded;
use crossbeam::channel::{Sender, Receiver};
mod executor;
mod tcps;

// cargo run -- -J 1 -e Never
// cargo run -- -J 1 -e Lazy | Eager
// cargo run -- -r localhost:12346/1

const PORT: u16 = 4466;

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

            pool = all_pools[0];
            // println!("server on the remote, pool is {:?}", pool);
            let remote_ip: String = String::from(pool.get_remote_ip());
            let n_workers = pool.get_workers();
            let mode = pool.get_mode();
            let remote_port = pool.get_remote_port();

            println!("remote_ip = {remote_ip}, remote_port = {remote_port}, n_worker = {n_workers}, mode = {mode}");
            thread::spawn(move || {
                executor::executor_helpers::wakeup_remote(remote_port
                            , remote_ip
                            , n_workers);                
            });
        }
        "-J" => {
            pool.init(&config_args[1..]);
            let (send_cmdx, recv_cmdx)
            :(Sender<Vec<Vec<String>>>, Receiver<Vec<Vec<String>>>)
             = unbounded();
        
            execute_local(&pool, recv_cmdx, send_cmdx);
        }
        _ => {}
    };


    // for the remote only
    // 6991 ./target/debug/pars --server-remote [num_worker] [mode]
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

        // it should finish!!!
        execute_remote(n_workers, mode, recv_cmdx, send_resp);

        let addr_server: SocketAddr = format!("127.0.0.1:{}", &PORT.to_string())
                                .parse().expect("SocketAddr Error");

        // println!("init_serv");
        // let serv_t = thread::spawn(move || {
        tcps::tcp_helpers::init_serv(addr_server, send_cmdx, recv_resp);
        // });

        // serv_t.join().unwrap();
        // create workers that takes commands from the channel
        return;
    }

    // local: client
    if config_args[1] == String::from("-r")  {
        println!("client: start trying to connect");
        sleep(Duration::from_secs(1));

        let addr_connect: SocketAddr = format!("{}:{}", "127.0.0.1", &PORT.to_string())
                                        .parse().expect("SocketAddr Error");

        tcps::tcp_helpers::connect_serv(addr_connect);
    }
}

fn execute_local(pool: & Pool, recv_cmdx: Receiver<Vec<Vec<String>>>, send_cmdx: Sender<Vec<Vec<String>>>) {
    let receiver = Arc::new(Mutex::new(recv_cmdx));
    let sender_placeholder  = Arc::new(Mutex::new(None));

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
        workers.push(executor::executor_helpers::Worker::new(i+1, mode) );
    }

    // let sender_worker_place  = Arc::new(Mutex::new(None));

    let flag_to_run = Arc::new(Mutex::new(true));
    for each_worker in workers.iter_mut() {
        each_worker.execute(Arc::clone(&flag_to_run), Arc::clone(&receiver)
        , true, Arc::clone(&sender_placeholder));
    }
    
    // start sending tasks
    reader(&send_cmdx);
    let _ = drop(send_cmdx);
    
    // println!("[MAIN] All tasks are finished");
    for each_worker in workers.iter_mut() {
        if let Some(t) = each_worker.runner.take() {
            t.join().unwrap();
        }
    }
}

// TODO: replace execute pool then
// It's only for remove now!!!!
// also send the result of execute to the sender
// start all the workers, they are waiting fot tasks
fn execute_remote(n_workers: i32, mode: i32, recv_cmdx: Receiver<Vec<Vec<String>>>, tx_resp: Sender<String> ) {
    let receiver = Arc::new(Mutex::new(recv_cmdx));
    let sender_worker  = Arc::new(Mutex::new(Some(tx_resp)));

    let mut workers : Vec<executor::executor_helpers::Worker> = Vec::new();

    // let (main_lock, cvar1) = &*signal_all_finish;
    println!("workers = {n_workers}, mode = {mode}");
    
    for i in 0..n_workers {
        workers.push(executor::executor_helpers::Worker::new(i+1,  mode) );
    }

    let flag_to_run = Arc::new(Mutex::new(true));

    for each_worker in workers.iter_mut () {
        each_worker.execute(Arc::clone(&flag_to_run)
        , Arc::clone(&receiver), false,  Arc::clone(&sender_worker) );
    }

    // for each_worker in workers.iter_mut() {
    //     if let Some(t) = each_worker.runner.take() {
    //         t.join().unwrap();
    //     }
    // }

    println!("finish executing remote init ");
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
            // executor::Executor::execute_command(&next_cmds);
            // println!("command {:?} is pushed", & next_cmds);
            let _ = sender.send(next_cmds);
        }            
    }

    // send to each thread to "Q"
    // // wait for all thread to join
}
