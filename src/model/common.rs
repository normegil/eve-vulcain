use std::{fmt::Debug, ops::Deref};

use crate::model::locations::SolarSystem;

pub trait Identified<T> {
    fn id(&self) -> T;
}

pub trait TryIdentified<T, Err> {
    fn try_id(&self) -> Result<T, Err>;
}

pub trait Named {
    fn name(&self) -> String;
}

pub trait Located {
    fn locate(&self) -> SolarSystem;
}

#[derive(Debug)]
pub struct DetailedCalculation<V: Debug, D: Clone + Debug> {
    pub value: V,
    pub details: D,
}

impl<V: Clone + Debug, D: Clone + Debug> Deref for DetailedCalculation<V, D> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
