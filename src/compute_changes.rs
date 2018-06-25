use chrono::{Datelike, Duration};
use itertools::Either;
use stable_marriage;
use std;
use strsim::levenshtein;
use todo_txt::Date as TaskDate;
use todo_txt::Task;

// These structs will be used in two stages: first with T=Task when matching tasks together,
// and then with T=Vec<Changes> when computing actual deltas to be displayed
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ChangedTask<T> {
    pub orig: Task,
    pub delta: TaskDelta<T>,
}

#[cfg_attr(feature = "integration_tests", derive(Deserialize))]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TaskDelta<T> {
    Identical,
    Deleted,
    Changed(T),
    Recurred(Vec<T>),
}

impl<T> IntoIterator for TaskDelta<T> {
    type Item = T;
    type IntoIter =
        Either<<Option<T> as IntoIterator>::IntoIter, <Vec<T> as IntoIterator>::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        use self::TaskDelta::*;
        match self {
            Identical => Either::Left(None),
            Deleted => Either::Left(None),
            Changed(t) => Either::Left(Some(t)),
            Recurred(vec) => Either::Right(vec),
        }.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a TaskDelta<T> {
    type Item = &'a T;
    type IntoIter =
        Either<<Option<&'a T> as IntoIterator>::IntoIter, <&'a Vec<T> as IntoIterator>::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        use self::TaskDelta::*;
        match self {
            Identical => Either::Left(None),
            Deleted => Either::Left(None),
            Changed(t) => Either::Left(Some(t)),
            Recurred(vec) => Either::Right(vec),
        }.into_iter()
    }
}

impl<T> TaskDelta<T> {
    pub fn iter(&self) -> <&Self as IntoIterator>::IntoIter {
        self.into_iter()
    }

    pub fn map<U, F>(self, mut f: F) -> TaskDelta<U>
    where
        F: FnMut(T) -> U,
    {
        use self::TaskDelta::*;
        match self {
            Identical => Identical,
            Deleted => Deleted,
            Changed(t) => Changed(f(t)),
            Recurred(vec) => Recurred(vec.into_iter().map(f).collect::<Vec<_>>()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Changes {
    Created,
    Copied,
    RecurredStrict,
    RecurredFrom(TaskDate),

    FinishedAt(TaskDate),
    PostponedStrictBy(Duration),

    // All the variants below are of the form (before, after)
    Finished(bool), // The exception: bool has only two values, so only store after
    Priority(Option<char>, Option<char>),
    FinishDate(Option<TaskDate>, Option<TaskDate>),
    CreateDate(Option<TaskDate>, Option<TaskDate>),
    Subject(String, String),
    DueDate(Option<TaskDate>, Option<TaskDate>),
    ThresholdDate(Option<TaskDate>, Option<TaskDate>),
    Tags(Vec<(String, String)>, Vec<(String, String)>),
}

fn postpone_days(from: &Task, to: &Task) -> Option<Duration> {
    if let Some(from_due) = from.due_date {
        if let Some(to_due) = to.due_date {
            if from.threshold_date == None && to.threshold_date == None {
                return Some(to_due.signed_duration_since(from_due));
            }
            if let Some(from_thresh) = from.threshold_date {
                if let Some(to_thresh) = to.threshold_date {
                    if to_due.signed_duration_since(from_due)
                        == to_thresh.signed_duration_since(from_thresh)
                    {
                        return Some(to_due.signed_duration_since(from_due));
                    }
                }
            }
        }
    }
    None
}

fn add_recspec_to_date(date: TaskDate, recspec: &str) -> Option<TaskDate> {
    let mut n = recspec.to_owned();
    n.pop();
    if let Ok(n) = n.parse::<u16>() {
        match recspec.chars().last() {
            Some('d') => Some(date + Duration::days(n as i64)),
            Some('w') => Some(date + Duration::weeks(n as i64)),
            Some('m') => Some(
                date.with_month0((date.month0() + n as u32) % 12)
                    .expect("Internal error E006")
                    .with_year(date.year() + ((date.month0() + n as u32) / 12) as i32)
                    .expect("Internal error E007"),
            ),
            Some('y') => Some(
                date.with_year(date.year() + n as i32)
                    .expect("Internal error E008"),
            ),
            _ => None,
        }
    } else {
        None
    }
}

pub fn changes_between(from: &Task, to: &Task, is_first: bool) -> Vec<Changes> {
    use self::Changes::*;

    let mut res = Vec::new();
    let mut done_recurred = false;
    let mut done_finished_at = false;
    let mut done_postponed_strict = false;

    // First, things that may trigger a removal of the `copied` item
    if !is_first && from.tags.get("rec") == to.tags.get("rec") {
        if let (Some(r), Some(_), Some(from_due), Some(to_due)) = (
            from.tags.get("rec"),
            postpone_days(from, to),
            from.due_date,
            to.due_date,
        ) {
            if r.chars().next() == Some('+') {
                let mut c = r.chars();
                c.next();
                let r = c.collect::<String>();
                if add_recspec_to_date(from_due, &r) == Some(to_due) {
                    res.push(RecurredStrict);
                    done_recurred = true;
                }
            } else {
                if let Some(to_create) = to.create_date {
                    if add_recspec_to_date(to_create, &r) == Some(to_due) {
                        res.push(RecurredFrom(to_create));
                        done_recurred = true;
                    }
                }
            }
        }
    }

    // Then, the `copied` item
    if !done_recurred && !is_first {
        res.push(Copied);
    }

    // Then, the optimizations handling multiple changes at once
    if from.finished == false
        && to.finished == true
        && from.finish_date.is_none()
        && to.finish_date.is_some()
    {
        res.push(FinishedAt(to.finish_date.expect("Internal error E005")));
        done_finished_at = true;
    }
    if !done_recurred && from.due_date != to.due_date {
        if let Some(d) = postpone_days(from, to) {
            res.push(PostponedStrictBy(d));
            done_postponed_strict = true;
        }
    }

    // And then add the changes that we couldn't cram into one of the optimized versions
    if !done_recurred && !done_postponed_strict && from.threshold_date != to.threshold_date {
        res.push(ThresholdDate(from.threshold_date, to.threshold_date));
    }
    if !done_recurred && !done_postponed_strict && from.due_date != to.due_date {
        res.push(DueDate(from.due_date, to.due_date));
    }
    if !done_finished_at && from.finished != to.finished {
        res.push(Finished(to.finished));
    }
    if !done_finished_at && from.finish_date != to.finish_date {
        res.push(FinishDate(from.finish_date, to.finish_date));
    }
    if from.priority != to.priority {
        let from_prio;
        if from.priority < 26 {
            from_prio = Some((b'A' + from.priority) as char);
        } else {
            from_prio = None;
        }
        let to_prio;
        if to.priority < 26 {
            to_prio = Some((b'A' + to.priority) as char);
        } else {
            to_prio = None;
        }
        if !(done_finished_at && to_prio.is_none()) {
            res.push(Priority(from_prio, to_prio));
        }
    }
    if !done_recurred && from.create_date != to.create_date {
        res.push(CreateDate(from.create_date, to.create_date));
    }
    if from.tags != to.tags {
        let mut from_t = from
            .tags
            .iter()
            .map(|(a, b)| (a.clone(), b.clone()))
            .collect::<Vec<(String, String)>>();
        let mut to_t = to
            .tags
            .iter()
            .map(|(a, b)| (a.clone(), b.clone()))
            .collect::<Vec<(String, String)>>();
        remove_common(&mut from_t, &mut to_t);
        res.push(Tags(from_t, to_t));
    }
    if from.subject != to.subject {
        res.push(Subject(from.subject.clone(), to.subject.clone()));
    }
    res
}

fn remove_common<T: Clone + Eq>(a: &mut Vec<T>, b: &mut Vec<T>) {
    for x in a.clone().into_iter() {
        if let Some(b_pos) = b.iter().position(|y| *y == x) {
            b.remove(b_pos);
            let a_pos = a.iter().position(|y| *y == x).expect("Internal error E003");
            a.remove(a_pos);
        }
    }
}

pub fn uncomplete(t: &Task) -> Task {
    let mut res = t.clone();
    res.finished = false;
    res.finish_date = None;
    res
}

fn is_task_admissible(from: &Task, other: &Task, allowed_divergence: usize) -> bool {
    let distance = levenshtein(&other.subject, &from.subject);
    distance * 100 / other.subject.len() <= allowed_divergence
}

// Compares two tasks to determine which is closest to a third task
fn cmp_tasks_3way(from: &Task, left: &Task, right: &Task) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let left_lev = levenshtein(&left.subject, &from.subject);
    let right_lev = levenshtein(&right.subject, &from.subject);
    if left_lev != right_lev {
        left_lev.cmp(&right_lev)
    } else {
        // TODO: compare on other fields
        Equal
    }
}

fn preferred_task_ids(t: &Task, tasks: &Vec<Task>, allowed_divergence: usize) -> Vec<usize> {
    let mut admissibles = tasks
        .iter()
        .enumerate()
        .filter(|(_, x)| is_task_admissible(&t, x, allowed_divergence))
        .collect::<Vec<_>>();

    admissibles.sort_unstable_by(|(_, left), (_, right)| cmp_tasks_3way(&t, left, right));

    admissibles.into_iter().map(|(i, _)| i).collect::<Vec<_>>()
}

pub fn compute_changeset(
    from: Vec<Task>,
    to: Vec<Task>,
    allowed_divergence: usize,
) -> (Vec<Task>, Vec<ChangedTask<Vec<Changes>>>) {
    use self::TaskDelta::*;

    // Compute for each task the candidate matches, ordered by preference
    let from_preferences_matrix = from
        .iter()
        .map(|t| preferred_task_ids(t, &to, allowed_divergence))
        .collect::<Vec<Vec<usize>>>();
    let to_preferences_matrix = to
        .iter()
        .map(|t| preferred_task_ids(t, &from, allowed_divergence))
        .collect::<Vec<Vec<usize>>>();

    // Compute a stable matching between the two task lists
    let (matching, _) =
        stable_marriage::stable_marriage(from_preferences_matrix, to_preferences_matrix);

    // Prepare `to` to be able to selectively move tasks out of it without invalidating indices
    let mut to = to.into_iter().map(Some).collect::<Vec<Option<Task>>>();

    // Extract changed and deleted tasks
    let mut matches = matching
        .into_iter()
        .zip(from.into_iter())
        .map(|(i, from)| {
            let delta = match i {
                Some(i) => {
                    let to = to[i].take().unwrap();
                    if from == to {
                        Identical
                    } else if from.tags.get("rec").is_some() && !from.finished {
                        Recurred(vec![to])
                    } else {
                        Changed(to)
                    }
                }
                None => Deleted,
            };
            ChangedTask {
                orig: from,
                delta: delta,
            }
        })
        .collect::<Vec<ChangedTask<Task>>>();

    // Extract new tasks
    let new_tasks = to
        .into_iter()
        // Get only the remaining Some(task)
        .flat_map(|x| x)
        // Separate recurred tasks from actual new ones
        .flat_map(|x| {
            let mut best_match = matches
                .iter_mut()
                .filter_map(|x| match x.delta {
                    Recurred(ref mut vec) => Some((&x.orig, vec)),
                    _ => None,
                })
                .filter(|(t, _)| is_task_admissible(t, &x, allowed_divergence))
                .min_by(|(left, _), (right, _)| cmp_tasks_3way(&x, left, right));
            if let Some((_, ref mut delta)) = best_match {
                delta.push(x);
                None
            } else {
                Some(x)
            }
        })
        .collect::<Vec<_>>();

    let changes = matches
        .into_iter()
        .map(|x| {
            let new_delta = match &x.delta {
                Identical => Identical,
                Deleted => Deleted,
                Changed(t) => Changed(changes_between(&x.orig, &t, true)),
                Recurred(tasks) => {
                    // TODO: compute changes more cleverly
                    let mut recurred = tasks
                        .iter()
                        .enumerate()
                        .map(|(i, to)| changes_between(&x.orig, to, i == 0))
                        .collect::<Vec<_>>();

                    if recurred.len() == 1 {
                        Changed(recurred.remove(0))
                    } else {
                        Recurred(recurred)
                    }
                }
            };
            ChangedTask {
                orig: x.orig,
                delta: new_delta,
            }
        })
        .collect::<Vec<ChangedTask<Vec<Changes>>>>();

    (new_tasks, changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use todo_txt::Task;

    fn cmp3(from: &str, left: &str, right: &str) -> std::cmp::Ordering {
        cmp_tasks_3way(
            &Task::from_str(from).unwrap(),
            &Task::from_str(left).unwrap(),
            &Task::from_str(right).unwrap(),
        )
    }

    #[test]
    fn test_cmp_3way() {
        use std::cmp::Ordering::*;
        assert_eq!(cmp3("do a thing", "do a thing", "do an thing"), Less);
        assert_eq!(cmp3("do a thing", "do an thing", "do a thingie"), Less);
        assert_eq!(cmp3("do a thing", "x do a thing", "do any thing"), Less);
    }
}
