//! Homescreen pages: current index, occupancy, and compact-on-idle.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pages {
    current: u32,
    /// Occupied-slot count when a lift started.
    lift_from: Option<u32>,
}

impl Pages {
    pub fn current(&self) -> u32 {
        self.current
    }

    pub fn jump(&mut self, page: u32) {
        self.current = page;
    }

    pub fn nearby(&self, page: u32) -> bool {
        page.abs_diff(self.current) <= 1
    }

    pub fn offset(&self, index: u32, width: i32) -> i32 {
        (index as i32 - self.current as i32) * width
    }

    pub fn begin_lift(&mut self, from: u32) {
        self.lift_from = Some(from.max(1));
    }

    pub fn end_lift(&mut self) {
        self.lift_from = None;
    }

    pub fn count(&self, occupied: &[u32], carrying: Option<u32>) -> u32 {
        let packed = unique_count(occupied).max(1);
        match (self.lift_from, carrying) {
            (Some(base), Some(lift)) if lift < base => base + 1,
            (Some(_), Some(lift)) => lift + 1,
            _ => packed,
        }
    }

    /// Dense `old → new` map. Caller applies it when nothing is in the air.
    pub fn compact(occupied: impl IntoIterator<Item = u32>) -> HashMap<u32, u32> {
        let mut pages: Vec<u32> = occupied.into_iter().collect();
        pages.sort_unstable();
        pages.dedup();
        pages
            .into_iter()
            .enumerate()
            .map(|(i, old)| (old, i as u32))
            .collect()
    }
}

fn unique_count(occupied: &[u32]) -> u32 {
    let mut pages = occupied.to_vec();
    pages.sort_unstable();
    pages.dedup();
    pages.len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_removes_holes() {
        let map = Pages::compact([0, 2, 2, 5]);
        assert_eq!(map.get(&0), Some(&0));
        assert_eq!(map.get(&2), Some(&1));
        assert_eq!(map.get(&5), Some(&2));
    }

    #[test]
    fn spare_only_while_carrying() {
        let idle = Pages::default();
        assert_eq!(idle.count(&[0, 1], None), 2);
        assert_eq!(idle.count(&[], None), 1);

        let mut pages = Pages::default();
        pages.begin_lift(2);
        assert_eq!(pages.count(&[0, 1], Some(1)), 3);
        assert_eq!(pages.count(&[0, 1, 2], Some(2)), 3);
        pages.end_lift();
        assert_eq!(pages.count(&[0, 1], None), 2);
    }
}
