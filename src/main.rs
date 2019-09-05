#![warn(clippy::all)]

use clap::{App, Arg};
use std::fs::File;
use std::io::{stdin, stdout, Write};
use std::num::NonZeroUsize;

use brainfuck::Error::*;
use brainfuck::*;

fn run() -> Result<()> {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::with_name("SOURCE").help("Source code to run").required_unless("interactive"))
        .arg(
            Arg::with_name("interactive")
                .short("i")
                .long("interactive")
                .help("Starts interactive shell")
        )
        .arg(
            Arg::with_name("limit")
                .short("s")
                .long("size")
                .value_name("SIZE")
                .takes_value(true)
                .validator(|s| s.parse::<NonZeroUsize>().map(|_| ()).map_err(|_| "Has to be a number greater than zero".to_owned()))
                .help("The amount of cells that the program can use")
        )
        .arg(
            Arg::with_name("wrap")
                .short("w")
                .long("wrap")
                .help("Whether the cell pointer should wrap around the cell size")
                .requires("limit")
        )
        .get_matches();

    let limit = CellsLimit::new(if let Some(limit) = matches.value_of("limit") {
        let wrap = matches.is_present("wrap");

        Some((limit.parse::<NonZeroUsize>().unwrap(), wrap))
    } else {
        None
    });

    let mut state = State::new(limit);
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
            run_with_state(s.as_bytes(), &mut state, &mut stdouter)?;

            let cells_iter = state.cells();

            let n = (cells_iter.len() - cells_iter.rev().take_while(|&x| x == 0).count()).max(state.cell_pointer+1);

            if state.cell_pointer == 0 {
                print!("[")
            }
            for (i, byte) in state.cells().chain(std::iter::repeat(0)).take(n).enumerate() {
                print!("{:02x}", byte);
                if i == state.cell_pointer {
                    print!("]");
                } else if i+1 == state.cell_pointer {
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
        run_with_state(file, &mut state, &mut stdouter)?;
    }
    state.evaluate().map(std::mem::drop)
}

fn main() {
    match run() {
        Ok(()) => (), 
        Err(IoError(e)) => panic!("Unexpected error:\n{:?}", e),
        Err(Stopped) => (),
        Err(OutOfBounds) => eprintln!("Error, out of bounds"),
        Err(NoLoopStarted) => eprintln!("Error, cannot end a loop when none has been started"),
        Err(UnendedLoop) => eprintln!("Error, ended with unended loops"),
        Err(CellPointerOverflow) => eprintln!("Error, cell pointer overflowed limit"),
    }
}
