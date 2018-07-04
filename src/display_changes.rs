use ansi_term::Color::{Blue, Green, Red, Yellow};
use ansi_term::{ANSIString, ANSIStrings};
use ansi_term::{Color, Style};
use compute_changes::*;
use diff;
use itertools::Itertools;
use std;
use todo_txt::Task;

fn is_recurred(c: &Changes) -> bool {
    use self::Changes::*;
    match *c {
        RecurredStrict => true,
        RecurredFrom(_) => true,
        _ => false,
    }
}
fn is_completion(c: &Changes) -> bool {
    use self::Changes::*;
    match *c {
        FinishedAt(_) => true,
        Finished(true) => true,
        _ => false,
    }
}
fn is_postponed(c: &Changes) -> bool {
    use self::Changes::*;
    match *c {
        PostponedStrictBy(_) => true,
        DueDate(Some(_), Some(_)) => true,
        _ => false,
    }
}

fn has_been_recurred(x: &ChangedTask<Vec<Changes>>) -> bool {
    x.delta.iter().flat_map(|c| c).any(is_recurred)
}
fn has_been_completed(x: &ChangedTask<Vec<Changes>>) -> bool {
    x.delta.iter().flat_map(|c| c).any(is_completion)
}
fn has_been_postponed(x: &ChangedTask<Vec<Changes>>) -> bool {
    x.delta.iter().flat_map(|c| c).any(is_postponed)
}

fn color<T>(colorize: bool, color: Color, e: &T) -> ANSIString
where
    T: std::fmt::Display,
{
    let e_str = format!("{}", e);
    if colorize {
        color.paint(e_str)
    } else {
        ANSIString::from(e_str)
    }
}

fn change_str(colorize: bool, c: &Changes) -> Vec<ANSIString> {
    use self::Changes::*;
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

fn display_changes(colorize: bool, chgs_for_me: &Vec<Changes>) {
    use itertools::Position::*;
    print!("    → ");
    for c in chgs_for_me.into_iter().with_position() {
        match c {
            First(c) | Only(c) => {
                let chg = change_str(colorize, &c);
                let mut chars = chg[0].chars();
                let first_char = chars.next().expect("Internal error E004").to_uppercase();
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

pub fn display_changeset(
    new_tasks: Vec<Task>,
    changes: Vec<ChangedTask<Vec<Changes>>>,
    colorize: bool,
) {
    use self::TaskDelta::*;

    // Sort changes by category
    let (completed_new_tasks, mut category_new) =
        new_tasks.into_iter().partition::<Vec<_>, _>(|x| x.finished);

    let category_deleted = changes
        .iter()
        .filter(|x| x.delta == Deleted)
        .map(|x| x.orig.clone())
        .collect::<Vec<Task>>();

    let category_completed = changes
        .iter()
        .filter(|x| has_been_recurred(x) || has_been_completed(x))
        .cloned()
        .chain(completed_new_tasks.into_iter().map(|x| {
            let u = uncomplete(&x);
            let c = changes_between(&u, &x, true);
            let mut chgs = vec![Changes::Created];
            chgs.extend(c);
            ChangedTask {
                orig: u,
                delta: Changed(chgs),
            }
        }))
        .collect::<Vec<ChangedTask<_>>>();

    let category_changed = changes
        .iter()
        .filter(|x| {
            x.delta != Identical
                && x.delta != Deleted
                && !has_been_recurred(x)
                && !has_been_completed(x)
        })
        .cloned()
        .collect::<Vec<ChangedTask<_>>>();

    category_new.sort_by_key(|x| x.create_date);

    let mut is_first_change = true;
    if !category_new.is_empty() {
        is_first_change = false;
        println!("New tasks");
        println!("---------");
        println!();
        for t in category_new {
            println!(" → {}", color(colorize, Green, &t));
        }
    }

    if !category_deleted.is_empty() {
        if !is_first_change {
            println!();
        }
        is_first_change = false;
        println!("Deleted tasks");
        println!("-------------");
        println!();
        for t in category_deleted {
            println!(" → {}", color(colorize, Red, &t));
        }
    }

    if !category_completed.is_empty() {
        if !is_first_change {
            println!();
        }
        is_first_change = false;
        println!("Completed tasks");
        println!("---------------");
        for x in category_completed {
            println!();

            if has_been_recurred(&x) {
                println!(" → {}", color(colorize, Green, &x.orig));
            } else {
                println!(" → {}", color(colorize, Blue, &x.orig));
            }

            for chgs in x.delta.iter() {
                display_changes(colorize, chgs);
            }
        }
    }

    if !category_changed.is_empty() {
        if !is_first_change {
            println!();
        }
        is_first_change = false;
        println!("Changed tasks");
        println!("-------------");
        for x in category_changed {
            println!();

            if has_been_postponed(&x) {
                println!(" → {}", color(colorize, Yellow, &x.orig));
            } else {
                println!(" → {}", x.orig);
            }

            for chgs in x.delta.iter() {
                display_changes(colorize, chgs);
            }
        }
    }

    // Nice display
    if is_first_change {
        println!("No changes.");
    }
}
