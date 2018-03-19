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
    Completed(Option<TaskDate>),
    Unknown,
}

fn changes(from: &Task, to: &Task) -> Changes {
    if from.subject == to.subject && from.create_date == to.create_date &&
            from.threshold_date == to.threshold_date && from.due_date == to.due_date &&
            from.contexts == to.contexts && from.projects == to.projects &&
            from.hashtags == to.hashtags && from.tags == to.tags && from.finish_date == None &&
            from.finished == false && to.finished == true {
        Changes::Completed(to.finish_date)
    } else {
        Changes::Unknown
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
    for x in from.clone().into_iter() {
        if let Some(pos) = to.iter().position(|y| *y == x) {
            to.remove(pos);
            let pos = from.iter().position(|y| *y == x).expect("Internal error E003");
            from.remove(pos);
        }
    }

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
                    use Changes::*;
                    match changes(&t.orig, &to) {
                        Completed(Some(d)) => println!("    → Completed at {}", d),
                        Completed(None) => println!("    → Completed (without date)"),
                        Unknown if num_to == 1 => println!("    → Changed to ‘{}’", to),
                        Unknown => println!("    → Copied and changed to ‘{}’", to),
                    }
                }
            }
            println!();
        }
    }
}
