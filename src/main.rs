extern crate clap;
extern crate strsim;
extern crate todo_txt;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use strsim::levenshtein;
use todo_txt::Task;
use todo_txt::Date as TaskDate;

fn read_tasks(path: &str) -> Vec<Task> {
    let file = File::open(path).expect(&format!("Unable to open file ‘{}’", path));
    let reader = BufReader::new(&file);
    let mut res = Vec::new();
    for line in reader.lines() {
        let line = line.expect(&format!("Unable to read file ‘{}’", path));
        res.push(Task::from_str(&line)
                    .expect(&format!("Unable to parse line in file ‘{}’:\n{}", path, line)));
    }
    res
}

#[derive(Debug)]
struct TaskChange {
    orig: Task,
    to: Vec<Task>,
}

enum Changes {
    Finished(bool),
    Priority(Option<char>),
    FinishDate(Option<TaskDate>),
    CreateDate(Option<TaskDate>),
    Subject(String),
    DueDate(Option<TaskDate>),
    ThresholdDate(Option<TaskDate>),
    Tags((Vec<(String, String)>, Vec<(String, String)>)),
}

fn change_str(c: &Changes) -> String {
    use Changes::*;
    match *c {
        Finished(true) => "completed".to_owned(),
        Finished(false) => "uncompleted".to_owned(),
        Priority(None) => "priority removed".to_owned(),
        Priority(Some(c)) => format!("priority set to ({})", c),
        FinishDate(None) => "completion date removed".to_owned(),
        FinishDate(Some(d)) => format!("completion date set to {}", d),
        CreateDate(None) => "creation date removed".to_owned(),
        CreateDate(Some(d)) => format!("creation date set to {}", d),
        Subject(ref s) => format!("subject set to ‘{}’", s),
        DueDate(None) => "due date removed".to_owned(),
        DueDate(Some(d)) => format!("due date set to {}", d),
        ThresholdDate(None) => "threshold date removed".to_owned(),
        ThresholdDate(Some(d)) => format!("threshold date set to {}", d),
        Tags((ref a, ref b)) => {
            let mut res = String::new();
            if a.len() == 1 {
                res += "removed tag ";
            } else if a.len() > 1 {
                res += "removed tags ";
            }
            for t in a {
                res += &format!("{}:{}", t.0, t.1);
            }
            if !a.is_empty() && !b.is_empty() {
                res += " and ";
            }
            if b.len() == 1 {
                res += "added tag ";
            } else if b.len() > 1 {
                res += "added tags ";
            }
            for t in b {
                res += &format!("{}:{}", t.0, t.1);
            }
            res
        }
    }
}

fn changes(from: &Task, to: &Task) -> Vec<Changes> {
    use Changes::*;

    let mut res = Vec::new();
    if from.threshold_date != to.threshold_date {
        res.push(ThresholdDate(to.threshold_date));
    }
    if from.due_date != to.due_date {
        res.push(DueDate(to.due_date));
    }
    if from.finished != to.finished {
        res.push(Finished(to.finished));
    }
    if from.finish_date != to.finish_date {
        res.push(FinishDate(to.finish_date));
    }
    if from.priority != to.priority {
        if to.priority < 26 {
            res.push(Priority(Some((b'A' + to.priority) as char)));
        } else {
            res.push(Priority(None));
        }
    }
    if from.create_date != to.create_date {
        res.push(CreateDate(to.create_date));
    }
    if from.tags != to.tags {
        let mut from_t = from.tags.iter().map(|(a, b)| (a.clone(), b.clone())).collect::<Vec<(String, String)>>();
        let mut to_t = to.tags.iter().map(|(a, b)| (a.clone(), b.clone())).collect::<Vec<(String, String)>>();
        remove_common(&mut from_t, &mut to_t);
        res.push(Tags((from_t, to_t)));
    }
    if from.subject != to.subject {
        res.push(Subject(to.subject.clone()));
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

fn main() {
    // Read arguments
    let matches = clap::App::new("Todiff")
        .version("0.1.0")
        .author("Leo Gaspard <todiff@leo.gaspard.ninja>")
        .about("Diffs two todo.txt files")
        .args_from_usage(
            "<BEFORE>     'The file to diff from'
            <AFTER>        'The file to diff to'")
        .get_matches();

    // Read files
    let mut from = read_tasks(matches.value_of("BEFORE").expect("Internal error E001"));
    let mut to = read_tasks(matches.value_of("AFTER").expect("Internal error E002"));

    // Remove elements in common
    remove_common(&mut from, &mut to);

    // Prepare the changeset
    let mut changeset = Vec::new();
    for x in from.into_iter() {
        changeset.push(TaskChange {
            orig: x,
            to: Vec::new(),
        });
    }

    // Add all right-hand tasks to the changeset
    let mut new_tasks = Vec::new();
    for x in to.into_iter() {
        let best_match = changeset.iter_mut()
            .min_by_key(|t| levenshtein(&t.orig.subject, &x.subject))
            .and_then(|t|
                if levenshtein(&t.orig.subject, &x.subject) * 100 / t.orig.subject.len() < 50 { Some(t) }
                else { None }
            );
        if let Some(best) = best_match {
            best.to.push(x);
        } else {
            new_tasks.push(x);
        }
    }

    // Nice display
    if new_tasks.is_empty() {
        println!("No new tasks.\n");
    } else {
        println!("New tasks:");
        for t in new_tasks {
            println!(" → {}", t);
        }
        println!();
    }
    if changeset.is_empty() {
        println!("No changed tasks.\n");
    } else {
        println!("Changed tasks:");
        for t in changeset {
            println!(" → {}", t.orig);
            if t.to.is_empty() {
                println!("    → Deleted task");
            } else {
                let num_to = t.to.len();
                for to in t.to {
                    print!("    → ");
                    let chgs = changes(&t.orig, &to);
                    for c in 0..chgs.len() {
                        let chg = change_str(&chgs[c]);
                        if c == 0 {
                            let mut chrs = chg.chars();
                            let first_chr = chrs.next().expect("Internal error E004").to_uppercase();
                            print!("{}{}", first_chr, chrs.as_str());
                        } else {
                            print!("{}", chg);
                        }
                        if c < chgs.len() - 2 {
                            print!(", ");
                        } else if c == chgs.len() - 2 {
                            print!(" and ");
                        }
                    }
                    println!();
                }
            }
            println!();
        }
    }
}
