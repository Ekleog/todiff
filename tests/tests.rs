#[macro_use]
extern crate pretty_assertions;
extern crate itertools;
extern crate serde;
extern crate serde_yaml;
extern crate todiff;
extern crate todo_txt;
#[macro_use]
extern crate serde_derive;

// Important: for these tests to run, run `cargo test --features=integration_tests`
use itertools::Itertools;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::str::FromStr;
use todiff::compute_changes::*;
use todiff::display_changes::*;
use todiff::merge_changes::*;
use todo_txt::task::Extended as Task;

fn tasks_from_strings(strings: Vec<String>) -> Vec<Task> {
    strings
        .into_iter()
        .map(|s| Task::from_str(&s).unwrap())
        .collect()
}

fn tasks_to_strings(tasks: &Vec<Task>) -> Vec<String> {
    tasks.iter().map(Task::to_string).collect()
}

fn deserialize_tasks<'de, D>(deserializer: D) -> Result<Vec<Task>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    use serde::Deserialize;
    <Vec<String>>::deserialize(deserializer).map(tasks_from_strings)
}

use serde::de::DeserializeOwned;
trait Test: DeserializeOwned {
    fn run(self);
}

#[derive(Deserialize, Debug)]
struct ChangesetTest {
    allowed_divergence: Option<usize>,
    #[serde(deserialize_with = "deserialize_tasks")]
    from: Vec<Task>,
    #[serde(deserialize_with = "deserialize_tasks")]
    to: Vec<Task>,
    new: Vec<String>,
    changes: Vec<TaskDelta<Vec<String>>>,
}

impl Test for ChangesetTest {
    fn run(self: ChangesetTest) {
        // Test that compute_changeset returns what is expected
        let allowed_divergence = self.allowed_divergence.unwrap_or(0);
        let (computed_new, computed_changes) =
            compute_changeset(self.from.clone(), self.to.clone(), allowed_divergence);

        let computed_new_as_str = tasks_to_strings(&computed_new);
        let computed_changes_as_strs = computed_changes
            .iter()
            .cloned()
            .map(|tc| {
                tc.delta
                    .map(|chgs| chgs.into_iter().map(|c| format!("{:?}", c)).collect())
            })
            .collect::<Vec<TaskDelta<Vec<String>>>>();

        assert_eq!(
            (self.new, self.changes),
            (computed_new_as_str, computed_changes_as_strs),
            "Mismatching new tasks/changes"
        );
    }
}

#[derive(Deserialize, Debug)]
struct DisplayTest {
    allowed_divergence: Option<usize>,
    #[serde(deserialize_with = "deserialize_tasks")]
    from: Vec<Task>,
    #[serde(deserialize_with = "deserialize_tasks")]
    to: Vec<Task>,
    changes: String,
}

impl Test for DisplayTest {
    fn run(self: DisplayTest) {
        // Test that the output of the command is as expected
        let allowed_divergence = self.allowed_divergence.unwrap_or(0);
        let (new_tasks, changes) =
            compute_changeset(self.from.clone(), self.to.clone(), allowed_divergence);
        let output = display_changeset(new_tasks, changes, false);

        // Split into lines to make diff easier to read
        assert_eq!(
            self.changes.lines().collect_vec(),
            output.lines().collect_vec()
        );
    }
}

#[derive(Deserialize, Debug)]
struct MergeTest {
    allowed_divergence: Option<usize>,
    #[serde(deserialize_with = "deserialize_tasks")]
    from: Vec<Task>,
    #[serde(deserialize_with = "deserialize_tasks")]
    left: Vec<Task>,
    #[serde(deserialize_with = "deserialize_tasks")]
    right: Vec<Task>,
    result: String,
}

impl Test for MergeTest {
    fn run(self: MergeTest) {
        // Test 3-way merges
        let allowed_divergence = self.allowed_divergence.unwrap_or(0);
        let computed_changes = merge_3way(
            self.from.clone(),
            self.left.clone(),
            self.right.clone(),
            allowed_divergence,
        );
        assert_eq!(
            self.result.trim(),
            merge_to_string(computed_changes.clone()),
            "Mismatching merge result"
        );

        if let Some(merge_result) = extract_merge_result(computed_changes) {
            let diff_from_left =
                compute_changeset(self.from.clone(), self.left.clone(), allowed_divergence);
            let diff_right_result =
                compute_changeset(self.right.clone(), merge_result.clone(), allowed_divergence);
            assert_eq!(
                display_changeset(diff_from_left.0, diff_from_left.1, false),
                display_changeset(diff_right_result.0, diff_right_result.1, false),
                "Mismatching diffs after merge"
            );

            let diff_from_right =
                compute_changeset(self.from.clone(), self.right.clone(), allowed_divergence);
            let diff_left_result =
                compute_changeset(self.left.clone(), merge_result.clone(), allowed_divergence);
            assert_eq!(
                display_changeset(diff_from_right.0, diff_from_right.1, false),
                display_changeset(diff_left_result.0, diff_left_result.1, false),
                "Mismatching diffs after merge"
            );
        }
    }
}

fn run_tests_from_yaml<T: Test>(suite: &str, path: &str) {
    let file = File::open(path).expect(&format!("Unable to open file ‘{}’", path));
    let test_map: BTreeMap<String, T> =
        serde_yaml::from_reader(BufReader::new(&file)).unwrap_or_else(|e| panic!("{}", e));
    for (name, test) in test_map {
        println!("Running test '{}/{}'", suite, name);
        test.run();
    }
}

#[test]
fn test_yamls() {
    run_tests_from_yaml::<ChangesetTest>("changeset", "tests/changeset_tests.yaml");
    run_tests_from_yaml::<DisplayTest>("display", "tests/display_tests.yaml");
    run_tests_from_yaml::<MergeTest>("merge", "tests/merge_tests.yaml");
}
