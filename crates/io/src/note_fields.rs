// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Variables carried in the note field, the convention Zotero uses for its "extra" field and that
//! citeproc-js reads. The note starts with lines like `container-title: Nature` or inline
//! `{:issued: 2004}` entries. Each one fills its variable if the item does not already have it,
//! `type:` replaces the item type, names are written as `Family || Given`, and the lines that
//! were used are removed from the note.

use crate::names::Name;
use crate::{DateOrRange, Reference};
use csl::{AnyVariable, CslType, Features, GetAttribute, Variable};
use std::str::FromStr;

pub(crate) fn apply(refr: &mut Reference) {
    let Some(note) = refr.ordinary.get(&Variable::Note) else {
        return;
    };
    let mut lines: Vec<String> = expand_inline_fields(note)
        .split('\n')
        .map(String::from)
        .collect();
    let mut names: Vec<(csl::NameVariable, Vec<Name>)> = Vec::new();
    for (ix, line) in lines.iter_mut().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((key, value)) = field(line) else {
            // Text before the fields is allowed only on the first line
            if ix == 0 {
                continue;
            }
            break;
        };
        let used = if key == "type" {
            match CslType::get_attr(value, &Features::default()) {
                Ok(typ) => {
                    refr.csl_type = typ;
                    true
                }
                Err(_) => false,
            }
        } else {
            match AnyVariable::get_attr(key, &Features::default()) {
                Ok(AnyVariable::Ordinary(v)) => {
                    if !refr.ordinary.contains_key(&v) {
                        refr.ordinary.insert(v, value.into());
                    }
                    true
                }
                Ok(AnyVariable::Number(v)) => {
                    if !refr.number.contains_key(&v) {
                        refr.number.insert(v, crate::NumberLike::Str(value.into()));
                    }
                    true
                }
                Ok(AnyVariable::Date(v)) => {
                    if !refr.date.contains_key(&v) {
                        let date = crate::edtf::parse(value)
                            .or_else(|| DateOrRange::from_str(value).ok())
                            .unwrap_or_else(|| DateOrRange::Literal {
                                literal: value.into(),
                                circa: false,
                            });
                        refr.date.insert(v, date);
                    }
                    true
                }
                Ok(AnyVariable::Name(v)) => {
                    if !refr.name.contains_key(&v) {
                        if let Some(name) = Name::from_note_field(value) {
                            match names.iter_mut().find(|(var, _)| *var == v) {
                                Some((_, list)) => list.push(name),
                                None => names.push((v, vec![name])),
                            }
                        }
                    }
                    true
                }
                Err(_) => false,
            }
        };
        if used {
            line.clear();
        }
    }
    refr.name.extend(names);
    let note = lines.join("\n").trim().to_string();
    if note.is_empty() {
        refr.ordinary.remove(&Variable::Note);
    } else {
        refr.ordinary.insert(Variable::Note, note);
    }
}

/// `key: value`, where the key is lowercase with hyphens or underscores, or all capitals.
fn field(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let lower = key
        .chars()
        .all(|c| c.is_ascii_lowercase() || c == '-' || c == '_');
    let upper = key.chars().all(|c| c.is_ascii_uppercase());
    let value = value.trim();
    if key.is_empty() || !(lower || upper) || value.is_empty() || value.contains('}') {
        return None;
    }
    Some((key, value))
}

/// Puts each `{:key: value}` on its own line, unless it follows ordinary text.
fn expand_inline_fields(note: &str) -> String {
    let mut out = String::with_capacity(note.len());
    for (ix, line) in note.split('\n').enumerate() {
        if ix > 0 {
            out.push('\n');
        }
        let mut rest = line;
        let mut first = true;
        while let Some(start) = rest.find("{:") {
            let Some(len) = rest[start..].find('}') else {
                break;
            };
            let before = &rest[..start];
            let inner = &rest[start + 2..start + len];
            if field(inner).is_none() || (!before.trim().is_empty() && (ix > 0 || !first)) {
                break;
            }
            out.push_str(before);
            out.push('\n');
            out.push_str(inner.trim());
            out.push('\n');
            rest = &rest[start + len + 1..];
            first = false;
        }
        out.push_str(rest);
    }
    out
}

#[test]
fn note_fields() {
    let mut refr = Reference::empty("id".into(), CslType::Book);
    refr.ordinary.insert(
        Variable::Note,
        "container-title: Container\ncontainer-author: Hall || W. C.\nissued: 2004-21\nKeep this."
            .into(),
    );
    apply(&mut refr);
    assert_eq!(
        refr.ordinary
            .get(&Variable::ContainerTitle)
            .map(|s| s.as_str()),
        Some("Container")
    );
    assert_eq!(
        refr.ordinary.get(&Variable::Note).map(|s| s.as_str()),
        Some("Keep this.")
    );
    assert_eq!(
        refr.date.get(&csl::DateVariable::Issued),
        Some(&DateOrRange::new(2004, 13, 0))
    );
    assert_eq!(
        refr.name
            .get(&csl::NameVariable::ContainerAuthor)
            .map(|n| n.len()),
        Some(1)
    );

    let mut refr = Reference::empty("id".into(), CslType::Book);
    refr.ordinary.insert(Variable::Title, "Original".into());
    refr.ordinary.insert(
        Variable::Note,
        "Some text {:title: Other}\ntype: dataset".into(),
    );
    apply(&mut refr);
    assert_eq!(
        refr.ordinary.get(&Variable::Title).map(|s| s.as_str()),
        Some("Original")
    );
    assert_eq!(refr.csl_type, CslType::Dataset);
}
