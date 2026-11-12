use super::types::SplitMix64;

impl SplitMix64 {
    pub(crate) fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub(crate) fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).unwrap_or(u64::MAX).max(1);
        usize::try_from(self.next() % bound).unwrap_or(0)
    }

    pub(crate) fn pick<T: Copy + Default>(&mut self, items: &[T]) -> T {
        items
            .get(self.below(items.len()))
            .copied()
            .unwrap_or_default()
    }
}
