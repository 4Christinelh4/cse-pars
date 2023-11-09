use executor::Executor::Worker;
use pars_libs::{Remote, RemoteCommand, parse_line};
use std::thread::{sleep};
use std::time::Duration;
use std::{io, process, collections, env};

use std::sync::{Arc, Mutex};

use crossbeam::channel::unbounded;
use crossbeam::channel::{Sender, Receiver};

mod executor;

// pars -J 1 -e Never
// pars -J 1 -e Lazy | Eager

// terminal mode = 0: Never, 1: Lazy, 2: Eager
// Never by default
// when one worker execute the last task, the main needs to send message to ALL
// worker to let them quit
fn main() {
    let config_args: Vec<String> = env::args().collect();

    let (send_cmdx, recv_cmdx)
    :(Sender<Vec<Vec<String>>>, Receiver<Vec<Vec<String>>>)
     = unbounded();

    let receiver = Arc::new(Mutex::new(recv_cmdx));

    // this is the condvar that the main thread will wait on
    // let signal_all_finish = Arc::new((Mutex::new(false), Condvar::new()));

    let mut workers : Vec<executor::Executor::Worker> = Vec::new();
    let mut pool = Pool::new();
    pool.init(&config_args);

    // start all the workers, they are waiting fot tasks
    let n_workers = pool.get_workers();
    let mode = pool.get_mode();
    // let (main_lock, cvar1) = &*signal_all_finish;

    println!("pool mode = {mode}");
    
    for i in 0..n_workers {
        workers.push(executor::Executor::Worker::new(i+1,  mode) );
    }

    let flag_to_run = Arc::new(Mutex::new(true));
    for each_worker in workers.iter() {
        each_worker.execute(Arc::clone(&flag_to_run),Arc::clone(&receiver));
    }
    
    // start sending tasks
    println!("start reader");
    reader(&send_cmdx);

    // let num_task = Arc::new(Mutex::new(total_tasks));
    // wait until received notification that all tasks are finished
    // while !num_task.lock().unwrap().eq(&0) {}

    println!("[MAIN] All tasks are finished");
    // notify all threads
    // drop(send_cmdx);

    // for i in 0..n_workers {
    //     workers[i].release();
    // }
}

#[derive(Copy, Clone)]
struct Pool {
    n_workers: usize,
    terminal_mode: i32,
}

impl Pool {
    fn new() -> Pool {
       Pool { n_workers: (0), terminal_mode: (0) }
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


