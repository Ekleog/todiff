/*
 * Copyright 2018 Leo Gaspard <todiff@leo.gaspard.ninja>
 * Copyright 2018 Nadrieril <nadrieril+todiff@gmail.com>
 *
 * Licensed under the MIT license, see LICENSE.
 */

extern crate ansi_term;
extern crate atty;
extern crate chrono;
extern crate clap;
extern crate diff;
extern crate itertools;
extern crate strsim;
extern crate todo_txt;

use ansi_term::{ANSIString, ANSIStrings};
use ansi_term::{Color, Style};
use ansi_term::Color::{Blue, Green, Red, Yellow};
use chrono::{Datelike, Duration};
use itertools::Itertools;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use strsim::levenshtein;
use todo_txt::Task;
use todo_txt::Date as TaskDate;


fn is_a_tty() -> bool {
    atty::is(atty::Stream::Stdout)
}
fn is_term_dumb() -> bool {
    env::var("TERM").ok() == Some(String::from("dumb"))
}

fn color<T>(colorize: bool, color: Color, e: &T) -> ANSIString
        where T: std::fmt::Display {
    let e_str = format!("{}", e);
    if colorize {
        color.paint(e_str)
    } else {
        ANSIString::from(e_str)
    }
}

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

#[derive(Clone)]
enum Changes {
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

fn is_recurred(c: &Changes) -> bool {
    use Changes::*;
    match *c {
        RecurredStrict => true,
        RecurredFrom(_) => true,
        _ => false,
    }
}

fn is_completion(c: &Changes) -> bool {
    use Changes::*;
    match *c {
        FinishedAt(_) => true,
        Finished(true) => true,
        _ => false,
    }
}

fn is_postponed(c: &Changes) -> bool {
    use Changes::*;
    match *c {
        PostponedStrictBy(_) => true,
        DueDate(Some(_), Some(_)) => true,
        _ => false,
    }
}

fn change_str(colorize: bool, c: &Changes) -> Vec<ANSIString> {
    use Changes::*;
    match *c {
        Created => vec!["created".into()],
        Copied => vec!["copied".into()],
        RecurredStrict => vec!["recurred (strict)".into()],
        RecurredFrom(d) => vec![format!("recurred (from {})", d).into()],

        FinishedAt(d) => vec![format!("completed on {}", d).into()],
        PostponedStrictBy(d) => vec![format!("postponed (strict) by {} days", d.num_days()).into()],

        Finished(true) => vec!["completed".into()],
        Finished(false) => vec!["uncompleted".into()],
        Priority(_, None) => vec!["removed priority".into()],
        Priority(None, Some(c)) => vec![format!("added priority ({})", c).into()],
        Priority(Some(_), Some(b)) => vec![format!("set priority to ({})", b).into()],
        FinishDate(_, None) => vec!["removed completion date".into()],
        FinishDate(None, Some(d)) => vec![format!("added completion date {}", d).into()],
        FinishDate(Some(_), Some(d)) => vec![format!("set completion date to {}", d).into()],
        CreateDate(_, None) => vec!["removed creation date".into()],
        CreateDate(None, Some(d)) => vec![format!("added creation date {}", d).into()],
        CreateDate(Some(_), Some(d)) => vec![format!("set creation date to {}", d).into()],
        Subject(ref s, ref t) if colorize => {
            let mut res = vec![ANSIString::from("changed subject ‘")];
            for d in diff::chars(s, t) {
                use diff::Result::*;
                match d {
                    Both(c, _) => res.push(c.to_string().into()),
                    Left(c) => res.push(Style::new().on(Red).paint(c.to_string())),
                    Right(c) => res.push(Style::new().on(Green).paint(c.to_string())),
                }
            }
            res.push("’".into());
            res
        }
        Subject(_, ref s) => vec![format!("set subject to ‘{}’", s).into()],
        DueDate(_, None) => vec!["removed due date".into()],
        DueDate(None, Some(d)) => vec![format!("added due date {}", d).into()],
        DueDate(Some(_), Some(d)) => vec![format!("postponed to {}", d).into()],
        ThresholdDate(_, None) => vec!["removed threshold date".into()],
        ThresholdDate(None, Some(d)) => vec![format!("added threshold date {}", d).into()],
        ThresholdDate(Some(_), Some(d)) => vec![format!("set threshold date to {}", d).into()],
        Tags(ref a, ref b) => {
            use itertools::Position::*;
            let mut res = String::new();
            if a.len() == 1 {
                res += "removed tag ";
            } else if a.len() > 1 {
                res += "removed tags ";
            }
            for t in a.iter().with_position() {
                match t {
                    First(t) | Only(t) => res += &format!("{}:{}", t.0, t.1),
                    Middle(t) => res += &format!(", {}:{}", t.0, t.1),
                    Last(t) => res += &format!(" and {}:{}", t.0, t.1),
                };
            }
            if !a.is_empty() && !b.is_empty() {
                res += " and ";
            }
            if b.len() == 1 {
                res += "added tag ";
            } else if b.len() > 1 {
                res += "added tags ";
            }
            for t in b.iter().with_position() {
                match t {
                    First(t) | Only(t) => res += &format!("{}:{}", t.0, t.1),
                    Middle(t) => res += &format!(", {}:{}", t.0, t.1),
                    Last(t) => res += &format!(" and {}:{}", t.0, t.1),
                };
            }
            vec![res.into()]
        }
    }
}

fn postpone_days(from: &Task, to: &Task) -> Option<Duration> {
    if let Some(from_due) = from.due_date {
        if let Some(to_due) = to.due_date {
            if from.threshold_date == None && to.threshold_date == None {
                return Some(to_due.signed_duration_since(from_due));
            }
            if let Some(from_thresh) = from.threshold_date {
                if let Some(to_thresh) = to.threshold_date {
                    if to_due.signed_duration_since(from_due) ==
                            to_thresh.signed_duration_since(from_thresh) {
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
            Some('m') =>
                Some(date.with_month0((date.month0() + n as u32) % 12)
                         .expect("Internal error E006")
                         .with_year(date.year() + ((date.month0() + n as u32) / 12) as i32)
                         .expect("Internal error E007")),
            Some('y') => Some(date.with_year(date.year() + n as i32)
                                  .expect("Internal error E008")),
            _ => None,
        }
    } else {
        None
    }
}

fn changes_between(from: &Task, to: &Task, is_first: bool) -> Vec<Changes> {
    use Changes::*;

    let mut res = Vec::new();
    let mut done_recurred = false;
    let mut done_finished_at = false;
    let mut done_postponed_strict = false;

    // First, things that may trigger a removal of the `copied` item
    if !is_first && from.tags.get("rec") == to.tags.get("rec") {
        if let (Some(r), Some(_), Some(from_due), Some(to_due)) =
                (from.tags.get("rec"), postpone_days(from, to), from.due_date, to.due_date) {
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
    if from.finished == false && to.finished == true &&
            from.finish_date.is_none() && to.finish_date.is_some() {
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
        let mut from_t = from.tags.iter().map(|(a, b)| (a.clone(), b.clone())).collect::<Vec<(String, String)>>();
        let mut to_t = to.tags.iter().map(|(a, b)| (a.clone(), b.clone())).collect::<Vec<(String, String)>>();
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

fn has_been_recurred(chgs: &Vec<Vec<Changes>>) -> bool {
    chgs.iter().flat_map(|c| c).any(is_recurred)
}

fn has_been_completed(chgs: &Vec<Vec<Changes>>) -> bool {
    chgs.iter().flat_map(|c| c).any(is_completion)
}

fn has_been_postponed(chgs: &Vec<Vec<Changes>>) -> bool {
    chgs.iter().flat_map(|c| c).any(is_postponed)
}

fn uncomplete(t: &Task) -> Task {
    let mut res = t.clone();
    res.finished = false;
    res.finish_date = None;
    res
}

fn display_changes(colorize: bool, chgs_for_me: Vec<Changes>) {
    use itertools::Position::*;
    print!("    → ");
    for c in chgs_for_me.into_iter().with_position() {
        match c {
            First(c) | Only(c) => {
                let chg = change_str(colorize, &c);
                let mut chars = chg[0].chars();
                let first_char = chars.next().expect("Internal error E004")
                    .to_uppercase();
                print!("{}{}{}", first_char, chars.as_str(), ANSIStrings(&chg[1..]));
            }
            Middle(c) => {
                print!(", {}", ANSIStrings(&change_str(colorize, &c)));
            }
            Last(c) => {
                print!(" and {}", ANSIStrings(&change_str(colorize, &c)));
            }
        };
    }
    println!();
}

fn main() {
    // Read arguments
    let matches = clap::App::new("todiff")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Leo Gaspard <todiff@leo.gaspard.ninja>")
        .about("Diffs two todo.txt files")
        .args_from_usage("
            <BEFORE>        'The file to diff from'
            <AFTER>         'The file to diff to'
        ")
        .arg(clap::Arg::with_name("color")
            .long("color")
            .takes_value(true)
            .possible_values(&["auto", "always", "never"])
            .default_value("auto")
            .help("Colorize the output"))
        .arg(clap::Arg::with_name("similarity")
             .long("similarity")
             .takes_value(true)
             .validator(|s| s.parse::<usize>()
                             .map_err(|e| format!("{}", e))
                             .and_then(|x| if x <= 100 { Ok(()) }
                                           else { Err("must be between 0 and 100".to_owned()) }))
             .default_value("75")
             .help("Similarity index to consider two tasks identical (in percents, higher is more restrictive)"))
        .get_matches();

    let color_option = matches.value_of("color").expect("Internal error E009");
    let colorize = match color_option {
        "never" => false,
        "always" => true,
        "auto" => is_a_tty() && !is_term_dumb(),
        _ => panic!("Internal error E010")
    };

    let similarity_option = matches.value_of("similarity").expect("Internal error E011");
    let similarity = similarity_option.parse::<usize>().expect("Internal error E012");
    let allowed_divergence = 100 - similarity;

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
            .and_then(|t| {
                let distance = levenshtein(&t.orig.subject, &x.subject);
                if distance * 100 / t.orig.subject.len() < allowed_divergence { Some(t) }
                else { None }
            });
        if let Some(best) = best_match {
            best.to.push(x);
        } else {
            new_tasks.push(x);
        }
    }

    // Retrieve changes
    let mut changes = changeset.into_iter()
        .map(|t| {
            let changes = t.to.iter()
                .enumerate()
                .map(|(i, to)| changes_between(&t.orig, &to, i == 0)).collect();
            (t.orig, changes)
        })
        .collect::<Vec<(Task, Vec<Vec<Changes>>)>>();

    // Sort tasks
    new_tasks.sort_by_key(|x| x.create_date);
    changes.sort_by_key(|&(_, ref chgs)| {
        if has_been_recurred(chgs) { 100 }
        else if has_been_completed(chgs) { 200 }
        else if has_been_postponed(chgs) { 300 }
        else if chgs.is_empty() { 400 }
        else { 500 }
    });

    // Sort changes by category
    let category_new = new_tasks.iter().filter(|x| !x.finished).cloned().collect::<Vec<Task>>();
    let category_deleted = changes.iter()
                                  .filter(|&&(_, ref to)| to.is_empty())
                                  .map(|&(ref from, _)| from.clone())
                                  .collect::<Vec<Task>>();
    let category_completed = changes.iter()
                                    .filter(|&&(_, ref to)| has_been_recurred(to) ||
                                                            has_been_completed(to))
                                    .cloned()
                                    .chain(new_tasks.iter()
                                                    .filter(|x| x.finished)
                                                    .map(|x| { let u = uncomplete(x);
                                                               let mut c = changes_between(&u, &x, true);
                                                               let mut chgs = vec![Changes::Created];
                                                               chgs.append(&mut c);
                                                               (u, vec![chgs]) }))
                                    .collect::<Vec<(Task, Vec<Vec<Changes>>)>>();
    let category_changed = changes.iter()
                                  .filter(|&&(_, ref to)| !has_been_recurred(to) &&
                                                          !has_been_completed(to) &&
                                                          !to.is_empty())
                                  .cloned()
                                  .collect::<Vec<(Task, Vec<Vec<Changes>>)>>();
    let no_changes = category_new.is_empty() && category_deleted.is_empty() &&
                     category_completed.is_empty() && category_changed.is_empty();

    // Nice display
    if no_changes {
        println!("No changes.");
    }

    let mut is_first = true;
    if !category_new.is_empty() {
        is_first = false;
        println!("New tasks");
        println!("---------");
        println!();
        for t in category_new {
            println!(" → {}", color(colorize, Green, &t));
        }
    }

    if !category_deleted.is_empty() {
        if !is_first { println!() }
        is_first = false;
        println!("Deleted tasks");
        println!("-------------");
        println!();
        for t in category_deleted {
            println!(" → {}", color(colorize, Red, &t));
        }
    }

    if !category_completed.is_empty() {
        if !is_first { println!() }
        is_first = false;
        println!("Completed tasks");
        println!("---------------");
        for (t, c) in category_completed {
            println!();

            if has_been_recurred(&c) {
                println!(" → {}", color(colorize, Green, &t));
            } else {
                println!(" → {}", color(colorize, Blue, &t));
            }

            for chgs in c {
                display_changes(colorize, chgs);
            }
        }
    }

    if !category_changed.is_empty() {
        if !is_first { println!() }
        println!("Changed tasks");
        println!("-------------");
        for (t, c) in category_changed {
            println!();

            if has_been_postponed(&c) {
                println!(" → {}", color(colorize, Yellow, &t));
            } else {
                println!(" → {}", t);
            }

            for chgs in c {
                display_changes(colorize, chgs);
            }
        }
    }
}
