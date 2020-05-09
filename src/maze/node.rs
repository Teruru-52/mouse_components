use core::marker::PhantomData;

use typenum::{PowerOfTwo, Unsigned};

use super::direction::{AbsoluteDirection, RelativeDirection};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Location {
    Cell,
    VerticalBound,
    HorizontalBound,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position<N> {
    x: i16,
    y: i16,
    _size: PhantomData<fn() -> N>,
}

impl<N> Position<N>
where
    N: Unsigned,
{
    pub fn new(x: i16, y: i16) -> Self {
        Self {
            x,
            y,
            _size: PhantomData,
        }
    }

    pub fn relative_node(&self, dx: i16, dy: i16, direction: AbsoluteDirection) -> Node<N> {
        Node::new(self.x + dx, self.y + dy, direction)
    }

    pub fn relative_position(&self, dx: i16, dy: i16) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    pub fn difference(&self, to: &Self) -> (i16, i16) {
        (to.x - self.x, to.y - self.y)
    }

    #[inline]
    pub fn x(&self) -> i16 {
        self.x
    }

    #[inline]
    pub fn y(&self) -> i16 {
        self.y
    }

    #[inline]
    fn x_is_even(&self) -> bool {
        self.x() & 1 == 0
    }

    #[inline]
    fn y_is_even(&self) -> bool {
        self.y() & 1 == 0
    }

    pub fn location(&self) -> Location {
        use Location::*;
        if self.x_is_even() {
            if self.y_is_even() {
                Cell
            } else {
                HorizontalBound
            }
        } else {
            if self.y_is_even() {
                VerticalBound
            } else {
                unreachable!()
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Node<N> {
    position: Position<N>,
    direction: AbsoluteDirection,
}

impl<N> Node<N>
where
    N: Unsigned,
{
    pub fn new(x: i16, y: i16, direction: AbsoluteDirection) -> Self {
        Self {
            position: Position::new(x, y),
            direction,
        }
    }

    #[inline]
    pub fn position(&self) -> Position<N> {
        self.position.clone()
    }

    #[inline]
    pub fn x(&self) -> i16 {
        self.position().x()
    }

    #[inline]
    pub fn y(&self) -> i16 {
        self.position().y()
    }

    #[inline]
    pub fn direction(&self) -> AbsoluteDirection {
        self.direction
    }

    pub fn relative_node(
        &self,
        x_diff: i16,
        y_diff: i16,
        dir_diff: RelativeDirection,
        base_dir: AbsoluteDirection,
    ) -> Option<Self> {
        let position = self.relative_position(x_diff, y_diff, base_dir)?;
        let direction = self.direction.rotate(dir_diff);
        Some(Node::<N>::new(position.x(), position.y(), direction))
    }

    pub fn relative_position(
        &self,
        x_diff: i16,
        y_diff: i16,
        base_dir: AbsoluteDirection,
    ) -> Option<Position<N>> {
        use RelativeDirection::*;
        let relative_direction = base_dir.relative(self.direction);
        match relative_direction {
            Front => Some(Position::new(self.x() + x_diff, self.y() + y_diff)),
            Right => Some(Position::new(self.x() + y_diff, self.y() - x_diff)),
            Back => Some(Position::new(self.x() - x_diff, self.y() - y_diff)),
            Left => Some(Position::new(self.x() - y_diff, self.y() + x_diff)),
            _ => None,
        }
    }

    pub fn difference(
        &self,
        to: &Self,
        base_dir: AbsoluteDirection,
    ) -> (i16, i16, RelativeDirection) {
        use RelativeDirection::*;

        let (dx, dy) = self.position.difference(&to.position);
        let (dx, dy) = match base_dir.relative(self.direction) {
            Front => (dx, dy),
            Right => (-dy, dx),
            Back => (-dx, -dy),
            Left => (dy, -dx),
            _ => unreachable!(),
        };
        let relative_direction = self.direction.relative(to.direction);
        (dx, dy, relative_direction)
    }

    pub fn location(&self) -> Location {
        self.position.location()
    }

    pub fn in_maze(&self) -> bool {
        self.x() >= NodeId::<N>::x_min() as i16
            && self.x() <= NodeId::<N>::x_max() as i16
            && self.y() >= NodeId::<N>::y_min() as i16
            && self.y() <= NodeId::<N>::y_max() as i16
    }
}

impl<N> Node<N>
where
    N: Unsigned + PowerOfTwo,
{
    pub fn to_node_id(&self) -> Option<NodeId<N>> {
        if !self.in_maze() {
            None
        } else {
            Some(NodeId::new(
                self.x() as u16,
                self.y() as u16,
                self.direction(),
            ))
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId<N> {
    raw: u16,
    _size: PhantomData<fn() -> N>,
}

impl<N> NodeId<N>
where
    N: Unsigned,
{
    #[inline]
    fn x_min() -> u16 {
        0
    }

    #[inline]
    fn y_min() -> u16 {
        0
    }

    #[inline]
    fn x_max() -> u16 {
        N::U16 * 2 - 1
    }

    #[inline]
    fn y_max() -> u16 {
        N::U16 * 2 - 1
    }
}

impl<N> NodeId<N>
where
    N: Unsigned + PowerOfTwo,
{
    pub fn new(x: u16, y: u16, direction: AbsoluteDirection) -> Self {
        use AbsoluteDirection::*;
        debug_assert!(x <= Self::x_max());
        debug_assert!(y <= Self::y_max());
        let direction = if x & 1 == 0 {
            if y & 1 == 0 {
                match direction {
                    North => 0,
                    East => 1,
                    South => 2,
                    West => 3,
                    _ => unreachable!("x:{}, y:{}, direction:{:?}", x, y, direction),
                }
            } else {
                match direction {
                    North => 0,
                    NorthEast => 1,
                    SouthEast => 2,
                    South => 3,
                    SouthWest => 4,
                    NorthWest => 5,
                    _ => unreachable!("x:{}, y:{}, direction:{:?}", x, y, direction),
                }
            }
        } else {
            if y & 1 == 0 {
                match direction {
                    NorthEast => 0,
                    East => 1,
                    SouthEast => 2,
                    SouthWest => 3,
                    West => 4,
                    NorthWest => 5,
                    _ => unreachable!("x:{}, y:{}, direction:{:?}", x, y, direction),
                }
            } else {
                unreachable!("x:{}, y:{}, direction:{:?}", x, y, direction)
            }
        };
        Self {
            raw: x | (y << Self::y_offset()) | (direction << Self::direction_offset()),
            _size: PhantomData,
        }
    }

    pub fn as_node(&self) -> Node<N> {
        use AbsoluteDirection::*;
        let x = self.x_raw();
        let y = self.y_raw();
        let direction = if x & 1 == 0 {
            if y & 1 == 0 {
                match self.raw >> Self::direction_offset() {
                    0 => North,
                    1 => East,
                    2 => South,
                    3 => West,
                    _ => unreachable!(),
                }
            } else {
                match self.raw >> Self::direction_offset() {
                    0 => North,
                    1 => NorthEast,
                    2 => SouthEast,
                    3 => South,
                    4 => SouthWest,
                    5 => NorthWest,
                    _ => unreachable!(),
                }
            }
        } else {
            if y & 1 == 0 {
                match self.raw >> Self::direction_offset() {
                    0 => NorthEast,
                    1 => East,
                    2 => SouthEast,
                    3 => SouthWest,
                    4 => West,
                    5 => NorthWest,
                    _ => unreachable!(),
                }
            } else {
                unreachable!()
            }
        };
        Node::<N>::new(x as i16, y as i16, direction)
    }

    #[inline]
    fn y_offset() -> u32 {
        (N::USIZE * 2).trailing_zeros()
    }

    #[inline]
    fn direction_offset() -> u32 {
        2 * (N::USIZE * 2).trailing_zeros()
    }

    #[inline]
    fn x_raw(&self) -> u16 {
        self.raw & Self::x_max()
    }

    #[inline]
    fn y_raw(&self) -> u16 {
        (self.raw >> Self::y_offset()) & Self::y_max()
    }
}

impl<N> Into<Node<N>> for NodeId<N>
where
    N: Unsigned + PowerOfTwo,
{
    fn into(self) -> Node<N> {
        self.as_node()
    }
}

impl<N> Into<usize> for NodeId<N> {
    fn into(self) -> usize {
        self.raw.into()
    }
}

impl<N> core::fmt::Debug for NodeId<N>
where
    N: Unsigned + PowerOfTwo,
    Node<N>: core::fmt::Debug,
{
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        let node: Node<N> = <NodeId<N> as Into<Node<N>>>::into(*self);
        writeln!(fmt, "{:?}", node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typenum::consts::*;
    use AbsoluteDirection::*;

    #[test]
    fn test_base_methods() {
        let test_data = vec![
            (0u16, 0u16, North),
            (0, 0, East),
            (0, 0, South),
            (0, 0, West),
            (0, 1, North),
            (0, 1, NorthEast),
            (0, 1, SouthEast),
            (0, 1, South),
            (0, 1, SouthWest),
            (0, 1, NorthWest),
            (1, 0, NorthEast),
            (1, 0, East),
            (1, 0, SouthEast),
            (1, 0, SouthWest),
            (1, 0, West),
            (1, 0, NorthWest),
            (4, 4, South),
            (5, 4, West),
            (4, 5, North),
            (14, 14, North),
            (15, 14, East),
            (14, 15, South),
            (30, 30, North),
            (31, 30, East),
            (30, 31, South),
        ];

        for (x, y, direction) in test_data {
            let node = NodeId::<U16>::new(x, y, direction);
            let node = node.as_node();
            assert_eq!(node.x() as u16, x);
            assert_eq!(node.y() as u16, y);
            assert_eq!(node.direction(), direction);
        }
    }

    #[test]
    #[should_panic]
    fn test_x_unreachable() {
        NodeId::<U16>::new(32, 31, NorthEast);
    }

    #[test]
    #[should_panic]
    fn test_y_unreachable() {
        NodeId::<U16>::new(31, 32, NorthEast);
    }

    #[test]
    #[should_panic]
    fn test_direction_unreachable_in_cell() {
        NodeId::<U16>::new(0, 0, NorthEast);
    }

    #[test]
    #[should_panic]
    fn test_direction_unreachable_on_horizontal_bound() {
        NodeId::<U16>::new(0, 1, East);
    }

    #[test]
    #[should_panic]
    fn test_direction_unreachable_on_vertical_bound() {
        NodeId::<U16>::new(1, 0, North);
    }

    #[test]
    fn test_to_node_id() {
        let test_data = vec![
            (-1, 0, West, false),
            (0, 100, North, false),
            (0, 0, North, true),
            (31, 30, East, true),
        ];

        for (x, y, direction, is_some) in test_data {
            let expected = if is_some {
                Some(NodeId::<U16>::new(x as u16, y as u16, direction))
            } else {
                None
            };
            let node = Node::<U16>::new(x, y, direction);
            assert_eq!(node.to_node_id(), expected);
        }
    }
}