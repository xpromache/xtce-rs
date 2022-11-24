use std::{
    collections::HashMap,
};

use crate::{mdb::ParameterIdx, value::ParameterValue};

struct Entry {
    //the index of the previous entry for the same parameter
    prev: u32,
    pv: ParameterValue,
}

/// Parameter Value list indexed by parameter
pub struct ParameterValueList {
    // list of parameter values
    entries: Vec<Entry>,
    // index into entries of the last pv for that parameter
    last_idx: HashMap<ParameterIdx, u32>,
}

impl ParameterValueList {
    pub fn new() -> Self {
        Self { entries: Vec::with_capacity(16), last_idx: HashMap::with_capacity(16) }
    }

    pub fn push(&mut self, pv: ParameterValue) {
        let idx: u32 = self.entries.len().try_into().expect("Parameter list too long");

        let prev = self.last_idx.insert(pv.pidx, idx).unwrap_or(u32::MAX);
        self.entries.push(Entry { prev, pv });
    }

    pub fn last_inserted<'a>(&'a self, pidx: ParameterIdx) -> Option<&'a ParameterValue> {
        self.last_idx.get(&pidx).and_then(|&lidx| {
            if lidx < u32::MAX {
                Some(&self.entries[lidx as usize].pv)
            } else {
                None
            }
        })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// this is to allow to do "for pv in pvlist"

impl IntoIterator for ParameterValueList {
    type Item = ParameterValue;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.entries.into_iter())
    }
}

/// can it be done simpler??
pub struct IntoIter(std::vec::IntoIter<Entry>);
impl Iterator for IntoIter {
    type Item = ParameterValue;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|e| e.pv)
    }
}

/// this is to allow to do "for pv in &pvlist"
impl<'a> IntoIterator for &'a ParameterValueList {
    type Item = &'a ParameterValue;
    type IntoIter = IntoIterRef<'a>;

    fn into_iter(self) -> IntoIterRef<'a> {
        IntoIterRef((&self.entries).into_iter())
    }
}

/// can it be done simpler??
pub struct IntoIterRef<'a>(std::slice::Iter<'a, Entry>);
impl<'a> Iterator for IntoIterRef<'a> {
    type Item = &'a ParameterValue;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|e| &e.pv)
    }
}
