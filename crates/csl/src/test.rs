// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2020 Corporation for Digital Scholarship

use super::*;

#[test]
fn features() {
    assert_snapshot_parse!(Features, r#"<features></features>"#);
    assert_snapshot_parse!(
        Features,
        r#"
        <features>
            <feature name="condition-date-parts" />
            <feature name="edtf-dates" />
        </features>
    "#
    );
    assert_snapshot_err!(
        Features,
        r#"
        <features>
            <feature name="edtf-dates" />
            <feature name="UNRECOGNIZED-FEATURE" />
            <feature name="SECOND-UNRECOGNIZED-FEATURE" />
        </features>
    "#
    );
}

#[test]
fn intext() {
    let features = Features {
        custom_intext: true,
        ..Default::default()
    };
    let options = ParseOptions {
        allow_no_info: true,
        features: Some(features),
        ..Default::default()
    };
    assert_snapshot_parse!(
        InText,
        r#"<intext><layout><text variable="title"/></layout></intext>"#
    );
    assert_snapshot_parse!(
        Style,
        r#"<style class="in-text">
            <citation><layout></layout></citation>
            <intext><layout><text variable="title" /></layout></intext>
        </style>"#,
        options.clone()
    );
    assert_snapshot_err!(
        Style,
        r#"<style class="in-text">
             <citation><layout></layout></citation>
             <intext><layout></layout></intext>
         </style>"#
    );
}

#[test]
fn unsupported_version() {
    assert_snapshot_err!(
        Style,
        r#"
        <style version="999.0" class="in-text">
            <citation><layout></layout></citation>
        </style>
    "#
    );
}

#[test]
fn unrecognised_macros() {
    assert_snapshot_err!(
        Style,
        r#"
        <style version="1.0" class="in-text">
            <citation>
                <layout>
                    <text macro="unknown" />
                </layout>
            </citation>
        </style>
    "#
    );
    assert_snapshot_err!(
        Style,
        r#"
        <style version="1.0" class="in-text">
            <citation>
                <sort>
                    <key macro="unknown" />
                </sort>
                <layout></layout>
            </citation>
        </style>
    "#
    );
    assert_snapshot_err!(
        Style,
        r#"
        <style version="1.0" class="in-text">
            <citation><layout></layout></citation>
            <bibliography>
                <sort>
                    <key macro="unknown" />
                </sort>
                <layout></layout>
            </bibliography>
        </style>
    "#
    );
    assert_snapshot_parse!(
        Style,
        r#"
        <style version="1.0" class="in-text">
            <macro name="known" />
            <citation>
                <layout>
                    <text macro="known" />
                </layout>
            </citation>
        </style>
    "#
    );
}

#[test]
fn missing_info() {
    // Externally, missing info should fail.
    insta::assert_debug_snapshot!(crate::Style::parse(::indoc::indoc!(
        r#"
            <style version="1.0.1" class="in-text">
                <citation><layout></layout></citation>
            </style>
        "#
    ))
    .expect_err("should have failed with errors"));
    // But internally we can ignore it.
    assert_snapshot_parse!(
        Style,
        r#"
        <style version="1.0.1" class="in-text">
            <citation><layout></layout></citation>
        </style>
    "#
    );
}

/// CSL 1.0.2 added item types, number variables, locator types and terms. These must parse under
/// plain CSL with no feature flags enabled, otherwise current marquee styles (APA, IEEE, MLA, ...)
/// fail to parse on a single missing variant.
mod csl_1_0_2 {
    use super::*;

    fn parse_all<T: GetAttribute + std::fmt::Debug>(label: &str, values: &[&str]) {
        let features = Features::new();
        for &s in values {
            T::get_attr(s, &features)
                .unwrap_or_else(|_| panic!("CSL 1.0.2 {} `{}` should parse", label, s));
        }
    }

    #[test]
    fn types() {
        parse_all::<CslType>(
            "type",
            &[
                "software",
                "standard",
                "collection",
                "document",
                "event",
                "performance",
                "periodical",
                "classic",
                "hearing",
                "regulation",
            ],
        );
    }

    #[test]
    fn number_variables() {
        // The pure CSL 1.0.2 number variables resolve to Number through AnyVariable.
        let features = Features::new();
        parse_all::<NumberVariable>(
            "number variable",
            &["part-number", "printing-number", "supplement-number"],
        );
        for s in &["part-number", "printing-number", "supplement-number"] {
            assert!(
                matches!(
                    AnyVariable::get_attr(s, &features),
                    Ok(AnyVariable::Number(_))
                ),
                "`{}` should resolve to a number variable",
                s
            );
        }

        // `section`/`version` are dual-listed: usable in `<number>` (so they must parse as
        // NumberVariable) while CSL-JSON data still treats them as ordinary strings (so AnyVariable
        // resolves them to Ordinary, Variable being tried first). This is the same pattern as
        // `authority`, and is what lets the `<number variable="section">` styles parse.
        parse_all::<NumberVariable>("dual-listed number variable", &["section", "version"]);
        for s in &["section", "version"] {
            assert!(
                matches!(
                    AnyVariable::get_attr(s, &features),
                    Ok(AnyVariable::Ordinary(_))
                ),
                "`{}` should resolve to an ordinary variable for input data",
                s
            );
        }
    }

    #[test]
    fn locator_types() {
        parse_all::<LocatorType>(
            "locator type",
            &[
                "act",
                "appendix",
                "article-locator",
                "canon",
                "elocation",
                "equation",
                "scene",
                "table",
                "timestamp",
                "title-locator",
                "version",
                "rule",
                "supplement",
            ],
        );
    }

    #[test]
    fn terms() {
        parse_all::<MiscTerm>(
            "term",
            &[
                "preprint",
                "advance-online-publication",
                "special-issue",
                "special-section",
                "loc-cit",
                "op-cit",
                "original-work-published",
                "personal-communication",
                "working-paper",
            ],
        );
    }

    /// End-to-end: a style branching on a CSL 1.0.2 type and rendering 1.0.2 variables/terms must
    /// parse as a whole, with no features enabled.
    #[test]
    fn style_round_trip() {
        let xml = r#"
            <style version="1.0" class="in-text">
                <citation>
                    <layout>
                        <choose>
                            <if type="software">
                                <text variable="part-number" />
                                <number variable="supplement-number" />
                                <number variable="section" />
                                <text variable="version" />
                                <label variable="locator" />
                                <text term="preprint" />
                            </if>
                        </choose>
                    </layout>
                </citation>
            </style>
        "#;
        crate::Style::parse_for_test(xml, None)
            .expect("CSL 1.0.2 style should parse successfully");
    }
}

#[test]
fn wrong_tag_name() {
    assert_snapshot_err!(
        Style,
        r#"
        <stylo version="1.0.1" class="in-text">
            <citation><layout></layout></citation>
        </stylo>
    "#
    );
    assert_snapshot_err!(
        Locale,
        r#"
        <localzzz xml:lang="en-US" version="1.0.1" class="in-text">
        </localzzz>
    "#
    );
}
