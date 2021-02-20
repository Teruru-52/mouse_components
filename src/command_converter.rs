use core::marker::PhantomData;

use uom::si::angle::degree;
use uom::si::f32::{Angle, Length};

use crate::nodes::Node;
use crate::trajectory_managers::CommandConverter as ICommandConverter;
use crate::types::data::{AbsoluteDirection, Pose};

//NOTE: This struct is intended to be used by SearchOperator
/// An implementation of [CommandConverter](crate::trajectory_managers::CommandConverter).
#[derive(Clone, PartialEq, Debug)]
pub struct CommandConverter {
    square_width_half: Length,
    front_offset: Length,
}

impl CommandConverter {
    pub fn new(square_width: Length, front_offset: Length) -> Self {
        Self {
            square_width_half: square_width / 2.0,
            front_offset,
        }
    }
}

/// A config for [CommandConverter](CommandConverter).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandConverterConfig {
    pub square_width: Length,
    pub front_offset: Length,
}

impl<'a, Config, State> From<(&'a Config, &'a State)> for CommandConverter
where
    &'a Config: Into<CommandConverterConfig>,
{
    fn from((config, _): (&'a Config, &'a State)) -> Self {
        let config = config.into();
        Self::new(config.square_width, config.front_offset)
    }
}

impl CommandConverter {
    const DEFAULT_SQUARE_WIDTH: Length = Length {
        dimension: PhantomData,
        units: PhantomData,
        value: 0.09,
    };
}

impl Default for CommandConverter {
    fn default() -> Self {
        Self::new(Self::DEFAULT_SQUARE_WIDTH, Default::default())
    }
}

fn _convert<N>(node: &Node<N>, square_width_half: Length, front_offset: Length) -> Pose {
    use AbsoluteDirection::*;

    let (dx, dy, theta) = if (node.x() ^ node.y()) & 1 == 1 {
        match node.direction() {
            North => (Default::default(), front_offset, 90.0),
            East => (front_offset, Default::default(), 0.0),
            South => (Default::default(), -front_offset, -90.0),
            West => (-front_offset, Default::default(), 180.0),
            NorthEast => (Default::default(), Default::default(), 45.0),
            NorthWest => (Default::default(), Default::default(), 135.0),
            SouthEast => (Default::default(), Default::default(), -45.0),
            SouthWest => (Default::default(), Default::default(), -135.0),
        }
    } else {
        (
            Default::default(),
            Default::default(),
            match node.direction() {
                East => 0.0,
                NorthEast => 45.0,
                North => 90.0,
                NorthWest => 135.0,
                West => 180.0,
                SouthEast => -45.0,
                South => -90.0,
                SouthWest => -135.0,
            },
        )
    };
    Pose {
        x: (node.x() + 1) as f32 * square_width_half + dx,
        y: (node.y() + 1) as f32 * square_width_half + dy,
        theta: Angle::new::<degree>(theta),
    }
}

//TODO: Write test.
impl<N, K: Clone> ICommandConverter<(Node<N>, K)> for CommandConverter {
    type Output = (Pose, K);

    fn convert(&self, (node, kind): &(Node<N>, K)) -> Self::Output {
        (
            _convert(node, self.square_width_half, self.front_offset),
            kind.clone(),
        )
    }
}

impl<N, INode, K: Clone> ICommandConverter<(INode, K)> for CommandConverter
where
    INode: core::ops::Deref<Target = Node<N>>,
{
    type Output = (Pose, K);

    fn convert(&self, (node, kind): &(INode, K)) -> Self::Output {
        (
            _convert(node.deref(), self.square_width_half, self.front_offset),
            kind.clone(),
        )
    }
}
