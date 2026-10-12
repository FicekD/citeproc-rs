// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

//! Runs the CSL test-suite, citeproc-js's own fixtures and the fixtures in `tests/data`.
//!
//! Fixtures listed in `tests/data/expected-failures/*.txt`, and fixtures that need features only
//! citeproc-js has, are reported as ignored. `cargo test --test suite -- --ignored` runs just
//! those, which shows which known failures have started passing.

cfg_if::cfg_if! {
    if #[cfg(all(feature="test-allocator-dlmalloc", feature="dlmalloc"))] {
        #[global_allocator]
        static A: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;
    } else {
        #[global_allocator]
        static A: std::alloc::System = std::alloc::System;
    }
}

mod test_format;
use test_format::{humans, yaml::parse_yaml_test, TestCase};

use libtest_mimic::{Arguments, Failed, Trial};
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use std::fmt;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

/// Wrapper around string slice that makes debug output `{:?}` to print string same way as `{}`.
/// Used with `pretty_assertions` so that test failures show readable multi-line diffs.
#[derive(PartialEq, Eq)]
struct PrettyString<'a>(&'a str);

impl<'a> fmt::Debug for PrettyString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Clone, Copy)]
enum Format {
    /// The CSL test-suite's text format, optionally with the custom-intext feature enabled
    Human {
        custom_intext: bool,
    },
    Yaml,
}

struct Source {
    name: &'static str,
    dir: &'static str,
    format: Format,
    /// Fixtures whose expected output is known to be wrong for citeproc-rs, compared against an
    /// insta snapshot instead
    snapshots: bool,
    /// `None` for sources that must always pass
    expected_failures: Option<&'static str>,
}

const SOURCES: &[Source] = &[
    Source {
        name: "csl_test_suite",
        dir: "tests/data/test-suite/processor-tests/humans",
        format: Format::Human {
            custom_intext: false,
        },
        snapshots: true,
        expected_failures: Some("tests/data/expected-failures/csl-test-suite.txt"),
    },
    Source {
        name: "citeproc_js",
        dir: "tests/data/citeproc-js/fixtures/local",
        format: Format::Human {
            custom_intext: false,
        },
        snapshots: false,
        expected_failures: Some("tests/data/expected-failures/citeproc-js.txt"),
    },
    Source {
        name: "humans",
        dir: "tests/data/humans",
        format: Format::Yaml,
        snapshots: false,
        expected_failures: None,
    },
    Source {
        name: "fixtures_local",
        dir: "tests/data/fixtures-local",
        format: Format::Human {
            custom_intext: true,
        },
        snapshots: true,
        expected_failures: Some("tests/data/expected-failures/fixtures-local.txt"),
    },
];

/// Reads a list of fixture file names, one per line, with `#` comments.
fn read_list(path: &str) -> HashSet<String> {
    let text = read_to_string(path).unwrap_or_else(|e| panic!("could not read {}: {}", path, e));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect()
}

fn fixtures(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            fixtures(&path, ext, out);
        } else if path.extension().map_or(false, |e| e == ext) {
            out.push(path);
        }
    }
}

fn run_fixture(path: &Path, format: Format, snapshot: bool) -> Result<(), Failed> {
    let input = read_to_string(path).map_err(|e| e.to_string())?;
    let mut test_case: TestCase = match format {
        Format::Human { custom_intext } => {
            let custom_intext = custom_intext || input.contains("<intext");
            let features = custom_intext.then(|| csl::Features {
                custom_intext: true,
                ..Default::default()
            });
            humans::parse_human_test(&input, features)?
        }
        Format::Yaml => parse_yaml_test(&input).map_err(|e| e.to_string())?,
    };
    let Some(res) = test_case.execute() else {
        return Ok(());
    };
    if snapshot && res != test_case.result {
        let name = path.file_name().unwrap().to_string_lossy();
        insta::assert_snapshot!(name.as_ref(), res);
    } else {
        assert_eq!(PrettyString(&res), PrettyString(&test_case.result));
    }
    Ok(())
}

fn main() {
    let _ = env_logger::try_init();
    let args = Arguments::from_args();
    let snapshots = read_list("tests/data/snapshot.txt");
    let mut trials = Vec::new();
    for source in SOURCES {
        let expected_failures = source.expected_failures.map(read_list).unwrap_or_default();
        let rel_root = Path::new(source.dir);
        let ext = match source.format {
            Format::Human { .. } => "txt",
            Format::Yaml => "yml",
        };
        let mut paths = Vec::new();
        // Only the top level of the CSL test-suite; its subdirectories hold experiments and
        // tests that contradict the specification.
        if source.name == "csl_test_suite" {
            for entry in std::fs::read_dir(rel_root).into_iter().flatten() {
                let path = entry.unwrap().path();
                if path.extension().map_or(false, |e| e == ext) {
                    paths.push(path);
                }
            }
        } else {
            fixtures(rel_root, ext, &mut paths);
        }
        paths.sort();
        for path in paths {
            let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
            let rel = path
                .strip_prefix(rel_root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let name = format!("{}::{}", source.name, rel);
            let unsupported = match source.format {
                Format::Human { .. } => read_to_string(&path)
                    .ok()
                    .and_then(|s| humans::unsupported(&s)),
                Format::Yaml => None,
            };
            let snapshot = source.snapshots && snapshots.contains(&file_name);
            let format = source.format;
            let mut trial = Trial::test(name, move || run_fixture(&path, format, snapshot));
            if let Some(why) = unsupported {
                trial = trial.with_kind(why).with_ignored_flag(true);
            } else if expected_failures.contains(&file_name) {
                trial = trial.with_kind("known failure").with_ignored_flag(true);
            }
            trials.push(trial);
        }
    }
    libtest_mimic::run(&args, trials).exit();
}
