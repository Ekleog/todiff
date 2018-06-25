#[macro_use]
extern crate pretty_assertions;
extern crate chrono;
extern crate todiff;
extern crate todo_txt;
use self::Changes::*;
use self::TaskDelta::*;
use chrono::Duration;
use std::str::FromStr;
use todiff::compute_changes::*;
use todo_txt::Date as TaskDate;
use todo_txt::Task;

fn tasks_from_strings(strings: Vec<&str>) -> Vec<Task> {
    strings
        .into_iter()
        .map(|x| Task::from_str(&x).unwrap())
        .collect()
}

fn changes_from_strings(
    changes: Vec<(&str, TaskDelta<Vec<Changes>>)>,
) -> Vec<ChangedTask<Vec<Changes>>> {
    changes
        .into_iter()
        .map(|(str, delta)| {
            let t = Task::from_str(str).unwrap();
            ChangedTask {
                orig: t,
                delta: delta,
            }
        })
        .collect()
}

#[test]
fn new_tasks() {
    let from = tasks_from_strings(vec!["do a thing"]);
    let to = tasks_from_strings(vec!["do a thing", "do another thing"]);
    let (new_tasks, changes) = compute_changeset(from, to, 0);

    assert_eq!(new_tasks, tasks_from_strings(vec!["do another thing"]));
    assert_eq!(changes, vec![]);
}

#[test]
fn copy_task() {
    let from = tasks_from_strings(vec!["do a thing"]);
    let to = tasks_from_strings(vec!["do a thing", "do a thing"]);
    let (new_tasks, changes) = compute_changeset(from, to, 0);

    assert_eq!(new_tasks, tasks_from_strings(vec!["do a thing"]));
    assert_eq!(changes, vec![]);

    // TODO: Unwanted behaviour
    let from = tasks_from_strings(vec!["do a thing"]);
    let to = tasks_from_strings(vec!["x do a thing", "x do a thing"]);
    let (new_tasks, changes) = compute_changeset(from, to, 0);

    assert_eq!(
        new_tasks,
        tasks_from_strings(vec!["x do a thing", "x do a thing"])
    );
    assert_eq!(changes, changes_from_strings(vec![("do a thing", Deleted)]));
}

#[test]
fn delete_task() {
    let from = tasks_from_strings(vec!["do a thing"]);
    let to = tasks_from_strings(vec!["what is this ?"]);
    let (new_tasks, changes) = compute_changeset(from, to, 30);

    assert_eq!(new_tasks, tasks_from_strings(vec!["what is this ?"]));
    assert_eq!(changes, changes_from_strings(vec![("do a thing", Deleted)]));
}

#[test]
fn change_subject() {
    let from = tasks_from_strings(vec!["do a thing", "eat a hamburger"]);
    let to = tasks_from_strings(vec!["drink a hamburger", "do an thing"]);
    let (new_tasks, changes) = compute_changeset(from, to, 40);

    assert_eq!(new_tasks, vec![]);
    assert_eq!(
        changes,
        changes_from_strings(vec![
            (
                "do a thing",
                Changed(vec![Subject(
                    "do a thing".to_string(),
                    "do an thing".to_string(),
                )]),
            ),
            (
                "eat a hamburger",
                Changed(vec![Subject(
                    "eat a hamburger".to_string(),
                    "drink a hamburger".to_string(),
                )]),
            ),
        ])
    );

    let from = tasks_from_strings(vec!["do a thing"]);
    let to = tasks_from_strings(vec!["do an thing", "x do a thing"]);
    let (new_tasks, changes) = compute_changeset(from, to, 40);

    assert_eq!(new_tasks, vec![Task::from_str("do an thing").unwrap()]);
    assert_eq!(
        changes,
        changes_from_strings(vec![("do a thing", Changed(vec![Finished(true)]))])
    );
}

#[test]
fn recurring_tasks() {
    // TODO: Unwanted behaviour
    let from = tasks_from_strings(vec!["2018-04-08 foo due:2018-04-08 rec:1d"]);
    let to = tasks_from_strings(vec![
        "x 2018-04-08 2018-04-08 foo due:2018-04-08 rec:1d",
        "x 2018-04-08 2018-04-08 foo due:2018-04-09 rec:1d",
        "2018-04-08 foo due:2018-04-10 rec:1d",
        "2018-04-08 bar",
    ]);
    let (new_tasks, changes) = compute_changeset(from, to, 50);

    assert_eq!(new_tasks, tasks_from_strings(vec!["2018-04-08 bar"]));
    assert_eq!(
        changes,
        changes_from_strings(vec![(
            "2018-04-08 foo due:2018-04-08 rec:1d",
            Recurred(vec![
                vec![FinishedAt(TaskDate::from_ymd(2018, 4, 8))],
                vec![
                    RecurredFrom(TaskDate::from_ymd(2018, 4, 8)),
                    FinishedAt(TaskDate::from_ymd(2018, 4, 8)),
                ],
                vec![Copied, PostponedStrictBy(Duration::days(2))],
            ]),
        )])
    );

    // TODO: Unwanted behaviour
    let from = tasks_from_strings(vec!["2018-06-01 foo due:2018-06-20 rec:1m"]);
    let to = tasks_from_strings(vec![
        "x 2018-06-17 2018-06-01 foo due:2018-06-15 rec:1m",
        "2018-06-17 foo due:2018-07-15 rec:1m",
    ]);
    let (new_tasks, changes) = compute_changeset(from, to, 50);

    assert_eq!(new_tasks, vec![]);
    assert_eq!(
        changes,
        changes_from_strings(vec![(
            "2018-06-01 foo due:2018-06-20 rec:1m",
            Recurred(vec![
                vec![
                    FinishedAt(TaskDate::from_ymd(2018, 6, 17)),
                    PostponedStrictBy(Duration::days(-5)),
                ],
                vec![
                    Copied,
                    PostponedStrictBy(Duration::days(25)),
                    CreateDate(
                        Some(TaskDate::from_ymd(2018, 6, 1)),
                        Some(TaskDate::from_ymd(2018, 6, 17)),
                    ),
                ],
            ]),
        )])
    );
}
