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
extern crate todiff;

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use todo_txt::Task;
use todiff::task_change::*;


fn is_a_tty() -> bool {
    atty::is(atty::Stream::Stdout)
}
fn is_term_dumb() -> bool {
    env::var("TERM").ok() == Some(String::from("dumb"))
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
    let from = read_tasks(matches.value_of("BEFORE").expect("Internal error E001"));
    let to = read_tasks(matches.value_of("AFTER").expect("Internal error E002"));
    let (new_tasks, changes) = compute_changeset(from, to, allowed_divergence);
    display_changeset(new_tasks, changes, colorize);
}
