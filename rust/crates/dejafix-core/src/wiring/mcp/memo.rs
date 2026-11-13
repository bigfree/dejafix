use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{MutexGuard, PoisonError};

use jiff::SignedDuration;

use super::types::{GitMemo, MemoGit, Remembered};
use crate::adapters::git::{RepoState, repository_state};
use crate::core::stale::FileHistory;
use crate::wiring::hooks::GitAnswers;

pub(super) const CHURN_LIFETIME: SignedDuration = SignedDuration::from_secs(60);
const REMEMBERED_ANSWERS: usize = 4096;

fn keep<K: Eq + Hash, T>(map: &mut HashMap<K, Remembered<T>>, key: K, value: Remembered<T>) {
    if map.len() >= REMEMBERED_ANSWERS && !map.contains_key(&key) {
        map.clear();
    }
    map.insert(key, value);
}

fn current<'r, T>(
    found: Option<&'r Remembered<T>>,
    state: &RepoState,
    fresh: impl Fn(&Remembered<T>) -> bool,
) -> Option<&'r T> {
    found
        .filter(|remembered| remembered.state == *state && fresh(remembered))
        .map(|remembered| &remembered.answer)
}

impl<G: GitAnswers> MemoGit<'_, G> {
    fn memo(&self) -> MutexGuard<'_, GitMemo> {
        self.memo.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn state(&self) -> Option<&RepoState> {
        self.state
            .get_or_init(|| repository_state(self.root))
            .as_ref()
    }

    fn remembered<T>(&self, state: &RepoState, answer: T) -> Remembered<T> {
        Remembered {
            state: state.clone(),
            at: self.now,
            answer,
        }
    }
}

impl<G: GitAnswers> FileHistory for MemoGit<'_, G> {
    fn exists(&self, relative: &str) -> bool {
        self.inner.exists(relative)
    }

    fn commit_times(&self, relative: &str, since: Option<&str>) -> Option<Vec<String>> {
        let key = (
            self.root.to_path_buf(),
            relative.to_owned(),
            since.map(str::to_owned),
        );
        let Some(state) = self.state() else {
            return self.inner.commit_times(relative, since);
        };
        if let Some(answer) = current(self.memo().times.get(&key), state, |_| true) {
            return Some(answer.clone());
        }
        let answer = self.inner.commit_times(relative, since)?;
        let value = self.remembered(state, answer.clone());
        keep(&mut self.memo().times, key, value);
        Some(answer)
    }
}

impl<G: GitAnswers> GitAnswers for MemoGit<'_, G> {
    fn churn(&self, relative: &str) -> Option<usize> {
        let key = (self.root.to_path_buf(), relative.to_owned());
        let now = self.now;
        let fresh = |remembered: &Remembered<usize>| {
            now >= remembered.at && now.duration_since(remembered.at) < CHURN_LIFETIME
        };
        let Some(state) = self.state() else {
            return self.inner.churn(relative);
        };
        if let Some(answer) = current(self.memo().churn.get(&key), state, fresh) {
            return Some(*answer);
        }
        let answer = self.inner.churn(relative)?;
        let value = self.remembered(state, answer);
        keep(&mut self.memo().churn, key, value);
        Some(answer)
    }
}
