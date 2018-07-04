use itertools::Itertools;
use std;
use std::cmp::Ordering;

pub trait Matcher {
    type Item;
    type Target;

    fn is_admissible(&self, x: &Self::Item, y: &Self::Target) -> bool;

    fn cmp_3way(&self, from: &Self::Item, left: &Self::Target, right: &Self::Target) -> Ordering;

    fn compute_preference_list<Q>(
        &self,
        item: &Self::Item,
        targets: &Vec<Woman<Self>>,
        other_matcher: &Q,
    ) -> Vec<usize>
    where
        Q: Matcher<Item = Self::Target, Target = Self::Item>,
    {
        let mut admissibles = targets
            .iter()
            .enumerate()
            .filter(|(_, x)| x.prefers_to_current(other_matcher, item))
            .map(|(i, x)| (i, &x.data))
            .filter(|(_, x)| self.is_admissible(item, x))
            .collect::<Vec<_>>();

        admissibles.sort_unstable_by(|(i, left), (j, right)| {
            self.cmp_3way(item, left, right).then(i.cmp(&j)).reverse()
        });

        admissibles.into_iter().map(|(i, _)| i).collect::<Vec<_>>()
    }

    // Captures a notion of a couple being made for each other.
    // If a good number of matchings have a perfect match, quadratic behaviour is strongly reduced.
    fn is_perfect_match(&self, _x: &Self::Item, _y: &Self::Target) -> bool {
        false
    }

    // Looks through currently unengaged women for a potential perfect match.
    fn find_perfect_match<'a>(
        &self,
        item: &Self::Item,
        targets: &'a mut Vec<Woman<Self>>,
    ) -> Option<&'a mut Woman<Self>> {
        targets
            .iter_mut()
            .filter(|x| x.current_match.is_none())
            .find(|x| self.is_perfect_match(item, &x.data))
    }
}

struct Man<P: Matcher + ?Sized> {
    data: P::Item,
    // Most preferred last
    prefs: Vec<usize>,
}

impl<P: Matcher + ?Sized> Man<P> {}

pub struct Woman<P: Matcher + ?Sized> {
    data: P::Target,
    current_match: Option<Man<P>>,
    current_is_perfect: bool,
}

impl<P: Matcher + ?Sized> Woman<P> {
    fn prefers_to_current<Q>(&self, matcher: &Q, item: &P::Item) -> bool
    where
        Q: Matcher<Item = P::Target, Target = P::Item>,
    {
        if self.current_is_perfect || !matcher.is_admissible(&self.data, item) {
            return false;
        }
        if let Some(ref cnt_man) = self.current_match {
            matcher.cmp_3way(&self.data, &cnt_man.data, item) == Ordering::Greater
        } else {
            true
        }
    }

    fn replace_match(&mut self, man: Man<P>) -> Option<Man<P>> {
        let mut old_match = Some(man);
        std::mem::swap(&mut self.current_match, &mut old_match);
        old_match
    }
}

// Computes a stable matching between two lists of individuals.
// See https://en.wikipedia.org/wiki/Stable_marriage_problem
// This implements an extended version of the Gale-Shapley algorithm that allows for some
// individuals to not rank every individual from the other list, in which case those two
// individuals will never be matched together. In particular, the lists need not be the same size.
// This algorithm favors men.
// Returns matchings from the women's perspective, and unmatched men.
// The order of women is preserved from the input list.
pub fn stable_marriage<M, W, P: Matcher<Item = M, Target = W>, Q: Matcher<Item = W, Target = M>>(
    men: Vec<M>,
    women: Vec<W>,
    men_matcher: &P,
    women_matcher: &Q,
) -> (Vec<(W, Option<M>)>, Vec<M>) {
    let mut women = women
        .into_iter()
        .map(|item| Woman {
            data: item,
            current_match: None,
            current_is_perfect: false,
        })
        .collect::<Vec<Woman<P>>>();

    let mut no_longer_engageables = Vec::new();
    'outer_loop: for item in men {
        let mut man = Man {
            data: item,
            prefs: vec![],
        };

        if let Some(woman) = men_matcher.find_perfect_match(&man.data, &mut women) {
            woman.current_is_perfect = true;
            woman.replace_match(man);
            continue;
        }
        man.prefs = men_matcher.compute_preference_list(&man.data, &women, women_matcher);

        // Loop while the man we hold is still engageable
        while let Some(i) = man.prefs.pop() {
            let woman = &mut women[i];
            if woman.prefers_to_current(women_matcher, &man.data) {
                if let Some(rejected_man) = woman.replace_match(man) {
                    man = rejected_man;
                } else {
                    // We no longer hold a man; fetch the next one
                    continue 'outer_loop;
                }
            }
        }
        // `man` has no remaining women he wants to propose to
        no_longer_engageables.push(man);
    }

    (
        women
            .into_iter()
            .map(|x| (x.data, x.current_match.map(|man| man.data)))
            .collect_vec(),
        no_longer_engageables
            .into_iter()
            .map(|man| man.data)
            .collect_vec(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;

    struct IndexMatcher(Vec<Vec<usize>>);

    impl Matcher for IndexMatcher {
        type Item = usize;
        type Target = usize;

        fn is_admissible(&self, x: &Self::Item, y: &Self::Target) -> bool {
            self.0[*x].iter().position(|j| j == y).is_some()
        }

        fn cmp_3way(
            &self,
            from: &Self::Item,
            left: &Self::Target,
            right: &Self::Target,
        ) -> Ordering {
            let pos_left = self.0[*from].iter().position(|j| j == left);
            let pos_right = self.0[*from].iter().position(|j| j == right);
            pos_left.cmp(&pos_right)
        }

        fn compute_preference_list<Q>(
            &self,
            item: &Self::Item,
            _targets: &Vec<Woman<Self>>,
            _other_matcher: &Q,
        ) -> Vec<usize>
        where
            Q: Matcher<Item = Self::Target, Target = Self::Item>,
        {
            // Cheat, because we know which targets will be used
            self.0[*item].iter().cloned().rev().collect_vec()
        }
    }

    // @arg men: A list of preference rankings (most preferred first) of indices in the list `women`
    // @arg women: A list of preference rankings (most preferred first) of indices in the list `men`
    fn stable_marriage_from_preference_lists(
        men: Vec<Vec<usize>>,
        women: Vec<Vec<usize>>,
    ) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
        let n_men = men.len();
        let n_women = women.len();
        let men_matcher = IndexMatcher(men);
        let women_matcher = IndexMatcher(women);

        let men_indices = (0..n_men).collect_vec();
        let women_indices = (0..n_women).collect_vec();
        let (matches_women, _) =
            stable_marriage(men_indices, women_indices, &men_matcher, &women_matcher);

        let mut matches_men = vec![None; n_men];
        for (i, mtch) in &matches_women {
            if let Some(j) = mtch {
                matches_men[*j] = Some(*i);
            }
        }

        (
            matches_men,
            matches_women.into_iter().map(|x| x.1).collect_vec(),
        )
    }

    pub fn test_case(
        init_men: Vec<Vec<usize>>,
        init_women: Vec<Vec<usize>>,
        expected_men: Vec<Option<usize>>,
        expected_women: Vec<Option<usize>>,
    ) {
        assert_eq!(
            stable_marriage_from_preference_lists(init_men, init_women),
            (expected_men, expected_women)
        );
    }

    #[test]
    fn test_stable_marriage() {
        let men = vec![
            vec![3, 1, 2, 0],
            vec![1, 0, 2, 3],
            vec![0, 1, 2, 3],
            vec![0, 1, 2, 3],
        ];
        let women = vec![
            vec![0, 1, 2, 3],
            vec![0, 1, 2, 3],
            vec![0, 1, 2, 3],
            vec![0, 1, 2, 3],
        ];

        let expected_men = vec![Some(3), Some(1), Some(0), Some(2)];
        let expected_women = vec![Some(2), Some(1), Some(3), Some(0)];

        test_case(men, women, expected_men, expected_women);

        let men = vec![
            vec![1, 2, 3, 0],
            vec![0, 2, 1, 3],
            vec![0, 3, 2, 1],
            vec![3, 1, 0, 2],
        ];
        let women = vec![
            vec![0, 3, 2, 1],
            vec![1, 0, 3, 2],
            vec![2, 1, 3, 0],
            vec![2, 3, 1, 0],
        ];

        let expected_men = vec![Some(1), Some(2), Some(0), Some(3)];
        let expected_women = vec![Some(2), Some(0), Some(1), Some(3)];

        test_case(men, women, expected_men, expected_women);

        let men = vec![vec![0, 1], vec![0]];
        let women = vec![vec![0, 1], vec![0]];

        let expected_men = vec![Some(0), None];
        let expected_women = vec![Some(0), None];

        test_case(men, women, expected_men, expected_women);
    }
}
