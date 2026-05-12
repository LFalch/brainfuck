#![warn(clippy::all)]

use clap::Parser;
use std::fs::File;
use std::io::{stdin, stdout, BufReader, Write};
use std::num::NonZeroUsize;
use std::process::ExitCode;

use brainfuck::{run_with_state, CellsLimit, Error::*, InOuter, Result, State};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Source code to run
    #[arg(required_unless_present = "interactive")]
    source: Option<String>,

    /// Starts interactive shell
    #[arg(short, long)]
    interactive: bool,

    /// Dumps cell state after running to STDERR (file descriptor 2)
    #[arg(short, long, conflicts_with("interactive"))]
    dump: bool,

    /// The amount of cells that the program can use
    #[arg(short = 's', long = "size", value_name = "SIZE")]
    limit: Option<NonZeroUsize>,
    /// Whether the cell pointer should wrap around the cell size
    #[arg(short, long, requires = "limit")]
    wrap: bool,
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let limit = CellsLimit::new(cli.limit.map(|limit| (limit, cli.wrap)));

    let mut state = State::new(limit);
    let mut stdouter = InOuter::new(stdout(), stdin());

    if cli.interactive {
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

            let mut cells_iter = state.cells();
            cells_iter.trim_end();

            let n = (cells_iter.len()).max(state.cell_pointer + 1);

            if state.cell_pointer == 0 {
                print!("[");
            }
            for (i, byte) in state
                .cells()
                .chain(std::iter::repeat(0))
                .take(n)
                .enumerate()
            {
                print!("{byte:02x}");
                if i == state.cell_pointer {
                    print!("]");
                } else if i + 1 == state.cell_pointer {
                    print!("[");
                } else {
                    print!(" ");
                }
            }
            println!();
        }
    } else {
        let src = cli.source.unwrap();

        let file = BufReader::new(File::open(src).unwrap());
        run_with_state(file, &mut state, &mut stdouter)?;
    }
    let cp = state.cell_pointer;
    let cells = state.evaluate()?;
    if cli.dump {
        let mut space_before = false;
        for (i, cell) in cells.enumerate() {
            if i == cp {
                eprint!("[{cell:02x}]");
                space_before = false;
            } else {
                if space_before {
                    eprint!(" ");
                }
                space_before = true;
                eprint!("{cell:02x}");
            }
        }
        eprintln!();
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => return ExitCode::SUCCESS,
        Err(IoError(e)) => eprintln!("Unexpected error:\n{e:?}"),
        Err(Stopped) => eprintln!("Stopped"),
        Err(OutOfBounds) => eprintln!("Error, out of bounds"),
        Err(NoLoopStarted) => eprintln!("Error, cannot end a loop when none has been started"),
        Err(UnendedLoop) => eprintln!("Error, ended with unended loops"),
        Err(CellPointerOverflow) => eprintln!("Error, cell pointer overflowed limit"),
    }

    ExitCode::FAILURE
}
