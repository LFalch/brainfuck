#![warn(clippy::all)]

use std::{
    sync::mpsc::{channel, Sender, Receiver},
    default::Default,
    io::{BufReader, Read, Write},
    num::Wrapping,
};

mod err;
pub use crate::err::{Error, Result};

#[derive(Clone, PartialEq)]
pub enum Command {
    PtrIncr,
    PtrDecr,
    Incr,
    Decr,
    Out,
    In,
    LoopBegin,
    LoopEnd,
}

impl Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            Incr => "+",
            Decr => "-",
            PtrIncr => ">",
            PtrDecr => "<",
            Out => ".",
            In => ",",
            LoopBegin => "[",
            LoopEnd => "]",
        })
    }
}

use std::fmt::{self, Debug};
use self::Command::*;

impl Command {
    pub fn from_byte(cmd: u8) -> Option<Self> {
        Some(match cmd {
            b'+' => Incr,
            b'-' => Decr,
            b'>' => PtrIncr,
            b'<' => PtrDecr,
            b'.' => Out,
            b',' => In,
            b'[' => LoopBegin,
            b']' => LoopEnd,
            _ => return None
        })
    }
}

pub struct State {
    pub cells: [Wrapping<u8>; 256],
    pub pointer: Wrapping<usize>,
    pub temp: Vec<Command>,
    pub loop_nesting: u8,
    pub channel: (Sender<()>, Receiver<()>),
}

impl Default for State {
    fn default() -> Self {
        State {
            cells: [Wrapping(0); 256],
            pointer: Wrapping(0),
            temp: Vec::new(),
            loop_nesting: 0,
            channel: channel(),
        }
    }
}

impl State {
    pub fn get_cur(&self) -> Wrapping<u8> {
        self.cells[self.pointer.0]
    }
    pub fn get_mut_cur(&mut self) -> &mut Wrapping<u8> {
        &mut self.cells[self.pointer.0]
    }
    pub fn pointer_add(&mut self) {
        self.pointer += Wrapping(1);
        self.pointer %= Wrapping(self.cells.len());
    }
    pub fn pointer_sub(&mut self) {
        self.pointer -= Wrapping(1);
        self.pointer %= Wrapping(self.cells.len());
    }
    pub fn get_stop_sender(&self) -> Sender<()> {
        self.channel.0.clone()
    }
}

pub struct InOuter<W: Write, R: Read> {
    o: W,
    i: BufReader<R>,
}

impl<W: Write, R: Read> InOuter<W, R> {
    pub fn new(o: W, i: R) -> Self {
        InOuter { o, i: BufReader::new(i) }
    }
    pub fn extract(self) -> (W, R) {
        let InOuter { i, o } = self;
        (o, i.into_inner())
    }
}

pub fn run_with_state<R, R2, W>(src: R, state: &mut State, io: &mut InOuter<W, R2>) -> Result<()>
where
    R: Read,
    R2: Read,
    W: Write,
{
    for cmd in src.bytes().map(|b| b.map(Command::from_byte)) {
        if let Ok(()) = state.channel.1.try_recv() {
            return Err(Error::Stopped);
        }
        match cmd {
            Ok(cmd) => {
                if let Some(cmd) = cmd {
                    run_command(state, cmd, io)?;
                }
            }
            Err(e) => return Err(Error::IoError(e)),
        }
    }

    Ok(())
}

use std::mem::replace;

fn run_command<W: Write, R: Read>(state: &mut State, cmd: Command, io: &mut InOuter<W, R>) -> Result<()> {
    match cmd {
        LoopEnd => match state.loop_nesting {
            0 => return Err(Error::NoLoopStarted),
            1 => {
                state.loop_nesting = 0;

                let cmds = replace(&mut state.temp, Vec::new());
                let mut cur = state.get_cur();
                while cur != Wrapping(0) {
                    if let Ok(()) = state.channel.1.try_recv() {
                        return Err(Error::Stopped);
                    }
                    for cmd in &cmds {
                        run_command(state, cmd.clone(), io)?;
                    }
                    cur = state.get_cur();
                }
            }
            _ => {
                state.loop_nesting -= 1;
                state.temp.push(LoopEnd);
            }
        }
        LoopBegin => {
            state.loop_nesting += 1;
            if state.loop_nesting > 1 {
                state.temp.push(LoopBegin);
            }
        }
        ref cmd if state.loop_nesting > 0 => state.temp.push(cmd.clone()),
        PtrIncr => state.pointer_add(),
        PtrDecr => state.pointer_sub(),
        Incr => *state.get_mut_cur() += Wrapping(1),
        Decr => *state.get_mut_cur() -= Wrapping(1),
        Out => io.o.write_all(&[state.get_cur().0])?,
        In => {
            let mut byte = [0];
            io.i.read_exact(&mut byte)?;
            *state.get_mut_cur() = Wrapping(byte[0]);
        }
    }

    Ok(())
}
