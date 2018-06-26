extern crate clap;
extern crate todiff;
extern crate todo_txt;

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use todiff::merge_changes::*;
use todo_txt::Task;

fn read_tasks(path: &str) -> Vec<Task> {
    let file = File::open(path).expect(&format!("Unable to open file ‘{}’", path));
    let reader = BufReader::new(&file);
    let mut res = Vec::new();
    for line in reader.lines() {
        let line = line.expect(&format!("Unable to read file ‘{}’", path));
        res.push(Task::from_str(&line).expect(&format!(
            "Unable to parse line in file ‘{}’:\n{}",
            path, line
        )));
    }
    res
}

fn main_exitcode() -> i32 {
    // Read arguments
    let matches = clap::App::new("todiff-merge")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Leo Gaspard <todiff@leo.gaspard.ninja>")
        .about("Performs a 3-way merge of todo.txt files")
        .args_from_usage("
            <ANCESTOR>      'The original file'
            <CURRENT>       'The first file to merge'
            <OTHER>         'The second file to merge'
        ")
        .arg(clap::Arg::with_name("similarity")
             .long("similarity")
             .takes_value(true)
             .validator(|s| s.parse::<usize>()
                             .map_err(|e| format!("{}", e))
                             .and_then(|x| if x <= 100 { Ok(()) }
                                           else { Err("must be between 0 and 100".to_owned()) }))
             .default_value("75")
             .help("Similarity index to consider two tasks identical (in percents, higher is more restrictive)"))
        .arg(clap::Arg::with_name("overwrite")
             .long("overwrite")
             .takes_value(false)
             .help("Overwrites <CURRENT> with the result of the merge, as expected by git"))
        .get_matches();

    let similarity_option = matches.value_of("similarity").expect("Internal error E011");
    let similarity = similarity_option
        .parse::<usize>()
        .expect("Internal error E012");
    let overwrite = matches.is_present("overwrite");
    let allowed_divergence = 100 - similarity;

    let current = matches.value_of("CURRENT").expect("Internal error E002");
    let from = read_tasks(matches.value_of("ANCESTOR").expect("Internal error E001"));
    let left = read_tasks(current);
    let right = read_tasks(matches.value_of("OTHER").expect("Internal error E003"));

    let changes = merge_3way(from, left, right, allowed_divergence);
    let success = merge_successful(&changes);
    let output = merge_to_string(changes);

    if overwrite {
        fs::write(current, output).expect(&format!("Unable to write to file ‘{}’", current));
    } else {
        println!("{}", output);
    }
    return if success { 0 } else { 1 };
}

// Need a separate function because exit() does not run destructors
fn main() {
    let exit_code = main_exitcode();
    std::process::exit(exit_code);
}
