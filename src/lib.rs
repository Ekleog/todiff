extern crate ansi_term;
extern crate atty;
extern crate chrono;
extern crate clap;
extern crate diff;
extern crate itertools;
extern crate strsim;

extern crate todo_txt;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
#[cfg(feature = "integration_tests")]
#[macro_use]
extern crate serde_derive;

pub mod compute_changes;
pub mod display_changes;
pub mod stable_marriage;

#[cfg(all(test, not(feature = "integration_tests")))]
#[test]
fn remember_integration_tests() {
    panic!("run `cargo test --features=integration_tests` to have all tests run");
}
