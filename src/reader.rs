pub mod pars_reader {
    use std::io;
    use pars_libs::parse_line;
    use std::io::{Read, Write};
    use crossbeam::channel::Sender;

    // local reader
    // the server read and send commands to the stream
    pub fn reader_local(sender: &Sender<Vec<Vec<String>>>) {
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

    pub fn reader_remote_mode(sender: Sender<String>) {
        let mut read_input: String = String::new();
        loop {
    
            read_input.clear();
    
            let _ = io::stdin().read_line(& mut read_input);
    
            if read_input.trim().is_empty() {
                break;
            }

            // commands.push(String::from(read_input.trim()));
            let _ = sender.send(read_input.clone());  
        }
        drop(sender);
    }
}