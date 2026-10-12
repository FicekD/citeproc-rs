// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::{FormatOptions, InlineElement, MarkupWriter, MaybeTrimStart};
use crate::output::markup::Link;
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;
use crate::String;
use csl::{DisplayMode, Formatting};

#[derive(Debug)]
pub struct PlainWriter<'a> {
    dest: &'a mut String,
    #[allow(unused)]
    options: FormatOptions,
}

impl<'a> PlainWriter<'a> {
    pub fn new(dest: &'a mut String, options: FormatOptions) -> Self {
        PlainWriter { dest, options }
    }
}

impl<'a> MarkupWriter for PlainWriter<'a> {
    fn buf(&mut self) -> &mut String {
        self.dest
    }

    fn write_escaped(&mut self, text: &str) {
        self.dest.push_str(text);
    }

    fn write_url(&mut self, url: &url::Url, trailing_slash: bool, in_attr: bool) {
        super::write_url(
            self.dest,
            url,
            trailing_slash,
            in_attr,
            |b, s| Ok(b.push_str(s)),
            |b, s| Ok(b.push_str(s)),
        )
        .unwrap()
    }

    fn stack_preorder(&mut self, _stack: &[FormatCmd]) {}

    fn stack_postorder(&mut self, _stack: &[FormatCmd]) {}

    fn write_micro(&mut self, micro: &MicroNode, trim_start: bool) {
        use MicroNode::*;
        match micro {
            Text(text) => {
                self.write_escaped(text.trim_start_if(trim_start));
            }
            Quoted {
                is_inner,
                localized,
                children,
            } => {
                self.dest
                    .push_str(localized.opening(*is_inner).trim_start_if(trim_start));
                self.write_micros(children, false);
                self.dest.push_str(localized.closing(*is_inner));
            }
            Formatted(nodes, _cmd) => {
                self.write_micros(nodes, trim_start);
            }
            NoCase(inners) => {
                self.write_micros(inners, trim_start);
            }
            NoDecor(inners) => {
                self.write_micros(inners, trim_start);
            }
        }
    }

    fn write_inline(&mut self, inline: &InlineElement, trim_start: bool) {
        use super::InlineElement::*;
        match inline {
            Text(text) => {
                self.write_escaped(text.trim_start_if(trim_start));
            }
            // Matches citeproc-js's text format: the left margin is separated from the rest of
            // the entry by a space, and blocks start on a new line.
            Div(display, inlines) => {
                match display {
                    DisplayMode::Block if !self.dest.is_empty() => self.dest.push('\n'),
                    DisplayMode::Indent if !self.dest.is_empty() => self.dest.push_str("\n    "),
                    _ => {}
                }
                self.stack_formats(inlines, Formatting::default(), Some(*display));
                if *display == DisplayMode::LeftMargin {
                    let trimmed = self.dest.trim_end_matches(' ').len();
                    self.dest.truncate(trimmed);
                    self.dest.push(' ');
                }
            }
            Micro(micros) => {
                self.write_micros(micros, trim_start);
            }
            Formatted(inlines, formatting) => {
                self.stack_formats(inlines, *formatting, None);
            }
            Quoted {
                is_inner,
                localized,
                inlines,
            } => {
                // TODO: move punctuation
                self.write_escaped(localized.opening(*is_inner).trim_start_if(trim_start));
                self.write_inlines(inlines, false);
                self.write_escaped(localized.closing(*is_inner));
            }
            Linked(link) => {
                self.write_link("", link, "", "", self.options);
            }
        }
    }
    fn write_link(&mut self, _: &str, link: &Link, _: &str, _: &str, _: FormatOptions) {
        match link {
            Link::Url {
                url,
                trailing_slash,
            } => {
                self.write_url(url, *trailing_slash, false);
            }
            Link::Id { id, url: _ } => self.write_escaped(id),
        }
    }
}
