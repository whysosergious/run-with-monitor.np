use std::{env, process::exit};
use tokio::{process::Command, sync::oneshot};

#[tokio::main]
async fn main() {
    /*
     * multiple commands and their arguments are separated with a `--` delimiter
     *
     * # example
     * ```
     * run_with_monitor deno repl -- sleep 2 -- nu -c "ls --du | sort-by size"
     * ```
     */
    let mut args: Vec<String> = env::args().collect();
    if args.len() < 1 {
        eprintln!("Please provide a command to run");
        exit(1);
    }

    /* remove the program name e.g the main cmd binary `run_with_callback` */
    args.remove(0);

    let mut commands: Vec<Vec<String>> = args
        .split(|arg| arg == "--")
        .map(|arg| arg.to_vec())
        .collect();
    // let shell_cmds = commands.remove(0);
    //
    // shell_init(shell_cmds).await;

    let callback = commands.remove(1);

    // Spawn a new task for running the commands
    let commands_handle = tokio::spawn(async move {
        run_command(commands).await;
    });

    sig_handler(callback, commands_handle).await;
}

async fn run_command(mut commands: Vec<Vec<String>>) {
    let cmd = commands[0].remove(0);
    let args = commands[0].drain(..).collect::<Vec<String>>();
    commands.remove(0); // remove the now empty vector

    /* create communication channel (tx: transmitter, rx: receiver) */
    let (tx, rx) = oneshot::channel();

    // spawn child process
    let child = spawn_process(&cmd, &args);

    // task to monitor the child process
    tokio::spawn(async move {
        println!("running main: {}", cmd);

        run_and_wait(child, &cmd).await;

        // do on final-finally (main task process is done)
        println!("killing main");

        // message main to kill task process
        let _ = tx.send(());

        println!("....\ndone\n");
    });

    // wait for murder
    let _ = rx.await;
    // exit(0)
}

fn spawn_process(cmd: &str, args: &[String]) -> tokio::process::Child {
    println!("spawn process for {}", cmd);
    Command::new(cmd)
        .args(args)
        // .raw_arg(args.join(" "))
        .spawn()
        .expect("Failed to start process")
}
async fn run_and_wait(mut child: tokio::process::Child, cmd: &str) {
    /* child process actions */
    let result = child.wait().await;

    match result {
        Ok(status) => {
            if status.success() {
                println!("{} process finished", cmd);
                // on gracefull
            } else {
                eprintln!("{} process failed. status: {}", cmd, status);
                // on not so gracefull
            }
        }
        Err(e) => {
            eprintln!("Failed to wait on {} process: {}", cmd, e);
            // on error
        }
    }

    println!("{} process terminated\n", cmd);
    // on finally
}
#[cfg(unix)]
async fn sig_handler(mut commands: Vec<Vec<String>>, commands_handle: tokio::task::JoinHandle<()>) {
    let sigterm =
        Some(tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap());

    let cleanup = async move {
        if !commands.is_empty() {
            let callback_cmd = commands.remove(0);
            let callback_args = commands.drain(..).collect::<Vec<String>>();
            commands.remove(0);

            let callback_child = spawn_process(&callback_cmd, &callback_args);

            println!(
                "running callback: {} {}",
                callback_cmd,
                callback_args.join(" ")
            );

            run_and_wait(callback_child, &callback_cmd).await;
        }
    };

    tokio::select! {
        _ = if let Some(mut sigterm) = sigterm {
            sigterm.recv().await
        }  => {
            println!("Received SIGTERM! Performing cleanup before exit.");
            cleanup.await;
        }
        _ = commands_handle => {
            // The commands task has finished
            task_done();
        }
    }
}

#[cfg(not(unix))]
async fn sig_handler(mut commands: Vec<String>, commands_handle: tokio::task::JoinHandle<()>) {
    // Asynchronously wait for a SIGINT signal
    let ctrl_c = tokio::signal::ctrl_c();

    let cleanup = async move {
        if !commands.is_empty() {
            let callback_cmd = commands.remove(0);
            // let callback_args = commands.drain(..).collect::<Vec<String>>();
            // commands.remove(0);

            let callback_child = spawn_process(&callback_cmd, &commands);

            println!("running callback: {}", callback_cmd);

            run_and_wait(callback_child, &callback_cmd).await;
        }
    };

    tokio::select! {
        _ = ctrl_c => {
            println!("\nrecieved sigint, cleanup before exit.");
            cleanup.await;
            task_done();

        }
        _ = commands_handle => {
            // The commands task has finished

            // cleanup.await;
            task_done();
        }
    }
}

fn task_done() {
    println!("All tasks have finished. Exiting.");
    // exit(0);
}
