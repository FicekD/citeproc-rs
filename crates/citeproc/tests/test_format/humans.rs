// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::test_format::yaml::TestInitOptions;

use super::{Mode, TestCase};

use citeproc::prelude::*;
use citeproc::string_id::Cluster as ClusterStr;
use citeproc_io::{
    cite_compat_vec, output::markup::Markup, Cite, ClusterMode, Reference, SmartString,
};

use core::fmt::Write;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum CompatCitationItem {
    Array(#[serde(with = "cite_compat_vec")] Vec<Cite<Markup>>),
    Map {
        #[serde(with = "cite_compat_vec")]
        cites: Vec<Cite<Markup>>,
        #[serde(
            flatten,
            default,
            deserialize_with = "ClusterMode::compat_opt",
            skip_serializing_if = "Option::is_none"
        )]
        mode: Option<ClusterMode>,
    },
}

impl CompatCitationItem {
    pub fn to_note_cluster(self, index: u32) -> ClusterStr<Markup> {
        let (v, mode) = match self {
            CompatCitationItem::Array(v) => (v, None),
            CompatCitationItem::Map { cites, mode } => (cites, mode),
        };
        ClusterStr {
            id: index.to_string().into(),
            cites: v,
            mode,
        }
    }
}

#[derive(Debug, PartialEq)]
enum ResultKind {
    Dots,
    Arrows,
}
#[derive(Debug, PartialEq)]
pub struct CiteResult {
    kind: ResultKind,
    // id: u32,
    note: ClusterNumber,
    text: String,
}
#[derive(Debug, PartialEq)]
pub struct Results(pub Vec<CiteResult>);

impl Results {
    pub fn output_independent(&self, options: &TestInitOptions) -> String {
        let mut output = String::new();
        for (n, res) in self.0.iter().enumerate() {
            // Whether or not something is recomputed is not part of the CSL spec. We will simply
            // ignore this.
            // output.push_str(if res.kind == ResultKind::Arrows {
            //     ">>"
            // } else {
            //     ".."
            // });
            output.push_str("[");
            write!(&mut output, "{}", n).unwrap();
            output.push_str("] ");
            output.push_str(&super::normalise_html(&res.text, options));
            output.push_str("\n");
        }
        output
    }
}

impl FromStr for Results {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use nom::{
            branch::alt,
            bytes::complete::{tag, take_until},
            character::complete::{char, digit1},
            combinator::map,
            multi::separated_list1,
            sequence::{delimited, preceded, tuple},
            IResult,
        };
        fn dots(inp: &str) -> IResult<&str, ResultKind> {
            map(alt((tag(".."), tag(">>"))), |s| match s {
                ".." => ResultKind::Dots,
                ">>" => ResultKind::Arrows,
                _ => unreachable!(),
            })(inp)
        }
        fn num(inp: &str) -> IResult<&str, u32> {
            map(delimited(char('['), digit1, char(']')), |ds: &str| {
                u32::from_str(ds).unwrap()
            })(inp)
        }
        fn formatted(inp: &str) -> IResult<&str, &str> {
            preceded(char(' '), take_until("\n"))(inp)
        }
        fn total(inp: &str) -> IResult<&str, CiteResult> {
            map(tuple((dots, num, formatted)), |(k, n, f)| CiteResult {
                kind: k,
                // id: n,
                // incorrect, but we don't actually know except by looking at the instructions what
                // the right note number is
                note: ClusterNumber::Note(IntraNote::Single(n)),
                text: f.to_string(),
            })(inp)
        }
        fn whole_thing(inp: &str) -> IResult<&str, Vec<CiteResult>> {
            separated_list1(char('\n'), total)(inp)
        }
        Ok(Results(whole_thing(s).unwrap().1))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct Properties {
    #[serde(rename = "noteIndex", alias = "note", default)]
    note_index: u32,
    #[serde(
        flatten,
        default,
        deserialize_with = "ClusterMode::compat_opt",
        skip_serializing_if = "Option::is_none"
    )]
    mode: Option<ClusterMode>,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct ClusterInstruction {
    #[serde(rename = "citationID", alias = "id")]
    cluster_id: SmartString,
    #[serde(rename = "citationItems", alias = "cites", with = "cite_compat_vec")]
    citation_items: Vec<Cite<Markup>>,
    #[serde(default)]
    properties: Properties,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct PrePost(SmartString, u32);

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct CiteprocJsInstruction {
    cluster: ClusterInstruction,
    pre: Vec<PrePost>,
    post: Vec<PrePost>,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum Instruction2 {
    Map(CiteprocJsInstruction),
    Array(ClusterInstruction, Vec<PrePost>, Vec<PrePost>),
}

impl From<Instruction2> for CiteprocJsInstruction {
    fn from(other: Instruction2) -> Self {
        match other {
            Instruction2::Map(i) => i,
            Instruction2::Array(cluster, pre, post) => CiteprocJsInstruction { cluster, pre, post },
        }
    }
}

pub struct JsExecutor<'a> {
    current_note_numbers: HashMap<ClusterId, ClusterNumber>,
    proc: &'a mut Processor,
}

impl JsExecutor<'_> {
    pub fn new<'a>(proc: &'a mut Processor) -> JsExecutor<'a> {
        JsExecutor {
            current_note_numbers: HashMap::new(),
            proc,
        }
    }
    fn get_id(&mut self, string_id: &str) -> ClusterId {
        self.proc.cluster_id(string_id)
    }

    pub fn get_results(&self) -> Results {
        let updates = self.proc.batched_updates();
        let mut mod_clusters = HashMap::new();
        let mut results = Vec::<CiteResult>::new();
        for (id, text) in updates.clusters {
            mod_clusters.insert(id, true);
            let &note = self.current_note_numbers.get(&id).unwrap();
            let text = (*text).clone();
            results.push(CiteResult {
                kind: ResultKind::Arrows,
                // id,
                note,
                text: text.to_string(),
            })
        }
        // for &id in self.current_note_numbers.keys() {
        //     if mod_clusters.contains_key(&id) {
        //         continue;
        //     }
        //     let &note = self.current_note_numbers.get(&id).unwrap();
        //     if let Some(text) = self.proc.get_cluster(id) {
        //         results.push(CiteResult {
        //             kind: ResultKind::Dots,
        //             id,
        //             note,
        //             text: super::normalise_html(&text),
        //         })
        //     }
        // }
        results.sort_by_key(|x| x.note);
        Results(results)
    }

    fn to_renumbering(&mut self, renum: &mut Vec<ClusterPosition>, prepost: &[PrePost]) {
        for &PrePost(ref string_id, note_number) in prepost.iter() {
            let id = self.get_id(string_id);
            let note = if note_number == 0 {
                None
            } else {
                Some(note_number)
            };
            renum.push(ClusterPosition { id: Some(id), note })
        }
    }

    pub fn execute(&mut self, instructions: &[CiteprocJsInstruction]) {
        self.proc.drain();
        let mut renum = Vec::new();
        for CiteprocJsInstruction { cluster, pre, post } in instructions {
            let ClusterInstruction {
                cluster_id,
                citation_items,
                properties,
            } = cluster;
            let Properties { mode, note_index } = properties;

            renum.clear();
            self.to_renumbering(&mut renum, pre);
            self.to_renumbering(
                &mut renum,
                &[PrePost(cluster.cluster_id.clone(), *note_index)],
            );
            self.to_renumbering(&mut renum, post);
            self.proc.insert_cluster_str(ClusterStr {
                id: cluster_id.clone(),
                mode: mode.clone(),
                cites: citation_items.to_vec(),
            });
            self.proc.set_cluster_order(&renum).unwrap();
            for &ClusterPosition { id, .. } in &renum {
                if let Some(id) = id {
                    if let Some(actual_note) = self.proc.get_cluster_note_number(id) {
                        self.current_note_numbers.insert(id, actual_note);
                    }
                }
            }
        }
    }
}

/// Splits a fixture into its `>>===== NAME =====>>` sections. Text between sections is a comment.
fn sections(contents: &str) -> HashMap<String, String> {
    use regex::Regex;
    lazy_static! {
        static ref BEGIN: Regex = Regex::new(r">>=+ ?([A-Z\-]+) ?=+>>").unwrap();
        static ref END: Regex = Regex::new(r"<<=+ ?([A-Z\-]+) ?=+<<").unwrap();
    }
    let mut sections = HashMap::new();
    let mut current: Option<(String, String)> = None;
    for line in contents.lines() {
        if END.is_match(line) {
            if let Some((name, body)) = current.take() {
                sections.entry(name).or_insert(body);
            }
        } else if let Some(caps) = BEGIN.captures(line) {
            current = Some((caps[1].to_string(), String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(line);
        }
    }
    sections
}

/// Sections that only citeproc-js understands. Fixtures using them test processor-specific
/// behaviour that citeproc-rs does not implement.
const UNSUPPORTED_SECTIONS: &[(&str, &str)] = &[
    ("ABBREVIATIONS", "abbreviation lists"),
    ("LANGPARAMS", "multilingual parameters"),
    ("MULTIAFFIX", "multilingual affixes"),
    ("OPTIONS", "citeproc-js engine options"),
    ("OPTIONZ", "citeproc-js engine options"),
    ("KEYS", "citeproc-js input key mapping"),
    ("BIBENTRIES", "bibliography entry selection"),
    ("BIBSECTION", "bibliography sections"),
];

const SUPPORTED_MODES: &[&str] = &[
    "citation",
    "bibliography",
    "bibliography-nosort",
    "citation-rtf",
    "bibliography-rtf",
];

/// Says why a fixture cannot run against citeproc-rs, if it cannot.
pub fn unsupported(contents: &str) -> Option<String> {
    let sections = sections(contents);
    if let Some((_, why)) = UNSUPPORTED_SECTIONS
        .iter()
        .find(|(name, _)| sections.contains_key(*name))
    {
        return Some(why.to_string());
    }
    if sections
        .get("CSL")
        .map_or(false, |csl| csl.contains("mlz1\""))
    {
        return Some("CSL-M style".into());
    }
    match sections.get("MODE").map(|m| m.trim()) {
        Some(mode) if !SUPPORTED_MODES.contains(&mode) => Some(format!("{} mode", mode)),
        _ => None,
    }
}

/// Parses a fixture in the CSL test-suite format.
pub fn parse_human_test(
    contents: &str,
    csl_features: Option<csl::Features>,
) -> Result<TestCase, String> {
    if let Some(why) = unsupported(contents) {
        return Err(format!("unsupported: {}", why));
    }
    let sections = sections(contents);
    let section = |name: &str| {
        sections
            .get(name)
            .ok_or_else(|| format!("fixture has no {} section", name))
    };

    let (mode, format, bibliography_no_sort) = match section("MODE")?.trim() {
        "citation" => (Mode::Citation, SupportedFormat::Html, false),
        "bibliography" => (Mode::Bibliography, SupportedFormat::Html, false),
        "bibliography-nosort" => (Mode::Bibliography, SupportedFormat::Html, true),
        "citation-rtf" => (Mode::Citation, SupportedFormat::Rtf, false),
        "bibliography-rtf" => (Mode::Bibliography, SupportedFormat::Rtf, false),
        other => unreachable!("unsupported mode {}", other),
    };
    let mut input: Vec<serde_json::Value> = serde_json::from_str(section("INPUT")?)
        .map_err(|e| format!("could not parse INPUT: {}", e))?;
    // A few fixtures have items without an id, which citeproc-js files under "undefined"
    for (n, item) in input.iter_mut().enumerate() {
        if let Some(obj) = item.as_object_mut() {
            obj.entry("id")
                .or_insert_with(|| format!("undefined-{}", n).into());
        }
    }
    let input: Vec<Reference> = serde_json::from_value(input.into())
        .map_err(|e| format!("could not parse INPUT: {}", e))?;
    let citation_items: Option<Vec<CompatCitationItem>> = sections
        .get("CITATION-ITEMS")
        .map(|s| serde_json::from_str(s))
        .transpose()
        .map_err(|e| format!("could not parse CITATION-ITEMS: {}", e))?;
    let citations: Option<Vec<Instruction2>> = sections
        .get("CITATIONS")
        .map(|s| serde_json::from_str(s))
        .transpose()
        .map_err(|e| format!("could not parse CITATIONS: {}", e))?;

    let options = TestInitOptions {
        format,
        format_options: FormatOptions {
            link_anchors: false,
        },
        csl_features,
        bibliography_no_sort,
        locale_override: None,
        normalise: true,
    };
    let result = super::normalise_html(section("RESULT")?, &options);

    TestCase::new(
        mode,
        options,
        section("CSL")?.clone(),
        input,
        result,
        citation_items.map(|items| {
            items
                .into_iter()
                .enumerate()
                .map(|(n, item)| item.to_note_cluster(n as u32 + 1))
                .collect()
        }),
        citations.map(|inst2s| {
            inst2s
                .into_iter()
                .map(CiteprocJsInstruction::from)
                .collect()
        }),
    )
}
