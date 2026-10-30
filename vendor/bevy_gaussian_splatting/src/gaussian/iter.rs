#[cfg(feature = "sort_rayon")]
use rayon::iter::plumbing::{Consumer, UnindexedConsumer};
#[cfg(feature = "sort_rayon")]
use rayon::prelude::*;

use crate::gaussian::f32::{Position, PositionVisibility};

pub struct PositionIter<'a> {
    slice_iter: std::slice::Iter<'a, PositionVisibility>,
}

impl<'a> PositionIter<'a> {
    pub fn new(slice: &'a [PositionVisibility]) -> Self {
        Self {
            slice_iter: slice.iter(),
        }
    }
}

impl<'a> Iterator for PositionIter<'a> {
    type Item = &'a Position;

    fn next(&mut self) -> Option<Self::Item> {
        self.slice_iter.next().map(|pv| &pv.position)
    }
}

#[cfg(feature = "sort_rayon")]
pub struct PositionParIter<'a> {
    slice_par_iter: rayon::slice::Iter<'a, PositionVisibility>,
}

#[cfg(feature = "sort_rayon")]
impl<'a> PositionParIter<'a> {
    pub fn new(slice: &'a [PositionVisibility]) -> Self {
        Self {
            slice_par_iter: slice.par_iter(),
        }
    }
}

#[cfg(feature = "sort_rayon")]
impl<'a> ParallelIterator for PositionParIter<'a> {
    type Item = &'a Position;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.slice_par_iter
            .map(|pv| &pv.position)
            .drive_unindexed(consumer)
    }
}

#[cfg(feature = "sort_rayon")]
impl IndexedParallelIterator for PositionParIter<'_> {
    fn len(&self) -> usize {
        self.slice_par_iter.len()
    }

    fn drive<C>(self, consumer: C) -> <C as Consumer<Self::Item>>::Result
    where
        C: Consumer<Self::Item>,
    {
        self.slice_par_iter.map(|pv| &pv.position).drive(consumer)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: rayon::iter::plumbing::ProducerCallback<Self::Item>,
    {
        self.slice_par_iter
            .map(|pv| &pv.position)
            .with_producer(callback)
    }
}
