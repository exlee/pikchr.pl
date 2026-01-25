// This file is part of pikchr.pl.
//
// pikchr.pl is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// pikchr.pl is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with pikchr.pl. If not, see <https://www.gnu.org/licenses/>.

use std::{iter::Peekable, ops::Range, str::CharIndices};

use iced::{
    Color, Font,
    advanced::text::{Renderer, highlighter::Format},
    highlighter::{self, Theme},
    widget::{text::Highlighter, text_editor::Catalog},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    InTripleQuotes,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    String,
    Heredoc,
    Operator,
    HighlightOperator,
    RiskyOperator,
    Dot,
    Text,
}

pub struct PrologHighlighter {
    current_line: usize
}

impl PrologHighlighter {
    pub fn colorize(token: &Token, theme: &impl Catalog) -> Format<Font> {
        let palette = theme.palette().unwrap();
        //let string_color = Color::from_rgb(0.2, 0.7, 0.2);
        let string_color = Color::from_rgb8(133,200,97);
        let operator_color = Color::from_rgb(0.7, 0.0, 1.0);
        let risky_color = Color::from_rgb8(233,51,58);
        let heredoc_color =Color::from_rgb8(238,253,84);
        match token {
            Token::String => Format {
                color: Some(string_color),
                font: None,
            },
            Token::RiskyOperator => Format {
                color: Some(risky_color),
                font: Some(Font::MONOSPACE),
            },
            Token::HighlightOperator => Format {
                color: Some(palette.primary),
                font: Some(Font::MONOSPACE),
            },
            Token::Dot => Format {
                color: Some(palette.warning),
                font: Some(Font::MONOSPACE),
            },
            Token::Operator => Format {
                color: Some(operator_color),
                font: Some(Font::MONOSPACE),
            },
            Token::Heredoc => Format {
                color: Some(heredoc_color),
                font: Some(Font::MONOSPACE),
            },
            Token::Text => Format::default(),
        }
    }
}

impl Highlighter for PrologHighlighter {
    type Settings = ();

    type Highlight = Token;
    type Iterator<'a> = Box<dyn Iterator<Item = (Range<usize>, Self::Highlight)> + 'a>;

    fn new(_settings: &Self::Settings) -> Self {
        Self {
            current_line: 0
        }
    }

    fn update(&mut self, _new_settings: &Self::Settings) {}
    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let mut highlights = Vec::new();
        let mut chars = line.char_indices().peekable();

        while let Some((i, _)) = chars.peek().cloned() {
            let mut it = line[i..].chars();
            let c1 = it.next().unwrap_or('\0');
            let c2 = it.next().unwrap_or('\0');
            let c3 = it.next().unwrap_or('\0');
            let check_chars = (c1, c2, c3);
            match check_chars {
                ('=', '=', '=') => {
                    handle_operator(i, &mut chars, &mut highlights, 3, Token::Heredoc)
                },
                ('-', '>', _) => {
                    handle_operator(i, &mut chars, &mut highlights, 2, Token::RiskyOperator)
                },
                ('-', '-', '>') => {
                    handle_operator(i, &mut chars, &mut highlights, 3, Token::HighlightOperator)
                },
                (':', '-', _) => {
                    handle_operator(i, &mut chars, &mut highlights, 2, Token::Operator)
                },
                ('"', _, _) => parse_string(i, &mut chars, &mut highlights),
                (c, _, _) if matches!(c, '.' | ',' | ';' | '=' | '!') => {
                    categorize_single_char_token(i, c, &mut chars, &mut highlights)
                },
                _ => {
                    chars.next();
                },
            }
        }

        Box::new(highlights.into_iter())
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

fn parse_string(
    i: usize,
    chars: &mut Peekable<CharIndices>,
    highlights: &mut Vec<(Range<usize>, Token)>,
) {
    let start = i;
    chars.next();
    let mut end = i + 1;
    while let Some((j, next_c)) = chars.next() {
        end = j + 1;
        if next_c == '"' {
            break;
        }
    }
    highlights.push((start..end, Token::String));
}
fn categorize_single_char_token(
    i: usize,
    c: char,
    chars: &mut Peekable<CharIndices>,
    highlights: &mut Vec<(Range<usize>, Token)>,
) {
    let token = match c {
        '.' => Token::Dot,
        ',' | ';' | '=' | '!' => Token::Operator,
        _ => unreachable!(),
    };
    let end = i + 1;
    chars.next();
    highlights.push((i..end, token));
}

fn handle_operator(
    i: usize,
    chars: &mut Peekable<CharIndices>,
    highlights: &mut Vec<(Range<usize>, Token)>,
    operator_length: usize,
    token: Token,
) {
    highlights.push((i..i + operator_length, token));
    for _ in 0..operator_length {
        chars.next();
    }
}
