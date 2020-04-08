use chrono::Duration;
use itertools::Either;
use itertools::Itertools;
use stable_marriage;
use std;
use strsim::levenshtein;
use todo_txt::task::Extended as Task;
use todo_txt::task::Recurrence;
use todo_txt::Date as TaskDate;

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
    RecurredStrict,
    RecurredFrom(Option<TaskDate>),

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

fn delta_task_dates(from: &Task, to: &Task) -> Option<Duration> {
    if let Some(from_due) = from.due_date {
        if let Some(to_due) = to.due_date {
            match (from.threshold_date, to.threshold_date) {
                (None, None) => {
                    let due_delta = to_due.signed_duration_since(from_due);
                    return Some(due_delta);
                }
                (Some(from_thresh), Some(to_thresh)) => {
                    let due_delta = to_due.signed_duration_since(from_due);
                    let thresh_delta = to_thresh.signed_duration_since(from_thresh);
                    if due_delta == thresh_delta {
                        return Some(due_delta);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn recur_task(from: &Task, rec: Recurrence) -> (Task, Changes) {
    let mut new_task = from.clone();
    new_task.uncomplete();

    let from_finish = from.finish_date;
    let change;
    if rec.strict {
        change = Changes::RecurredStrict;
        new_task.due_date = from.due_date.map(|d| rec.clone() + d);
        new_task.threshold_date = from.threshold_date.map(|d| rec + d);
    } else {
        change = Changes::RecurredFrom(from_finish);
        new_task.due_date = from_finish.map(|d| rec + d);
        match (from.due_date, from.threshold_date) {
            (Some(from_due), Some(from_thresh)) => {
                let delta = from_due.signed_duration_since(from_thresh);
                new_task.threshold_date = new_task.due_date.map(|d| d - delta);
            }
            _ => {}
        }
    }

    if let Some(_) = from_finish {
        new_task.create_date = from_finish;
    }

    (new_task, change)
}

pub fn changes_between(from: &Task, to: &Task) -> Vec<Changes> {
    use self::Changes::*;

    let mut res = Vec::new();

    // Completion
    let mut done_finished_at = false;
    if (from.finished == false)
        && to.finished == true
        && from.finish_date.is_none()
        && to.finish_date.is_some()
    {
        res.push(FinishedAt(to.finish_date.expect("Internal error E005")));
        done_finished_at = true;
    }
    if !done_finished_at && from.finished != to.finished {
        res.push(Finished(to.finished));
    }
    if !done_finished_at && from.finish_date != to.finish_date {
        res.push(FinishDate(from.finish_date, to.finish_date));
    }

    // Dates
    let mut done_postponed_strict = false;
    if from.due_date != to.due_date {
        if let Some(d) = delta_task_dates(from, to) {
            res.push(PostponedStrictBy(d));
            done_postponed_strict = true;
        }
    }
    if !done_postponed_strict && from.threshold_date != to.threshold_date {
        res.push(ThresholdDate(from.threshold_date, to.threshold_date));
    }
    if !done_postponed_strict && from.due_date != to.due_date {
        res.push(DueDate(from.due_date, to.due_date));
    }
    if from.create_date != to.create_date {
        res.push(CreateDate(from.create_date, to.create_date));
    }

    // Other changes
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

fn changes_between_rec(mut from: Task, to: Task, orig: &Task) -> Vec<Changes> {
    let rec = orig.recurrence.clone().unwrap();
    // If the finish date of `from` was not recorded, infer it from `to`
    if from.finished && from.finish_date == None {
        from.finish_date = to.create_date;
    }
    let (mut virtual_task, recur_change) = recur_task(&from, rec);
    // Work around priority being removed on completion
    if orig.priority < 26 {
        virtual_task.priority = orig.priority;
    }

    std::iter::once(recur_change)
        .chain(changes_between(&virtual_task, &to))
        .collect::<Vec<Changes>>()
}

pub fn remove_common<T: Clone + Eq>(a: &mut Vec<T>, b: &mut Vec<T>) -> Vec<T> {
    a.clone()
        .into_iter()
        .enumerate()
        .rev()
        .filter_map(|(i, x)| {
            if let Some(b_pos) = b.iter().position(|y| *y == x) {
                b.swap_remove(b_pos);
                Some(a.swap_remove(i))
            } else {
                None
            }
        })
        .collect()
}

fn is_task_admissible(from: &Task, other: &Task, allowed_divergence: usize) -> bool {
    // The levenshtein distance is at least the difference between the lenghts
    if 100 * (other.subject.len() as i64 - from.subject.len() as i64).abs()
        > allowed_divergence as i64 * other.subject.len() as i64
    {
        return false;
    }
    let distance = levenshtein(&other.subject, &from.subject);
    distance * 100 <= allowed_divergence * other.subject.len()
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

struct TaskMatcher {
    allowed_divergence: usize,
}

impl stable_marriage::Matcher for TaskMatcher {
    type Item = Task;
    type Target = Task;

    fn is_admissible(&self, x: &Self::Item, y: &Self::Target) -> bool {
        is_task_admissible(x, y, self.allowed_divergence)
    }

    fn is_perfect_match(&self, x: &Self::Item, y: &Self::Target) -> bool {
        x == y
    }

    fn cmp_3way(
        &self,
        from: &Self::Item,
        left: &Self::Target,
        right: &Self::Target,
    ) -> std::cmp::Ordering {
        cmp_tasks_3way(from, left, right)
    }
}

pub fn match_tasks(
    from: Vec<Task>,
    to: Vec<Task>,
    allowed_divergence: usize,
) -> (Vec<Task>, Vec<ChangedTask<Task>>) {
    use self::TaskDelta::*;

    let matcher = TaskMatcher {
        allowed_divergence: allowed_divergence,
    };

    // Compute a stable matching between the two task lists
    let (matches, new_tasks) = stable_marriage::stable_marriage(to, from, &matcher, &matcher);

    // Extract changed and deleted tasks
    let mut matches = matches
        .into_iter()
        .map(|(from, mtch)| {
            let delta = match mtch {
                Some(to) => {
                    if from == to {
                        Identical
                    } else if from.recurrence.is_some() && !from.finished {
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
    let new_tasks = new_tasks
        .into_iter()
        // Separate recurred tasks from actual new ones
        .flat_map(|x| {
            let mut best_match = matches
                .iter_mut()
                .filter_map(|x| match x.delta {
                    Recurred(ref mut recurred) => Some((&x.orig, recurred)),
                    _ => None,
                })
                .filter(|(t, _)| is_task_admissible(t, &x, allowed_divergence))
                .min_by(|(left, _), (right, _)| cmp_tasks_3way(&x, left, right));
            if let Some((_, ref mut recurred)) = best_match {
                recurred.push(x);
                None
            } else {
                Some(x)
            }
        })
        .collect::<Vec<_>>();

    let matches = matches
        .into_iter()
        .map(|ChangedTask { orig, delta }| {
            let new_delta = match delta {
                Recurred(mut recurred) => {
                    if recurred.len() == 1 {
                        Changed(recurred.remove(0))
                    } else {
                        recurred.sort_by_key(|t| t.due_date);
                        Recurred(recurred)
                    }
                }
                _ => delta,
            };
            ChangedTask {
                orig: orig,
                delta: new_delta,
            }
        })
        .collect::<Vec<ChangedTask<Task>>>();

    (new_tasks, matches)
}

pub fn compute_changeset(
    from: Vec<Task>,
    to: Vec<Task>,
    allowed_divergence: usize,
) -> (Vec<Task>, Vec<ChangedTask<Vec<Changes>>>) {
    use self::TaskDelta::*;
    let (new_tasks, matches) = match_tasks(from, to, allowed_divergence);

    let changes = matches
        .into_iter()
        .map(|ChangedTask { orig, delta }| {
            let new_delta = match delta {
                Identical => Identical,
                Deleted => Deleted,
                Changed(t) => Changed(changes_between(&orig, &t)),
                Recurred(tasks) => {
                    let init_change = changes_between(&orig, &tasks[0]);
                    let rec_changes = tasks
                        .into_iter()
                        .tuple_windows()
                        .map(|(t1, t2)| changes_between_rec(t1, t2, &orig));
                    let all_changes = std::iter::once(init_change)
                        .chain(rec_changes)
                        .collect::<Vec<_>>();
                    Recurred(all_changes)
                }
            };
            ChangedTask {
                orig: orig,
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
    use todo_txt::task::Extended as Task;

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

    #[test]
    fn test_add_recspec() {
        fn test(from: &str, rec: &str, to: &str) {
            let from = TaskDate::from_str(from).unwrap();
            let to = TaskDate::from_str(to).unwrap();
            assert_eq!(Recurrence::from_str(rec).unwrap() + from, to);
        }

        test("2010-01-01", "2d", "2010-01-03");
        test("2010-01-01", "2w", "2010-01-15");
        test("2010-01-01", "2m", "2010-03-01");
        test("2010-01-01", "2y", "2012-01-01");

        test("2010-01-30", "1m", "2010-02-28");
        test("2010-02-28", "1m", "2010-03-31");
        test("2010-01-30", "2m", "2010-03-30");
        // test("2010-01-01", "20m", "2011-09-01");
        test("2003-02-28", "1y", "2004-02-29");
        test("2004-02-29", "1y", "2005-02-28");
    }
}
