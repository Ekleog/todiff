#[macro_use]
extern crate pretty_assertions;
extern crate serde;
extern crate serde_yaml;
extern crate todiff;
extern crate todo_txt;
#[macro_use]
extern crate serde_derive;

// Important: for these tests to run, run `cargo test --features=integration_tests`
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::str::FromStr;
use todiff::compute_changes::*;
use todo_txt::Task;

fn tasks_from_strings(strings: Vec<String>) -> Vec<Task> {
    strings
        .into_iter()
        .map(|s| Task::from_str(&s).unwrap())
        .collect()
}

fn deserialize_tasks<'de, D>(deserializer: D) -> Result<Vec<Task>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    use serde::Deserialize;
    <Vec<String>>::deserialize(deserializer).map(tasks_from_strings)
}

#[derive(Deserialize, Debug)]
struct Test {
    allowed_divergence: Option<usize>,
    #[serde(deserialize_with = "deserialize_tasks")]
    from: Vec<Task>,
    #[serde(deserialize_with = "deserialize_tasks")]
    to: Vec<Task>,
    new: Vec<String>,
    changes: Vec<TaskDelta<Vec<String>>>,
}

fn read_yaml(path: &str) -> BTreeMap<String, Test> {
    let file = File::open(path).expect(&format!("Unable to open file ‘{}’", path));
    serde_yaml::from_reader(BufReader::new(&file)).unwrap_or_else(|e| panic!("{}", e))
}

fn run_test(test: Test) {
    let allowed_divergence = test.allowed_divergence.unwrap_or(0);
    let (computed_new, computed_changes) =
        compute_changeset(test.from, test.to, allowed_divergence);

    let computed_new_as_str = computed_new
        .iter()
        .map(Task::to_string)
        .collect::<Vec<String>>();
    let computed_changes_as_strs = computed_changes
        .into_iter()
        .map(|tc| {
            tc.delta
                .map(|chgs| chgs.into_iter().map(|c| format!("{:?}", c)).collect())
        })
        .collect::<Vec<TaskDelta<Vec<String>>>>();

    assert_eq!(test.new, computed_new_as_str);
    assert_eq!(test.changes, computed_changes_as_strs);
}

#[test]
fn test_yamls() {
    for (name, test) in read_yaml("tests/tests.yaml") {
        println!("Running test '{}'", name);
        run_test(test);
    }
}
