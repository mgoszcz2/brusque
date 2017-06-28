#[macro_use]
extern crate pest;
extern crate docopt;
extern crate num;

use std::io::prelude::*;
use std::fs::File;
use std::collections::HashMap;
use std::default::Default;

use docopt::Docopt;
use pest::prelude::*;
use num::bigint::BigUint;
use num::Zero;

const USAGE: &'static str = "
Usage:
  brusque [-v] <tm2>
  brusque -h | --help

Options:
  -h --help  Show this screen.
  -v         Print all states
";

type StateNumber = usize;

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    L,
    R,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Symbol {
    A,
    B,
}

#[derive(Debug)]
pub struct TransitionInfo {
    next: String,
    mov: Direction,
    write: Symbol,
}

#[derive(Debug)]
pub struct StateInfo {
    on_a: TransitionInfo,
    on_b: TransitionInfo,
    start: bool,
    name: String
}

#[derive(Debug)]
pub struct Transition {
    next: StateNumber,
    mov: Direction,
    write: Symbol,
}

#[derive(Debug)]
pub struct State {
    on_a: Transition,
    on_b: Transition,
}

#[derive(Debug, Default)]
pub struct Tm {
    states: Vec<State>,
    start_state: Option<StateNumber>,
}

impl_rdp! {
    grammar! {
        whitespace = _{ [" "] | ["\t"] } // Magic
        nl = _{ ["\n"] }

        number = @{ (["0"] | ['1'..'9'] ~ ['0'..'9']*) }
        state_name = @{ (['a'..'z'] | ['A'..'Z'] | ["_"] | ['0'..'9'] | ["."])+ }
        tm_alphabet = @{ ['a'..'b'] }
        head_direction = @{ ["R"] | ["L"] | ["-"] }
        start = @{ [i"START"] ~ whitespace }

        transition = _{ tm_alphabet ~ ["->"] ~ state_name ~ [";"] ~ head_direction ~ [";"] ~ tm_alphabet ~ nl }
        state = _{ start? ~ state_name ~ [":"] ~ nl ~ transition ~ transition ~ nl* }
        header = _{ [i"states:"] ~ number ~ nl+ }
    }

    process! {
        _transition(&self) -> TransitionInfo {
            (_: tm_alphabet, &next: state_name, &mov: head_direction, &write: tm_alphabet) => {
                let mov = match mov {
                    "R" => Direction::R,
                    "L" => Direction::L,
                    "-" => Direction::None,
                    _ => unreachable!(),
                };
                let write = match write {
                    "a" => Symbol::A,
                    "b" => Symbol::B,
                    _ => unreachable!(),
                };
                TransitionInfo { next: next.to_string(), mov, write }
            },
        }

        _state(&self) -> StateInfo {
            (_: start, state: _state()) => {
                StateInfo { start: true, ..state }
            },
            (&name: state_name, on_a: _transition(), on_b: _transition()) => {
                StateInfo {
                    start: false,
                    name: name.to_string(),
                    on_a,
                    on_b,
                }
            },
        }

        _header(&self) -> usize {
            (&number: number) => {
                number.parse().unwrap()
            },
        }
    }
}

fn make_tm(buf: &str) -> (Tm, HashMap<StateNumber, String>) {
    let mut parser = Rdp::new(StringInput::new(buf));
    assert!(parser.header());
    let states = parser._header();

    let mut tm: Tm = Default::default();
    let mut infos = Vec::new();
    let mut name_map = HashMap::new();
    let mut state_map = HashMap::new();

    name_map.insert("HALT".to_string(), 0);
    name_map.insert("ERROR".to_string(), 1);
    name_map.insert("REJECT".to_string(), 2);
    name_map.insert("OUT".to_string(), 3);
    name_map.insert("ACCEPT".to_string(), 4);

    for i in 0..name_map.len() {
        tm.states.push(State {
            on_a: Transition {
                mov: Direction::None,
                write: Symbol::A,
                next: i,
            },
            on_b: Transition {
                mov: Direction::None,
                write: Symbol::B,
                next: i,
            }
        })
    }

    for _ in 0..states {
        assert!(parser.state());
        let info = parser._state();
        let state_num = name_map.len();
        name_map.insert(info.name.clone(), state_num);
        if info.start {
            assert!(tm.start_state.is_none());
            tm.start_state = Some(state_num);
        }
        infos.push(info);
    }

    for info in infos {
        tm.states.push(State {
            on_a: Transition {
                mov: info.on_a.mov,
                write: info.on_a.write,
                next: name_map[&info.on_a.next],
            },
            on_b: Transition {
                mov: info.on_b.mov,
                write: info.on_b.write,
                next: name_map[&info.on_b.next],
            },
        })
    }

    for (name, num) in name_map {
        state_map.insert(num, name);
    }
    (tm, state_map)
}

fn main() {
    let args = Docopt::new(USAGE)
        .and_then(|d| d.parse())
        .unwrap_or_else(|e| e.exit());

    let mut buf = String::new();
    let mut fd = File::open(args.get_str("<tm2>")).expect("open file");
    fd.read_to_string(&mut buf).expect("readable");
    let (tm, state_map) = make_tm(&buf);

    let verbose = args.get_bool("-v");
    let mut tape = Vec::new();
    let mut tape_ix = 2;
    let mut current_state = tm.start_state.expect("starting state");

    let mut overall_step_count = BigUint::zero();
    let mut step_count = 0usize;
    println!("{:>24} {}", 0, state_map[&current_state]);
    loop {
        step_count += 1;
        if verbose || step_count % 1_000_000_000 == 0 {
            overall_step_count = overall_step_count + BigUint::from(step_count);
            step_count = 0;
            println!("{:>24} {}", overall_step_count, state_map[&current_state]);
        }

        if tape_ix >= tape.len() {
            tape.resize(tape_ix * 2, Symbol::A);
        }

        let state = &tm.states[current_state];
        let transition = if tape[tape_ix] == Symbol::A {
            &state.on_a
        } else {
            &state.on_b
        };

        tape[tape_ix] = transition.write;
        current_state = transition.next;

        if current_state < 5 {
            break;
        }

        match transition.mov {
            Direction::L => {
                debug_assert!(tape_ix > 0);
                tape_ix -= 1
            },
            Direction::R => tape_ix += 1,
            Direction::None => {},
        };
    }
    println!("{:>24} {}", overall_step_count + BigUint::from(step_count), state_map[&current_state]);
}
