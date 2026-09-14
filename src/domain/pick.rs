//! Which repository to replay.

/// Tries `repos` in a random order and returns the first one `open` opens.
///
/// # Errors
///
/// When `open` fails for every repository: why each failed, in the order
/// tried.
pub fn open_random<'r, R, T, E>(
    repos: &'r [R],
    rng: &mut fastrand::Rng,
    mut open: impl FnMut(&R) -> Result<T, E>,
) -> Result<T, Vec<(&'r R, E)>> {
    let mut order: Vec<&R> = repos.iter().collect();
    rng.shuffle(&mut order);
    let mut failures = Vec::new();
    for repo in order {
        match open(repo) {
            Ok(opened) => return Ok(opened),
            Err(error) => failures.push((repo, error)),
        }
    }
    Err(failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastrand::Rng;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use std::collections::HashSet;

    /// Repositories are indices; those in `openable` open to themselves and
    /// the rest fail with themselves. Returns the result and the order tried.
    fn open(openable: &[bool], seed: u64) -> (Result<usize, Vec<usize>>, Vec<usize>) {
        let repos: Vec<usize> = (0..openable.len()).collect();
        let mut tried = vec![];
        let result = open_random(&repos, &mut Rng::with_seed(seed), |&i| {
            tried.push(i);
            if openable[i] { Ok(i) } else { Err(i) }
        });
        let result = result.map_err(|failures| {
            failures
                .into_iter()
                .map(|(&repo, error)| {
                    assert_eq!(repo, error, "a failure names its repository");
                    repo
                })
                .collect()
        });
        (result, tried)
    }

    #[test]
    fn every_repository_gets_picked_across_seeds() {
        let picked: HashSet<usize> = (0..200)
            .map(|seed| open(&[true; 4], seed).0.unwrap())
            .collect();
        assert_eq!(picked.len(), 4);
    }

    #[test]
    fn nothing_to_open_is_an_empty_failure() {
        assert_eq!(open(&[], 1), (Err(vec![]), vec![]));
    }

    proptest! {
        #[test]
        fn opens_one_that_opens_after_only_failures(openable in vec(any::<bool>(), 0..12), seed: u64) {
            let (result, tried) = open(&openable, seed);
            let mut seen = tried.clone();
            seen.sort_unstable();
            seen.dedup();
            prop_assert_eq!(seen.len(), tried.len(), "none is tried twice");
            match result {
                Ok(opened) => {
                    let (last, before) = tried.split_last().unwrap();
                    prop_assert_eq!(*last, opened);
                    prop_assert!(openable[opened]);
                    prop_assert!(before.iter().all(|&i| !openable[i]));
                }
                Err(failures) => {
                    prop_assert!(openable.iter().all(|&o| !o));
                    prop_assert_eq!(&failures, &tried);
                    prop_assert_eq!(failures.len(), openable.len());
                }
            }
        }

        #[test]
        fn a_seed_fixes_the_choice(openable in vec(any::<bool>(), 0..12), seed: u64) {
            prop_assert_eq!(open(&openable, seed), open(&openable, seed));
        }
    }
}
