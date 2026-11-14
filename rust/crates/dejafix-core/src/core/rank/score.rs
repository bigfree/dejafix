use super::index::slot;
use super::types::{Index, QueryTerms, RankParams, Scored};

impl RankParams {
    pub const DEFAULT: Self = Self {
        k1: 1.2,
        b: 0.75,
        min_coverage: 0.5,
        min_ratio: 0.5,
        limit: usize::MAX,
    };
}

fn count(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

impl Index {
    fn idf(&self, term: u32) -> f64 {
        let documents = count(self.len());
        let containing = f64::from(
            self.document_frequency
                .get(slot(term))
                .copied()
                .unwrap_or(0),
        );
        ((documents - containing + 0.5) / (containing + 0.5)).ln_1p()
    }

    #[must_use]
    pub fn rank(&self, query: &str, params: &RankParams) -> Vec<Scored> {
        self.rank_terms(&self.query_terms(query), params)
    }

    #[must_use]
    pub fn rank_terms(&self, query: &QueryTerms, params: &RankParams) -> Vec<Scored> {
        if query.distinct == 0 || self.is_empty() || query.ids.is_empty() {
            return Vec::new();
        }
        let mut scores = vec![0.0_f64; self.len()];
        let mut covered = vec![0_u32; self.len()];
        let mut weighted = vec![0.0_f64; self.len()];
        let mut touched: Vec<u32> = Vec::new();
        for &term in &query.ids {
            for (field, spec) in self.fields.iter().enumerate() {
                let average = self.average.get(field).copied().unwrap_or(0.0);
                for &(doc, frequency) in self.postings(term, field) {
                    let Some(slot_weight) = weighted.get_mut(slot(doc)) else {
                        continue;
                    };
                    if *slot_weight <= 0.0 {
                        touched.push(doc);
                    }
                    let norm =
                        1.0 - params.b + params.b * f64::from(self.length(field, doc)) / average;
                    *slot_weight += spec.weight * f64::from(frequency) / norm;
                }
            }
            let idf = self.idf(term);
            for doc in touched.drain(..) {
                let index = slot(doc);
                let Some(tf) = weighted.get_mut(index) else {
                    continue;
                };
                let saturated = *tf * (params.k1 + 1.0) / (*tf + params.k1);
                *tf = 0.0;
                if let (Some(score), Some(hits)) = (scores.get_mut(index), covered.get_mut(index)) {
                    *score += idf * saturated;
                    *hits += 1;
                }
            }
        }
        let distinct = count(query.distinct);
        let mut found: Vec<Scored> = covered
            .iter()
            .zip(&scores)
            .enumerate()
            .filter(|(_, (hits, _))| **hits > 0)
            .map(|(doc, (hits, score))| Scored {
                doc: u32::try_from(doc).unwrap_or(u32::MAX),
                score: *score,
                coverage: f64::from(*hits) / distinct,
            })
            .filter(|scored| scored.coverage >= params.min_coverage)
            .collect();
        let best = found
            .iter()
            .map(|scored| scored.score)
            .fold(0.0_f64, f64::max);
        found.retain(|scored| scored.score >= params.min_ratio * best);
        found.sort_by(|left, right| {
            right.score.total_cmp(&left.score).then_with(|| {
                let order = |scored: &Scored| self.order.get(slot(scored.doc)).copied();
                order(left).cmp(&order(right))
            })
        });
        found.truncate(params.limit);
        found
    }
}
