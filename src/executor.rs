pub mod executor_helpers {
    use std::thread::JoinHandle;
    use std::{process::Command, thread};
    use crossbeam::channel::{Sender, Receiver};
    use pars_libs::{Remote, RemoteCommand};
    use std::sync::{ Arc, Mutex};
    use std::net::TcpStream;
    use std::io::Write;
    
    pub struct Worker {
        worker_id: i32,
        mode: i32,
        pub runner: Option<JoinHandle<()>>,
    }

    impl Worker {
        pub fn new(id: i32, halt_mode: i32 ) -> Worker {
            Worker { worker_id: id, mode: halt_mode, runner: None }
        }

        /*
            pub fn execute_remoteConnHandler(& self, running_flag: Arc::<Mutex<bool>>
            , rx: Arc::<Mutex<Receiver<Vec<Vec<String>>>>>, tx_main: Arc::<Mutex<Sender<String>>>) 

            pub fn execute(& self, running_flag: Arc::<Mutex<bool>>, rx: Arc::<Mutex<Receiver<Vec<Vec<String>>>>>, is_remote: bool
            , tx_main_option: Arc::<Mutex<Option<Sender<String>>>> )
         */
        pub fn execute(&mut self, running_flag: Arc::<Mutex<bool>>
            , rx: Arc::<Mutex<Receiver<Vec<Vec<String>>>>>, on_local: bool
            , stream_main: Arc::<Mutex<Option<TcpStream>>>) {

            let mode_clone = self.mode;
            let id_clone  = self.worker_id;
            
            let thr = thread::spawn(move || {

                    loop {

                        if running_flag.lock().unwrap().eq(& false) {
                            break;
                        }                        
                        // println!("worker {id_clone}: waiting");
                        let cmd_line: Vec<Vec<String>>;
                        match rx.lock().unwrap().recv( ) {
                            Ok(tmp_line) => cmd_line = tmp_line,
                            Err(_) => break, 
                        };

                        // println!("cmd_line is assigned {:?} to worker {}", cmd_line, id_clone);
                        // in Lazy mode, if this is true,
                        let mut eager_and_false = false;
                        let mut all_out: Vec<String> = Vec::new();
                        
                        // print together
                        for each_cmd in cmd_line.into_iter() {
                            
                            // in Eager mode, all commands after that /false should not be executed
                            // in Lazy mode, the lines other than the /false will  still be executed, 
                            // therefore, in lazy mode, the break will NOT happen
                            if running_flag.lock().unwrap().eq(& false) && 2 == mode_clone{
                                break;
                            }
                            
                            // println!("=================each_cmd: {:?}===============", each_cmd);
                        
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
                                        // if mode_clone > 0 {
                                            // if it's 1 or 2, other command after the line will not be executed
                                        eager_and_false = true;
                                        // }
                                    } 

                                    break;
                                } else {
                                    let out = String::from_utf8(ret.stdout).unwrap();
                                    all_out.push(out);

                                    
                                    // print!("{}", String::from_utf8(ret.stdout).unwrap());       
                                }
                            }
                        }

                        if on_local {
                            for string_out in all_out.iter() {
                                print!("{string_out}");
                            }

                        } else {

                            for string_out in all_out.iter() {
                                let id = String::from("[REMOTE] ");
                                // let out = String::from_utf8(ret.stdout).unwrap();

                                let to_send = id+&string_out ;
                                println!("worker {id_clone} send result to main {to_send}");

                                if !to_send.is_empty() {
                                    let mut stream_write_opt  = stream_main.lock().unwrap();
                                    if let Some(stream_write) = &mut *stream_write_opt {
                                        stream_write.write_all(to_send.as_bytes()).unwrap();
                                        
                                        println!("finish sending to main");  
                                    }
                                    drop(stream_write_opt)    ;
                                }
                            }

                        }

                        // in eager mode, all commands after this line will not be executed
                        if eager_and_false {
                            break;  // break the loop
                        }
                    }
                });

            self.runner = Some(thr);
        }
    }


    

    // this is how to wake up the remote client
    pub fn wakeup_remote(port_: u16,  addr_: String, n_workers: i32 , mode: i32) {
        println!("execute wakeup_remote");
        let test_remote = Remote {
            addr: addr_,
            port: port_,
        };

        let arg_to_run = vec![ String::from("pars") , String::from("--client-remote")
        , n_workers.to_string(), mode.to_string() ];

        // let mut cmd_obj = Command::new(&arg_to_run[0]);
        // let cmd_withargs = cmd_obj.args(&arg_to_run[1..]);

        // let result = cmd_withargs.remote_output(&test_remote).expect("Remote error");

        let _ =  Command::new(&arg_to_run[0])
                        .args(&arg_to_run[1..])
                        .remote_output(&test_remote).expect("Remote error"); 

                    
    }
}