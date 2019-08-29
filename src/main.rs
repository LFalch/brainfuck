#![warn(clippy::all)]

use clap::{App, Arg};
use std::fs::File;
use std::io::{stdin, stdout, Write};

use brainfuck::Error::*;
use brainfuck::*;

fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::with_name("SOURCE").help("Source code to run").required_unless("interactive"))
        .arg(
            Arg::with_name("interactive")
                .short("i")
                .long("interactive")
                .help("Starts interactive shell"),
        )
        .get_matches();
    let mut state = State::default();
    let mut stdouter = InOuter::new(stdout(), stdin());

    if matches.is_present("interactive") {
        println!("Brainfuck Interactive Shell");
        println!("Type $exit to exit");
        loop {
            print!("$> ");
            stdout().flush().unwrap();

            let mut s = String::new();
            stdin().read_line(&mut s).unwrap();
            if s.trim_end() == "$exit" {
                println!();
                break;
            }
            match run_with_state(s.as_bytes(), &mut state, &mut stdouter) {
                Ok(()) => (),
                Err(e) => handle_error(e),
            }

            let n = (state.cells.len() - state.cells.iter().rev().take_while(|x| x.0 == 0).count()).max(state.pointer.0+1);

            if state.pointer.0 == 0 {
                print!("[")
            }
            for (i, byte) in state.cells.iter().take(n).map(|w| w.0).enumerate() {
                print!("{:02x}", byte);
                if i == state.pointer.0 {
                    print!("]");
                } else if i+1 == state.pointer.0 {
                    print!("[");
                } else {
                    print!(" ");
                }
            }
            println!();
        }
    } else {
        let src = matches.value_of("SOURCE").unwrap();

        let file = File::open(src).unwrap();
        match run_with_state(file, &mut state, &mut stdouter) {
            Ok(()) => (),
            Err(e) => handle_error(e),
        }
    }
}

fn handle_error(e: Error) {
    match e {
        IoError(e) => panic!("Unexpected error:\n{:?}", e),
        CharsError(e) => panic!("Unexpected error:\n{:?}", e),
        Exit => (),
        OutOfBounds => eprintln!("Error, out of bounds"),
        NoBlockStarted => eprintln!("Error, cannot end a block when none has been started"),
    }
}
