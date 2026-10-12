// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The parts of the Extended Date/Time Format (EDTF, ISO 8601-2) that CSL dates can hold:
//! dates with year, month or day precision, intervals, seasons (21 to 24), uncertain or
//! approximate dates (rendered as circa), and unspecified months or days (`1985-04-XX`).
//! Anything else that is still valid EDTF, like a decade (`199X`) or an open interval, becomes a
//! literal date, so it is printed as written rather than misread.

use crate::{Date, DateOrRange};

/// `None` if the string is not EDTF at all.
pub(crate) fn parse(s: &str) -> Option<DateOrRange> {
    let s = s.trim();
    let looks_like_edtf = !s.is_empty()
        && s.chars().any(|c| c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_digit() || "X?~%+-/.".contains(c));
    if !looks_like_edtf {
        return None;
    }
    let parsed = match s.split_once('/') {
        Some((from, to)) => date(from)
            .zip(date(to))
            .map(|(from, to)| DateOrRange::Range(from, to)),
        None => date(s).map(DateOrRange::Single),
    };
    Some(parsed.unwrap_or_else(|| DateOrRange::Literal {
        literal: s.into(),
        circa: false,
    }))
}

fn date(s: &str) -> Option<Date> {
    let circa = s.ends_with(|c| matches!(c, '?' | '~' | '%'));
    let s = s.trim_end_matches(|c| matches!(c, '?' | '~' | '%'));
    let (negative, s) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let mut parts = s.split('-');
    let year = parts.next().filter(|y| y.len() == 4)?;
    let year: i32 = year.parse().ok()?;
    let month = unspecified_or(parts.next(), 2)?;
    let day = unspecified_or(parts.next(), 2)?;
    if parts.next().is_some() {
        return None;
    }
    let (month, day) = match (month, day) {
        (0, d) if d > 0 => return None,
        // Seasons 21 to 24 are spring to winter, which CSL dates store as months 13 to 16
        (m @ 21..=24, 0) => (m - 8, 0),
        (m @ 0..=12, d @ 0..=31) => (m, d),
        _ => return None,
    };
    Some(Date {
        year: if negative { -year } else { year },
        month,
        day,
        circa,
    })
}

/// A missing part or one made of `X`s is 0, meaning "not present".
fn unspecified_or(part: Option<&str>, len: usize) -> Option<u32> {
    match part {
        None => Some(0),
        Some(p) if p.len() != len => None,
        Some(p) if p.chars().all(|c| c == 'X') => Some(0),
        Some(p) => p.parse().ok(),
    }
}

#[test]
fn edtf() {
    let single = |y, m, d| Some(DateOrRange::Single(Date::new(y, m, d)));
    let literal = |s: &str| {
        Some(DateOrRange::Literal {
            literal: s.into(),
            circa: false,
        })
    };
    assert_eq!(parse("1985-04-12"), single(1985, 4, 12));
    assert_eq!(parse("1985-04"), single(1985, 4, 0));
    assert_eq!(parse("1985"), single(1985, 0, 0));
    assert_eq!(parse("-0044-03-15"), single(-44, 3, 15));
    assert_eq!(parse("1985-04-XX"), single(1985, 4, 0));
    assert_eq!(parse("1985-XX-XX"), single(1985, 0, 0));
    assert_eq!(parse("2004-21"), single(2004, 13, 0));
    let mut circa = Date::new(1985, 0, 0);
    circa.circa = true;
    assert_eq!(parse("1985~"), Some(DateOrRange::Single(circa)));
    assert_eq!(
        parse("1985-04-12/1985-06"),
        Some(DateOrRange::Range(
            Date::new(1985, 4, 12),
            Date::new(1985, 6, 0)
        ))
    );
    assert_eq!(parse("199X"), literal("199X"));
    assert_eq!(parse("1985/.."), literal("1985/.."));
    assert_eq!(parse("1985-13"), literal("1985-13"));
    assert_eq!(parse("Spring 1985"), None);
}
