use self::MergeResult::*;
use compute_changes::TaskDelta::*;
use compute_changes::*;
use itertools::Itertools;
use todo_txt::task::Extended as Task;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MergeResult<T> {
    Merged(T),
    Conflict(T, Vec<T>, Vec<T>),
}

impl<T> MergeResult<T> {
    pub fn map<U, F>(self, mut f: F) -> MergeResult<U>
    where
        F: FnMut(T) -> U,
    {
        use self::MergeResult::*;
        match self {
            Merged(t) => Merged(f(t)),
            Conflict(t, t1, t2) => Conflict(
                f(t),
                t1.into_iter().map(|x| f(x)).collect(),
                t2.into_iter().map(|x| f(x)).collect(),
            ),
        }
    }
}

pub fn merge_3way(
    from: Vec<Task>,
    left: Vec<Task>,
    right: Vec<Task>,
    allowed_divergence: usize,
) -> Vec<MergeResult<Task>> {
    let (mut new_left, changes_left) = match_tasks(from.clone(), left, allowed_divergence);
    let (mut new_right, changes_right) = match_tasks(from, right, allowed_divergence);

    let mut merged_new = remove_common(&mut new_left, &mut new_right);
    merged_new.extend(new_left);
    merged_new.extend(new_right);

    changes_left
        .into_iter()
        .zip(changes_right.into_iter())
        .flat_map(
            |(left_chgt, right_chgt)| match (left_chgt.delta, right_chgt.delta) {
                (Identical, Identical) => vec![Merged(left_chgt.orig)],
                (Identical, right_delta) => right_delta.into_iter().map(Merged).collect_vec(),
                (left_delta, Identical) => left_delta.into_iter().map(Merged).collect_vec(),
                (left_delta, right_delta) => vec![Conflict(
                    left_chgt.orig,
                    left_delta.into_iter().collect_vec(),
                    right_delta.into_iter().collect_vec(),
                )],
            },
        )
        .chain(merged_new.into_iter().map(Merged))
        .collect::<Vec<MergeResult<Task>>>()
}

pub fn merge_to_string(merge: Vec<MergeResult<Task>>) -> String {
    merge
        .into_iter()
        .flat_map(|m| match m.map(|t| Task::to_string(&t)) {
            Merged(t) => vec![t],
            Conflict(t, left, right) => Some("<<<<<".to_owned())
                .into_iter()
                .chain(left)
                .chain(Some("|||||".to_owned()))
                .chain(Some(t))
                .chain(Some("=====".to_owned()))
                .chain(right)
                .chain(Some(">>>>>".to_owned()))
                .collect::<Vec<_>>(),
        })
        .join("\n")
}

pub fn merge_successful(merge: &Vec<MergeResult<Task>>) -> bool {
    merge.iter().all(|x| match x {
        Merged(_) => true,
        Conflict(_, _, _) => false,
    })
}

pub fn extract_merge_result(merge: Vec<MergeResult<Task>>) -> Option<Vec<Task>> {
    merge
        .into_iter()
        .map(|x| match x {
            Merged(t) => Some(t),
            Conflict(_, _, _) => None,
        })
        .collect()
}
