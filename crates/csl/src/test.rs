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

    #[test]
    fn date_variables() {
        parse_all::<DateVariable>("date variable", &["available-date"]);
        let features = Features::new();
        assert!(
            matches!(
                AnyVariable::get_attr("available-date", &features),
                Ok(AnyVariable::Date(_))
            ),
            "`available-date` should resolve to a date variable"
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
                                <date variable="available-date" />
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

mod lenient_parser {
    use super::*;

    fn parse_style(xml: &str) -> crate::Style {
        crate::Style::parse_for_test(xml, None)
            .expect("style should parse despite unknown attributes")
    }

    #[test]
    fn unknown_text_variable_becomes_nop() {
        let style = parse_style(r#"
            <style version="1.0" class="in-text">
                <citation>
                    <layout>
                        <text variable="totally-unknown-future-variable" />
                        <text variable="title" />
                    </layout>
                </citation>
            </style>
        "#);
        let elements = &style.citation.layout.elements;
        assert_eq!(elements.len(), 2, "both elements should be present (one as Nop)");
        assert!(
            matches!(elements[0], Element::Nop),
            "unknown variable should become Element::Nop"
        );
        assert!(
            matches!(elements[1], Element::Text(_)),
            "known variable should remain as Element::Text"
        );
    }

    #[test]
    fn unknown_number_variable_becomes_nop() {
        let style = parse_style(r#"
            <style version="1.0" class="in-text">
                <citation>
                    <layout>
                        <number variable="totally-unknown-number-var" />
                    </layout>
                </citation>
            </style>
        "#);
        let elements = &style.citation.layout.elements;
        assert!(
            matches!(elements[0], Element::Nop),
            "unknown number variable should become Element::Nop"
        );
    }

    #[test]
    fn unknown_type_in_condition_is_filtered() {
        let style = parse_style(r#"
            <style version="1.0" class="in-text">
                <citation>
                    <layout>
                        <choose>
                            <if type="totally-unknown-future-type article-journal">
                                <text variable="title" />
                            </if>
                        </choose>
                    </layout>
                </citation>
            </style>
        "#);
        let elements = &style.citation.layout.elements;
        assert!(
            matches!(elements[0], Element::Choose(_)),
            "choose with mixed known/unknown types should still parse"
        );
    }

    #[test]
    fn unknown_term_becomes_nop() {
        let style = parse_style(r#"
            <style version="1.0" class="in-text">
                <citation>
                    <layout>
                        <text term="patent" />
                        <text term="manuscript" />
                        <text term="ibid" />
                    </layout>
                </citation>
            </style>
        "#);
        let elements = &style.citation.layout.elements;
        assert_eq!(elements.len(), 3);
        assert!(matches!(elements[0], Element::Nop), "unknown term 'patent' → Nop");
        assert!(matches!(elements[1], Element::Nop), "unknown term 'manuscript' → Nop");
        assert!(matches!(elements[2], Element::Text(_)), "known term 'ibid' → Text");
    }

    #[test]
    fn unknown_names_variable_is_filtered() {
        let style = parse_style(r#"
            <style version="1.0" class="in-text">
                <citation>
                    <layout>
                        <names variable="author totally-unknown-name-var" />
                    </layout>
                </citation>
            </style>
        "#);
        let elements = &style.citation.layout.elements;
        if let Element::Names(names) = &elements[0] {
            assert_eq!(names.variables.len(), 1, "unknown name variable should be filtered");
        } else {
            panic!("expected Element::Names");
        }
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
