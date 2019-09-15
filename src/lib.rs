#![warn(clippy::all)]

use std::{
    sync::mpsc::{sync_channel, SyncSender, Receiver},
    default::Default,
    io::{BufReader, Read, Write},
    num::{Wrapping, NonZeroUsize},
};

mod err;
pub use crate::err::{Error, Result};

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellsLimit {
    /// The bool signifies whether to wrap or not
    limit: Option<(NonZeroUsize, bool)>,
}

impl CellsLimit {
    pub fn new(limit: Option<(NonZeroUsize, bool)>) -> Self {
        Self {
            limit
        }
    }
    pub fn limit(self) -> Option<usize> {
        self.limit.map(|(n, _)| n.get())
    }
    pub fn wraps(self) -> bool {
        self.limit.map(|(_, b)| b).unwrap_or(false)
    }
    #[inline]
    fn get_limit_if_wrap(self) -> Option<usize> {
        match self.limit {
            Some((n, true)) => Some(n.get()),
            _ => None,
        }
    }
}

pub struct State {
    cells: Vec<Wrapping<u8>>,
    cells_limit: CellsLimit,
    pub cell_pointer: usize,
    pub ongoing_loops: Vec<Command>,
    pub loop_nesting: u16,
    pub channel: (SyncSender<()>, Receiver<()>),
}

impl Default for State {
    #[inline]
    fn default() -> Self {
        State {
            cells: vec![Wrapping(0)],
            cells_limit: CellsLimit::default(),
            cell_pointer: 0,
            ongoing_loops: Vec::new(),
            loop_nesting: 0,
            channel: sync_channel(0),
        }
    }
}

impl State {
    #[inline]
    pub fn new(cells_limit: CellsLimit) -> Self {
        State{
            cells_limit,
            .. Self::default()
        }
    }
    pub fn get_cur(&self) -> Wrapping<u8> {
        self.cells.get(self.cell_pointer).copied().unwrap_or_default()
    }
    pub fn get_mut_cur(&mut self) -> &mut Wrapping<u8> {
        // Make sure the cells has allocated enough space
        if self.cells.len() <= self.cell_pointer {
            self.cells.resize(self.cell_pointer + 1, Wrapping(0));
        }
        // This is safe since we're checking above and making sure the `Vec` is big enough
        unsafe { self.cells.get_unchecked_mut(self.cell_pointer) }
    }
    pub fn pointer_add(&mut self) -> Result<()> {
        let (cp, overflow) = self.cell_pointer.overflowing_add(1);

        match self.cells_limit.limit {
            Some((lim, true)) => self.cell_pointer = cp % lim.get(),
            _ if overflow => return Err(Error::CellPointerOverflow),
            None => self.cell_pointer = cp,
            Some((lim, false)) => if cp >= lim.get() {
                return Err(Error::CellPointerOverflow)
            } else {
                self.cell_pointer = cp;
            }
        }

        Ok(())
    }
    pub fn pointer_sub(&mut self) -> Result<()> {
        let (cp, overflow) = self.cell_pointer.overflowing_sub(1);

        if overflow {
            if let Some(limit) = self.cells_limit.get_limit_if_wrap() {
                self.cell_pointer = limit - 1;
            } else {
                return Err(Error::CellPointerOverflow);
            }
        } else {
            self.cell_pointer = cp;
        }

        Ok(())
    }
    pub fn get_stop_sender(&self) -> SyncSender<()> {
        self.channel.0.clone()
    }
    pub fn cells_limit(&self) -> &CellsLimit {
        &self.cells_limit
    }
    pub fn cells(&self) -> CellsIter {
        CellsIter {
            size: self.cells_limit.limit().unwrap_or_else(|| self.cells.len()),
            inner: self.cells.iter(),
        }
    }
    pub fn evaluate(self) -> Result<CellsIntoIter> {
        let State{loop_nesting, cells, cells_limit, ..} = self; 
        if loop_nesting == 0 {
            Ok(CellsIntoIter {
                size: cells_limit.limit().unwrap_or_else(|| cells.len()),
                inner: cells.into_iter(),
            })
        } else {
            Err(Error::UnendedLoop)
        }
    }
}

pub struct CellsIter<'a> {
    inner: std::slice::Iter<'a, Wrapping<u8>>,
    size: usize, 
}

impl CellsIter<'_> {
    pub fn trim_end(&mut self) {
        while let Some(Wrapping(0)) = self.inner.as_slice().last() {
            self.inner.next_back();
        }
        self.size = self.inner.len();
    }
}

impl Iterator for CellsIter<'_> {
    type Item = u8;
    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.inner.next().map(|w| w.0);

        if self.size > 0 {
            self.size -= 1;
            if ret.is_none() {
                return Some(0);
            }
        }

        ret
    }
}

impl DoubleEndedIterator for CellsIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.size > self.inner.len() {
            self.size -= 1;
            Some(0)
        } else {
            self.size = self.size.saturating_sub(1);

            self.inner.next_back().map(|w| w.0)
        }
    }
}

impl ExactSizeIterator for CellsIter<'_> {
    fn len(&self) -> usize {
        self.size
    }
}

pub struct CellsIntoIter {
    inner: std::vec::IntoIter<Wrapping<u8>>,
    size: usize, 
}

impl CellsIntoIter {
    #[inline]
    pub fn as_ref(&self) -> CellsIter<'_> {
        CellsIter {
            inner: self.inner.as_slice().iter(),
            size: self.size
        }
    }
    pub fn trim_end(&mut self) {
        while let Some(Wrapping(0)) = self.inner.as_slice().last() {
            self.inner.next_back();
        }
        self.size = self.inner.len();
    }
}

impl Iterator for CellsIntoIter {
    type Item = u8;
    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.inner.next().map(|w| w.0);

        if self.size > 0 {
            self.size -= 1;
            if ret.is_none() {
                return Some(0);
            }
        }

        ret
    }
}

impl DoubleEndedIterator for CellsIntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.size > self.inner.len() {
            self.size -= 1;
            Some(0)
        } else {
            self.size = self.size.saturating_sub(1);

            self.inner.next_back().map(|w| w.0)
        }
    }
}

impl ExactSizeIterator for CellsIntoIter {
    fn len(&self) -> usize {
        self.size
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

                let cmds = replace(&mut state.ongoing_loops, Vec::new());
                let mut cur = state.get_cur();
                while cur != Wrapping(0) {
                    if let Ok(()) = state.channel.1.try_recv() {
                        return Err(Error::Stopped);
                    }
                    for &cmd in &cmds {
                        run_command(state, cmd, io)?;
                    }
                    cur = state.get_cur();
                }
            }
            _ => {
                state.loop_nesting -= 1;
                state.ongoing_loops.push(LoopEnd);
            }
        }
        LoopBegin => {
            state.loop_nesting += 1;
            if state.loop_nesting > 1 {
                state.ongoing_loops.push(LoopBegin);
            }
        }
        cmd if state.loop_nesting > 0 => state.ongoing_loops.push(cmd),
        PtrIncr => state.pointer_add()?,
        PtrDecr => state.pointer_sub()?,
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
