use super::Mode;
use super::NotFinishError;

pub trait Operator<T> {
    fn tick(&self);
    fn run(&self) -> Result<Mode, NotFinishError>;
}

mod search_operator;

pub use search_operator::SearchOperator;