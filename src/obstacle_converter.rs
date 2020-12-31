use core::marker::PhantomData;

use generic_array::GenericArray;
use typenum::{consts::*, PowerOfTwo, Unsigned};
use uom::si::f32::Length;
use uom::si::{angle::revolution, length::meter};

use crate::obstacle_detector::Obstacle;
use crate::simple_maze::{ObstacleConverter as IObstacleConverter, WallInfo};
use crate::utils::{math::Math, total::Total};
use crate::wall_manager::Wall;

pub struct ObstacleConverter<N, M> {
    i_square_width: i32, //[mm]
    square_width_half: Length,
    square_width: Length,
    ignore_radius_from_pillar: Length,
    p1: Length,
    p2: Length,
    n1: Length,
    n2: Length,
    _maze_width: PhantomData<fn() -> N>,
    _math: PhantomData<fn() -> M>,
}

impl<N, M> ObstacleConverter<N, M> {
    pub fn new(
        square_width: Length,
        wall_width: Length,
        ignore_radius_from_pillar: Length,
    ) -> Self {
        let p1 = square_width - wall_width / 2.0;
        let p2 = p1 + square_width;
        let n1 = wall_width / 2.0;
        let n2 = n1 - square_width;
        Self {
            i_square_width: (square_width.get::<meter>() * 1000.0) as i32,
            square_width_half: square_width / 2.0,
            square_width,
            ignore_radius_from_pillar,
            p1,
            p2,
            n1,
            n2,
            _maze_width: PhantomData,
            _math: PhantomData,
        }
    }

    //(remainder, quotient)
    fn remquof_with_width(&self, val: Length) -> (Length, i32) {
        let quo = (val.get::<meter>() * 1000.0) as i32 / self.i_square_width;
        let rem = val - Length::new::<meter>((quo * self.i_square_width) as f32 * 0.001);
        (rem, quo)
    }
}

impl<N, M> ObstacleConverter<N, M>
where
    M: Math,
{
    fn is_near_pillar(&self, obstacle: &Obstacle) -> bool {
        let pose = obstacle.source;
        let distance = obstacle.distance.mean;

        let x = pose.x + distance * M::cos(pose.theta);
        let y = pose.y + distance * M::sin(pose.theta);

        let (x, _) = self.remquof_with_width(x);
        let (y, _) = self.remquof_with_width(y);

        let diff_from_pillar = |x: Length| {
            let pillar = if x < self.square_width_half {
                Length::default()
            } else {
                self.square_width
            };
            x - pillar
        };

        let xd = diff_from_pillar(x);
        let yd = diff_from_pillar(y);

        let distance = M::sqrt(xd * xd + yd * yd);

        distance < self.ignore_radius_from_pillar
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct OutOfBoundError {
    obstacle: Obstacle,
}

#[derive(Clone, PartialEq, Debug)]
pub enum ConversionError {
    WallCreation(crate::wall_manager::OutOfBoundError),
    OutOfBound(OutOfBoundError),
}

impl From<crate::wall_manager::OutOfBoundError> for ConversionError {
    fn from(value: crate::wall_manager::OutOfBoundError) -> Self {
        ConversionError::WallCreation(value)
    }
}

impl From<OutOfBoundError> for ConversionError {
    fn from(value: OutOfBoundError) -> Self {
        ConversionError::OutOfBound(value)
    }
}

impl<N, M> IObstacleConverter for ObstacleConverter<N, M>
where
    N: Unsigned + PowerOfTwo,
    M: Math,
{
    type Error = ConversionError;
    type Wall = Wall<N>;

    fn convert(&self, obstacle: &Obstacle) -> Result<WallInfo<Self::Wall>, Self::Error> {
        #[derive(Clone, Copy, Debug)]
        enum Axis {
            X(Length),
            Y(Length),
        }

        #[derive(Clone, Copy, Debug)]
        enum Direction {
            Right,
            Left,
            Top,
            Bottom,
        }

        use Direction::*;

        let create_error = || {
            Err(ConversionError::OutOfBound(OutOfBoundError {
                obstacle: obstacle.clone(),
            }))
        };

        if self.is_near_pillar(obstacle) {
            return create_error();
        }

        let pose = obstacle.source;

        let (x_rem, x_quo) = self.remquof_with_width(pose.x);
        let (y_rem, y_quo) = self.remquof_with_width(pose.y);

        if x_rem <= self.n1 || x_rem >= self.p1 || y_rem <= self.n1 || y_rem >= self.p1 {
            return create_error();
        }

        let rot = M::rem_euclidf(pose.theta.get::<revolution>(), 1.0);

        let axes = if rot < 0.25 {
            [
                (Axis::X(self.p1), Right),
                (Axis::X(self.p2), Right),
                (Axis::Y(self.p1), Top),
                (Axis::Y(self.p2), Top),
            ]
        } else if rot < 0.5 {
            [
                (Axis::X(self.n1), Left),
                (Axis::X(self.n2), Left),
                (Axis::Y(self.p1), Top),
                (Axis::Y(self.p2), Top),
            ]
        } else if rot < 0.75 {
            [
                (Axis::X(self.n1), Left),
                (Axis::X(self.n2), Left),
                (Axis::Y(self.n1), Bottom),
                (Axis::Y(self.n2), Bottom),
            ]
        } else {
            [
                (Axis::X(self.p1), Right),
                (Axis::X(self.p2), Right),
                (Axis::Y(self.n1), Bottom),
                (Axis::Y(self.n2), Bottom),
            ]
        };

        let (sin_th, cos_th) = M::sincos(pose.theta);

        let mut axes_distance = axes
            .iter()
            .map(|&(axis, direction)| {
                let dist = match axis {
                    Axis::X(x) => (x - x_rem) / cos_th,
                    Axis::Y(y) => (y - y_rem) / sin_th,
                };
                //assign infinity to invalid values
                let dist = if dist.get::<meter>() < 0.0 || dist.get::<meter>().is_nan() {
                    Length::new::<meter>(core::f32::INFINITY)
                } else {
                    dist
                };
                (direction, Total(dist))
            })
            .collect::<GenericArray<_, U4>>();

        axes_distance.sort_unstable_by_key(|e| e.1);

        let wall = match axes_distance[0].0 {
            Right => {
                if x_quo < 0 || y_quo < 0 {
                    return create_error();
                }
                Wall::new(x_quo as u16, y_quo as u16, false)
            }
            Left => {
                if x_quo < 1 || y_quo < 0 {
                    return create_error();
                }
                Wall::new((x_quo - 1) as u16, y_quo as u16, false)
            }
            Top => {
                if x_quo < 0 || y_quo < 0 {
                    return create_error();
                }
                Wall::new(x_quo as u16, y_quo as u16, true)
            }
            Bottom => {
                if x_quo < 0 || y_quo < 1 {
                    return create_error();
                }
                Wall::new(x_quo as u16, (y_quo - 1) as u16, true)
            }
        }?;

        Ok(WallInfo {
            wall,
            existing_distance: (axes_distance[0].1).0,
            not_existing_distance: (axes_distance[1].1).0,
        })
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use uom::si::angle::degree;
    use uom::si::f32::Angle;

    use super::*;
    use crate::data_types::Pose;
    use crate::utils::{math::MathFake, sample::Sample};

    macro_rules! convert_ok_tests {
        ($($name:ident: ($size:ty, $value: expr),)*) => {
            $(
                #[test]
                fn $name() {
                    let (input, expected) = $value;
                    let input = Obstacle {
                        source: Pose{
                            x: Length::new::<meter>(input.0),
                            y: Length::new::<meter>(input.1),
                            theta: Angle::new::<degree>(input.2),
                        },
                        distance: Sample{
                            mean: Length::new::<meter>(input.3),
                            standard_deviation: Length::new::<meter>(0.001),
                        },
                    };
                    let expected = WallInfo{
                        wall: Wall::<$size>::new(expected.0, expected.1, expected.2).unwrap(),
                        existing_distance: Length::new::<meter>(expected.3),
                        not_existing_distance: Length::new::<meter>(expected.4),
                    };

                    let converter = ObstacleConverter::<$size, MathFake>::new(
                        Length::new::<meter>(0.09),
                        Length::new::<meter>(0.006),
                        Length::new::<meter>(0.01),
                    );
                    let info = converter.convert(&input).unwrap();
                    assert_eq!(info.wall, expected.wall);
                    assert_relative_eq!(
                        info.existing_distance.get::<meter>(),
                        expected.existing_distance.get::<meter>(),
                    );
                    assert_relative_eq!(
                        info.not_existing_distance.get::<meter>(),
                        expected.not_existing_distance.get::<meter>(),
                    );
                }
            )*
        }
    }

    macro_rules! convert_err_tests {
        ($($name:ident: ($size:ty, $value: expr),)*) => {
            $(
                #[test]
                fn $name() {
                    let input = $value;
                    let input = Obstacle {
                        source: Pose{
                            x: Length::new::<meter>(input.0),
                            y: Length::new::<meter>(input.1),
                            theta: Angle::new::<degree>(input.2),
                        },
                        distance: Sample{
                            mean: Length::new::<meter>(input.3),
                            standard_deviation: Length::new::<meter>(0.001),
                        },
                    };

                    let converter = ObstacleConverter::<$size, MathFake>::new(
                        Length::new::<meter>(0.09),
                        Length::new::<meter>(0.006),
                        Length::new::<meter>(0.01),
                    );
                    assert!(converter.convert(&input).is_err());
                }
            )*
        }
    }

    convert_ok_tests! {
        convert_ok_test1: (U4, (
            (0.045, 0.045, 0.0, 0.045),
            (0, 0, false, 0.042, 0.132),
        )),
        convert_ok_test2: (U4, (
            (0.077, 0.045, 45.0, 0.045),
            (0, 0, false, 2.0f32.sqrt() * 0.01, 2.0f32.sqrt() * 0.042),
        )),
        convert_ok_test3: (U4, (
            (0.135, 0.045, 180.0, 0.045),
            (0, 0, false, 0.042, 0.132),
        )),
        convert_ok_test4: (U4, (
            (0.045, 0.135, 270.0, 0.045),
            (0, 0, true, 0.042, 0.132),
        )),
        convert_ok_test5: (U4, (
            (0.135, 0.135, 90.0, 0.045),
            (1, 1, true, 0.042, 0.132),
        )),
        convert_ok_test6: (U4, (
            (0.135, 0.135, 180.0, 0.045),
            (0, 1, false, 0.042, 0.132),
        )),
        convert_ok_test7: (U4, (
            (0.045, 0.135, 0.0, 0.045),
            (0, 1, false, 0.042, 0.132),
        )),
    }

    convert_err_tests! {
        convert_err_test1: (U4, (0.0, 0.0, 0.0, 0.045)),
        convert_err_test2: (U4, (0.405, 0.045, 0.0, 0.045)),
        convert_err_test3: (U4, (-0.045, 0.045, 0.0, 0.045)),
        convert_err_test4: (U4, (0.045, 0.045, 45.0, 0.0636)),
    }
}
