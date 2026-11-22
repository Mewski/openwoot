use super::{Converter, Recheck, Reference};
use crate::analog::NoiseStatistics;

pub trait Sensing: 'static {
    type Converter: Converter + Send;
    type Noise: NoiseStatistics + Send;
    type Recheck: Recheck + Send;
    type Reference: Reference + Send;
}
