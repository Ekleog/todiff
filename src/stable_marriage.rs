struct Individual {
    cnt_match: Option<usize>,

    // For efficiency: most preferred last for men, most preferred first for women
    is_man: bool,
    prefs: Vec<usize>,
}

impl Individual {
    fn get_pref_idx(&self, idx: usize) -> Option<usize> {
        debug_assert!(!self.is_man);
        self.prefs.iter().position(|i| *i == idx)
    }

    fn pop_preferred_if_unmatched(&mut self) -> Option<usize> {
        debug_assert!(self.is_man);
        if self.cnt_match.is_some() {
            None
        } else {
            self.prefs.pop()
        }
    }
}

fn make_empty_matching(v: Vec<Vec<usize>>, is_man: bool) -> Vec<Individual> {
    v.into_iter()
        .map(|prefs| Individual {
            cnt_match: None,
            is_man: is_man,
            prefs: prefs,
        })
        .collect()
}

fn init_matching(
    men: Vec<Vec<usize>>,
    women: Vec<Vec<usize>>,
) -> (Vec<Individual>, Vec<Individual>) {
    let mut men = make_empty_matching(men, true);
    let women = make_empty_matching(women, false);
    for mut x in &mut men {
        x.prefs.reverse();
    }

    (men, women)
}

fn extract_matching(v: Vec<Individual>) -> Vec<Option<usize>> {
    v.into_iter().map(|x| x.cnt_match).collect()
}

fn saturate_matching(men: &mut Vec<Individual>, women: &mut Vec<Individual>) {
    while let Some(i_man) = men.iter()
        .position(|m| m.cnt_match.is_none() && !m.prefs.is_empty())
    {
        while let Some(i_woman) = men[i_man].pop_preferred_if_unmatched() {
            let woman = &mut women[i_woman];

            if let Some(i_man_in_w_prefs) = woman.get_pref_idx(i_man) {
                if let Some(i_otherman) = woman.cnt_match {
                    men[i_otherman].cnt_match = None;
                }
                men[i_man].cnt_match = Some(i_woman);
                woman.cnt_match = Some(i_man);
                woman.prefs.truncate(i_man_in_w_prefs);
            }
        }
    }
}

// Computes a stable matching between two lists of individuals.
// See https://en.wikipedia.org/wiki/Stable_marriage_problem
// This implements an extended version of the Gale-Shapley algorithm that allows for some
// individuals to not rank every individual from the other list, in which case those two
// individuals will never be matched together. In particular, the lists need not be the same size.
//
// @arg men: A list of preference rankings (most preferred first) of indices in the list `women`
// @arg women: A list of preference rankings (most preferred first) of indices in the list `men`
pub fn stable_marriage(
    men: Vec<Vec<usize>>,
    women: Vec<Vec<usize>>,
) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
    let (mut men, mut women) = init_matching(men, women);

    saturate_matching(&mut men, &mut women);

    (extract_matching(men), extract_matching(women))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn test_case(
        init_men: Vec<Vec<usize>>,
        init_women: Vec<Vec<usize>>,
        expected_men: Vec<Option<usize>>,
        expected_women: Vec<Option<usize>>,
    ) {
        assert_eq!(
            stable_marriage(init_men, init_women),
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
