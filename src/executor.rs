
pub mod Executor {
    use std::thread::JoinHandle;
    use std::{process::Command, error::Error, thread};
    use crossbeam::channel::{Sender, Receiver};
    use pars_libs::{Remote, RemoteCommand};
    use std::sync::{ Arc, Mutex};
    use std::time::Duration;

    pub struct Worker {
        worker_id: usize,
        mode: i32,
        // runner: Option<JoinHandle<()>>,
        
        remote_addr: String,
        remote_port: u16,

        // receiver: Arc::<Mutex<Receiver<Vec<Vec<String>>>>>,
    }

    impl Worker {

        pub fn new(id: usize, halt_mode: i32 ) -> Worker {

            Worker { worker_id: id, mode: halt_mode
                , remote_addr: String::from("127.0.0.1"), remote_port: 0 }
        }

        pub fn execute(& self, running_flag: Arc::<Mutex<bool>>, rx: Arc::<Mutex<Receiver<Vec<Vec<String>>>>>) {

            let mode_clone = self.mode;
            let id_clone  = self.worker_id;
            
            thread::spawn(move || {

                    loop {
                        // println!("worker {id_clone}: waiting");
                        let cmd_line: Vec<Vec<String>>;
                        match rx.lock().unwrap().recv_timeout(Duration::from_millis(300)) {
                            Ok(tmp_line) => cmd_line = tmp_line,
                            Err(_) => continue,  
                        };

                        println!("cmd_line is assigned {:?} to worker {}", cmd_line, id_clone);

                        if running_flag.lock().unwrap().eq(& false) {
                            break;
                        }

                        // if cmd_line[0][0] == "QUIT" {
                        //     break;
                        // }
                        
                        // in Lazy mode, if this is true,
                        let mut eager_and_false = false;
                        
                        for each_cmd in cmd_line.into_iter() {
                        
                            // in Eager mode, need to check 
                            let ret = Command::new(&each_cmd[0])
                            .args(&each_cmd[1..])
                            .output()
                            .expect("Error execute");

                            if let Some(ret_code) = ret.status.code() {
                                if ret_code != 0 {
                                    if mode_clone != 0 {

                                        *running_flag.lock().unwrap() = false;
                                        // in Eager mode, the num_task is 0 immediately
                                        if mode_clone == 2 {
                                            eager_and_false = true;
                                            break; 
                                        }
                                    } 
                                    
                                    break;
                                } else {
                                    let out = String::from_utf8(ret.stdout);

                                    match out {
                                        Ok(v) => print!("RESULT: {v}"),
                                        Err(_) => {} ,
                                    }                                    
                                }
                            }

                            if eager_and_false {
                                break; 
                            }
                        }
                    }                    
                });

            // self.runner = Some(thr);
        }

        pub fn execute_remote(& self) {
            // let addr_string = self.remote_addr.clone();

            // let test_remote = Remote {
            //     addr: addr_string,
            //     port: self.remote_port,
            // };

            // let mut arg: Vec<String> = vec![String::from("ssh")
            // , String::from("-i"), String::from("/my_key"), String::from("-p")
            // , String::from("4444"), String::from("ls")];

            // let mut binding = Command::new(&arg[0]);

            // let cmd = binding.args(&arg[1..]);

            // let result = cmd.remote_output(&test_remote).expect("Remote error");

            // if let Ok(show_line) =    String::from_utf8(result.stdout) {
            //     println!("result of remote = {}", show_line);
            // }
        }

    }
}