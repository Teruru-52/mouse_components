mod agent;
mod counter;
mod maze;
mod mode;
mod searcher;
mod solver;
mod switch;

use core::marker::PhantomData;
use core::sync::atomic::Ordering;

pub use agent::Agent;
pub use counter::Counter;
pub use maze::{
    CheckableGraph, DirectionInstructor, DirectionalGraph, Graph, GraphTranslator, Storable,
};
use mode::{AtomicMode, Mode};
use searcher::Searcher;
pub use solver::Solver;
pub use switch::Switch;

pub struct Operator<Node, Cost, AgentState, Direction, M, A, S, SW, C>
where
    M: Storable
        + DirectionalGraph<Node, Cost, Direction>
        + GraphTranslator<Node, AgentState>
        + DirectionInstructor<Node, Direction>,
    A: Agent<AgentState, Direction>,
    S: Solver<Node, Cost, Direction, M>,
    SW: Switch,
    C: Counter,
{
    maze: M,
    agent: A,
    solver: S,
    mode: AtomicMode,
    switch: SW,
    counter: C,
    searcher: Searcher<Node>,
    _node: PhantomData<fn() -> Node>,
    _cost: PhantomData<fn() -> Cost>,
    _agent_state: PhantomData<fn() -> AgentState>,
    _direction: PhantomData<fn() -> Direction>,
}

impl<Node, Cost, AgentState, Direction, M, A, S, SW, C>
    Operator<Node, Cost, AgentState, Direction, M, A, S, SW, C>
where
    Node: Copy + Clone,
    M: Storable
        + DirectionalGraph<Node, Cost, Direction>
        + GraphTranslator<Node, AgentState>
        + DirectionInstructor<Node, Direction>,
    A: Agent<AgentState, Direction>,
    S: Solver<Node, Cost, Direction, M>,
    SW: Switch,
    C: Counter,
{
    pub fn new(maze: M, agent: A, solver: S, switch: SW, counter: C) -> Self {
        let start = solver.start_node();
        Self {
            maze: maze,
            agent: agent,
            solver: solver,
            mode: AtomicMode::new(Mode::Idle),
            switch: switch,
            counter: counter,
            searcher: Searcher::new(start),
            _node: PhantomData,
            _cost: PhantomData,
            _agent_state: PhantomData,
            _direction: PhantomData,
        }
    }

    //called by periodic interrupt
    pub fn tick(&self) {
        use Mode::*;
        use Ordering::Relaxed;
        match self.mode.load(Relaxed) {
            Idle => (),
            Search => self.searcher.tick(&self.maze, &self.agent),
            Select => return,
            _ => (),
        }
        self.store_if_switch_enabled(Select);
    }

    fn store_if_switch_enabled(&self, mode: Mode) {
        if self.switch.is_enabled() {
            self.mode.store(mode, Ordering::Relaxed);
        }
    }

    pub fn run(&self) {
        use Mode::*;
        loop {
            match self.mode.load(Ordering::Relaxed) {
                Idle => (),
                Search => {
                    if !self.searcher.search(&self.maze, &self.agent, &self.solver) {
                        self.mode.store(FastRun, Ordering::Relaxed);
                    }
                }
                FastRun => self.fast_run(),
                Select => self.mode_select(),
            }
        }
    }

    fn fast_run(&self) {}

    fn mode_select(&self) {
        self.counter.reset();
        let mut mode = Mode::Idle;
        //waiting for switch off
        while self.switch.is_enabled() {}

        while !self.switch.is_enabled() {
            let count = self.counter.count();
            mode = Mode::from(count % Mode::size());
        }
        self.mode.store(mode, Ordering::Relaxed);

        while self.switch.is_enabled() {}
    }
}
