// Copyright (C) 2014  Daniel Trebbien
//
// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 3 of the License, or (at your option) any later version.
//
// This library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Lesser General Public License for more details.

#![crate_type = "lib"]
#![feature(struct_variant)]

extern crate libc;

use std::fmt::{FormatError, Formatter, Show};
use std::from_str::{from_str};
use std::io::{Buffer};
use std::option::{Option};
use std::result::{Result, fold_};
use std::slice::{Items};
use std::string::{String};
use std::vec::{Vec};

/// Holds information about a parse error generated while parsing a suppressions file.
pub struct ParseError {
    /// Line number where the parse error occurred.
    pub lineno: uint,

    /// Description of the parse error.
    pub message: String,
}

#[deriving(Clone)]
pub enum Frame {
    /// A frame-level wildcard, represented by `'...'`.
    FrameWildcard,

    /// An object frame.
    ObjFrame {
        /// A file glob for the path to the object file. This may contain wildcard characters
        /// `*` and `?`.
        pub glob: String,
    },

    /// A function frame.
    FunFrame {
        /// Glob for the name of the function. This may contain wildcard characters `*` and `?`.
        pub glob: String,
    },
}

impl Show for Frame {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), FormatError> {
        match self {
            &FrameWildcard => write!(fmt, "..."),
            &ObjFrame {
                glob: ref glob
            } => {
                write!(fmt, "obj:{}", glob.as_slice())
            },
            &FunFrame {
                glob: ref glob
            } => {
                write!(fmt, "fun:{}", glob.as_slice())
            },
        }
    }
}

#[deriving(Clone, PartialEq)]
pub enum SuppressionType {
    MemcheckAddr(uint),
    MemcheckCond,
    MemcheckFree,
    MemcheckLeak,
    MemcheckOverlap,
    MemcheckParam,
    MemcheckValue(uint),
    OtherType {
        pub tool_name: String,
        pub suppression_type: String,
    },
}

impl Show for SuppressionType {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), FormatError> {
        match self {
            &MemcheckAddr(n) => write!(fmt, "Memcheck:Addr{:u}", n),
            &MemcheckCond => write!(fmt, "Memcheck:Cond"),
            &MemcheckFree => write!(fmt, "Memcheck:Free"),
            &MemcheckLeak => write!(fmt, "Memcheck:Leak"),
            &MemcheckOverlap => write!(fmt, "Memcheck:Overlap"),
            &MemcheckParam => write!(fmt, "Memcheck:Param"),
            &MemcheckValue(n) => write!(fmt, "Memcheck:Value{:u}", n),
            &OtherType {
                tool_name: ref tool_name,
                suppression_type: ref suppression_type,
            } => {
                write!(fmt, "?{}:{}", tool_name.as_slice(), suppression_type.as_slice())
            },
        }
    }
}

/// Holds information about a single Valgrind suppression.
#[deriving(Clone)]
pub struct Suppression {
    /// The name of the suppression.
    pub name: String,
    /// The type of suppression.
    pub type_: SuppressionType,
    /// Any extra information, where used by the suppression type (e.g. a Memcheck `Param` suppression).
    pub opt_extra_info: Option<Vec<String>>,
    /// The calling context of the suppression.
    pub frames: Vec<Frame>,
}

impl Show for Suppression {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), FormatError> {
        (writeln!(fmt, "{{"))
            .and_then(|()| -> Result<(), FormatError> {
                writeln!(fmt, "   {}", self.name)
            })
            .and_then(|()| -> Result<(), FormatError> {
                writeln!(fmt, "   {}", self.type_)
            })
            .and_then(|()| -> Result<(), FormatError> {
                match self.opt_extra_info {
                    None => Ok(()),
                    Some(ref extra_info) => {
                        fold_(extra_info.iter().map(|line| -> Result<(), FormatError> {
                            writeln!(fmt, "   {}", line.as_slice())
                        }))
                    },
                }
            })
            .and_then(|()| -> Result<(), FormatError> {
                fold_(self.frames.iter().map(|frame| -> Result<(), FormatError> {
                    writeln!(fmt, "   {}", frame)
                }))
            })
            .and_then(|()| -> Result<(), FormatError> {
                write!(fmt, "}}")
            })
    }
}

/// A set of Valgrind suppressions.
#[deriving(Clone)]
pub struct Suppressions {
    suppressions_: Vec<Suppression>
}

enum ParseState {
    BeforeOpeningBrace,
    AfterOpeningBrace {
        opening_brace_lineno: uint,
    },
    HaveName {
        opening_brace_lineno: uint,
        name: String,
    },
    HaveSuppressionType {
        opening_brace_lineno: uint,
        name: String,
        tool_names: Vec<String>,
        suppression_type: String,
        /// Lines of extra information, used by some suppression types (e.g. a Memcheck `Param` suppression).
        opt_extra_info: Option<Vec<String>>,
    },
    HaveOptExtraInfo {
        opening_brace_lineno: uint,
        name: String,
        tool_names: Vec<String>,
        suppression_type: String,
        opt_extra_info: Option<Vec<String>>,
        frames: Vec<Frame>,
    },
}

impl Suppressions {

    /// Parses the suppressions from `buf` in Valgrind suppression syntax.
    ///
    /// # See also
    /// * [Suppressing errors](http://valgrind.org/docs/manual/manual-core.html#manual-core.suppress). Valgrind User Manual.
    pub fn parse<B: Buffer>(buf: &mut B) -> Result<Suppressions, ParseError> {
        let mut suppressions: Vec<Suppression> = Vec::new();

        let mut lineno = 0u;
        let mut state = BeforeOpeningBrace;
        for line_res in buf.lines() {
            match line_res {
                Err(e) => {
                    return Err(ParseError {
                        lineno: lineno,
                        message: format!("IoError returned: {}", e),
                    });
                },
                Ok(line) => {
                    lineno = lineno + 1;

                    let trimmed_line = line.as_slice().trim();
                    if !trimmed_line.is_empty() && !trimmed_line.starts_with("#") {
                        state = match state {
                                BeforeOpeningBrace => {
                                    if trimmed_line == "{" {
                                        AfterOpeningBrace {
                                            opening_brace_lineno: lineno
                                        }
                                    } else if trimmed_line.starts_with("{") {
                                        return Err(ParseError {
                                            lineno: lineno,
                                            message: String::from_str("expecting an opening brace on its own line"),
                                        });
                                    } else {
                                        return Err(ParseError {
                                            lineno: lineno,
                                            message: String::from_str("expecting an opening brace"),
                                        });
                                    }
                                },
                                AfterOpeningBrace {
                                    opening_brace_lineno: opening_brace_lineno,
                                } => {
                                    // If there is a closing brace immediately after the opening brace,
                                    // then skip this "empty" suppression (go back to the BeforeOpeningBrace
                                    // state).
                                    if trimmed_line == "}" {
                                        BeforeOpeningBrace
                                    } else if trimmed_line.contains_char('}') {
                                        return Err(ParseError {
                                            lineno: lineno,
                                            message: String::from_str("the suppression name cannot contain a closing brace '}'"),
                                        });
                                    } else {
                                        HaveName {
                                            opening_brace_lineno: opening_brace_lineno,
                                            name: trimmed_line.to_string(),
                                        }
                                    }
                                },
                                HaveName {
                                    opening_brace_lineno: opening_brace_lineno,
                                    name: name,
                                } => {
                                    let colon_pos = match trimmed_line.find(':') {
                                            None => {
                                                return Err(ParseError {
                                                    lineno: lineno,
                                                    message: String::from_str("no suppression type was found"),
                                                });
                                            }
                                            Some(colon_pos) => colon_pos
                                        };
                                    let splits = trimmed_line.slice_to(colon_pos).split(',');
                                    let tool_names: Vec<String> = splits.map(|part| part.to_string()).collect();
                                    HaveSuppressionType {
                                        opening_brace_lineno: opening_brace_lineno,
                                        name: name,
                                        tool_names: tool_names,
                                        suppression_type: trimmed_line.slice_from(colon_pos + 1).to_string(),
                                        opt_extra_info: None,
                                    }
                                },
                                HaveSuppressionType {
                                    opening_brace_lineno: opening_brace_lineno,
                                    name: name,
                                    tool_names: tool_names,
                                    suppression_type: suppression_type,
                                    opt_extra_info: opt_extra_info,
                                } => {
                                    if trimmed_line == "..." {
                                        HaveOptExtraInfo {
                                            opening_brace_lineno: opening_brace_lineno,
                                            name: name,
                                            tool_names: tool_names,
                                            suppression_type: suppression_type,
                                            opt_extra_info: opt_extra_info,
                                            frames: Vec::from_elem(1, FrameWildcard),
                                        }
                                    } else if trimmed_line.starts_with("obj:") {
                                        let glob = trimmed_line.slice_from(4).trim_left().to_string();
                                        HaveOptExtraInfo {
                                            opening_brace_lineno: opening_brace_lineno,
                                            name: name,
                                            tool_names: tool_names,
                                            suppression_type: suppression_type,
                                            opt_extra_info: opt_extra_info,
                                            frames: Vec::from_elem(1, ObjFrame { glob: glob }),
                                        }
                                    } else if trimmed_line.starts_with("fun:") {
                                        let glob = trimmed_line.slice_from(4).trim_left().to_string();
                                        HaveOptExtraInfo {
                                            opening_brace_lineno: opening_brace_lineno,
                                            name: name,
                                            tool_names: tool_names,
                                            suppression_type: suppression_type,
                                            opt_extra_info: opt_extra_info,
                                            frames: Vec::from_elem(1, ObjFrame { glob: glob }),
                                        }
                                    // If there is no calling context for this suppression, then skip it.
                                    // TODO This might not be 100% correct. Perhaps some suppressions only use extra info?
                                    } else if trimmed_line == "}" {
                                        BeforeOpeningBrace
                                    } else {
                                        let extra_info = match opt_extra_info {
                                                None => Vec::from_elem(1, trimmed_line.to_string()),
                                                Some(mut extra_info) => {
                                                    extra_info.push(trimmed_line.to_string());
                                                    extra_info
                                                }
                                            };
                                        HaveSuppressionType {
                                            opening_brace_lineno: opening_brace_lineno,
                                            name: name,
                                            tool_names: tool_names,
                                            suppression_type: suppression_type,
                                            opt_extra_info: Some(extra_info),
                                        }
                                    }
                                },
                                HaveOptExtraInfo {
                                    opening_brace_lineno: opening_brace_lineno,
                                    name: name,
                                    tool_names: tool_names,
                                    suppression_type: suppression_type,
                                    opt_extra_info: opt_extra_info,
                                    frames: mut frames,
                                } => {
                                    if trimmed_line == "..." {
                                        frames.push(FrameWildcard);
                                        HaveOptExtraInfo {
                                            opening_brace_lineno: opening_brace_lineno,
                                            name: name,
                                            tool_names: tool_names,
                                            suppression_type: suppression_type,
                                            opt_extra_info: opt_extra_info,
                                            frames: frames,
                                        }
                                    } else if trimmed_line.starts_with("obj:") {
                                        frames.push(ObjFrame {
                                            glob: trimmed_line.slice_from(4).trim_left().to_string(),
                                        });
                                        HaveOptExtraInfo {
                                            opening_brace_lineno: opening_brace_lineno,
                                            name: name,
                                            tool_names: tool_names,
                                            suppression_type: suppression_type,
                                            opt_extra_info: opt_extra_info,
                                            frames: frames,
                                        }
                                    } else if trimmed_line.starts_with("fun:") {
                                        frames.push(FunFrame {
                                            glob: trimmed_line.slice_from(4).trim_left().to_string(),
                                        });
                                        HaveOptExtraInfo {
                                            opening_brace_lineno: opening_brace_lineno,
                                            name: name,
                                            tool_names: tool_names,
                                            suppression_type: suppression_type,
                                            opt_extra_info: opt_extra_info,
                                            frames: frames,
                                        }
                                    } else if trimmed_line == "}" {

                                        suppressions.extend(tool_names.iter().map(|tool_name| -> Suppression {
                                            let type_ = if tool_name.as_slice() == "Memcheck" {
                                                    if suppression_type.as_slice().starts_with("Addr") {
                                                        match from_str(suppression_type.as_slice().slice_from(4)) {
                                                            None => OtherType {
                                                                tool_name: tool_name.to_string(),
                                                                suppression_type: suppression_type.clone(),
                                                            },
                                                            Some(n) => MemcheckAddr(n)
                                                        }
                                                    } else if suppression_type.as_slice() == "Cond" {
                                                        MemcheckCond
                                                    } else if suppression_type.as_slice() == "Free" {
                                                        MemcheckFree
                                                    } else if suppression_type.as_slice() == "Leak" {
                                                        MemcheckLeak
                                                    } else if suppression_type.as_slice() == "Overlap" {
                                                        MemcheckOverlap
                                                    } else if suppression_type.as_slice() == "Param" {
                                                        MemcheckParam
                                                    } else if suppression_type.as_slice().starts_with("Value") {
                                                        match from_str(suppression_type.as_slice().slice_from(5)) {
                                                            None => OtherType {
                                                                tool_name: tool_name.to_string(),
                                                                suppression_type: suppression_type.clone(),
                                                            },
                                                            Some(n) => MemcheckValue(n)
                                                        }
                                                    } else {
                                                        OtherType {
                                                            tool_name: tool_name.to_string(),
                                                            suppression_type: suppression_type.clone(),
                                                        }
                                                    }
                                                } else {
                                                    OtherType {
                                                        tool_name: tool_name.to_string(),
                                                        suppression_type: suppression_type.clone(),
                                                    }
                                                };
                                            Suppression {
                                                name: name.clone(),
                                                type_: type_,
                                                opt_extra_info: opt_extra_info.clone(),
                                                frames: frames.clone(),
                                            }
                                        }));

                                        BeforeOpeningBrace
                                    } else {
                                        return Err(ParseError {
                                            lineno: lineno,
                                            message: String::from_str("invalid calling context line"),
                                        });
                                    }
                                },
                            }; // end match state
                    }
                }, // end Ok(line)
            }
        }
        match state {
            AfterOpeningBrace {
                opening_brace_lineno: opening_brace_lineno,
                ..
            } => {
                return Err(ParseError {
                    lineno: opening_brace_lineno,
                    message: String::from_str("unexpectedly encountered EOF while parsing a suppression"),
                });
            },
            HaveName {
                opening_brace_lineno: opening_brace_lineno,
                name: name,
            } => {
                return Err(ParseError {
                    lineno: opening_brace_lineno,
                    message: format!("unexpectedly encountered EOF while parsing the suppression named '{}'", name.as_slice()),
                });
            },
            HaveSuppressionType {
                opening_brace_lineno: opening_brace_lineno,
                name: name,
                ..
            } => {
                return Err(ParseError {
                    lineno: opening_brace_lineno,
                    message: format!("unexpectedly encountered EOF while parsing the suppression named '{}'", name.as_slice()),
                });
            },
            HaveOptExtraInfo {
                opening_brace_lineno: opening_brace_lineno,
                name: name,
                ..
            } => {
                return Err(ParseError {
                    lineno: opening_brace_lineno,
                    message: format!("unexpectedly encountered EOF while parsing the suppression named '{}'", name.as_slice()),
                });
            },
            _ => (),
        }

        Ok(Suppressions {
            suppressions_: suppressions,
        })
    }

    /// Clones all the suppressions in `other` and adds them to these suppressions.
    pub fn add_all(&mut self, other: &Suppressions) {
        self.suppressions_.push_all(other.suppressions_.as_slice());
    }

    pub fn suppressions<'a>(&'a self) -> Items<'a, Suppression> {
        self.suppressions_.iter()
    }
}

impl Show for Suppressions {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), FormatError> {
        fold_(self.suppressions_.iter().map(|suppression| -> Result<(), FormatError> {
            writeln!(fmt, "{}", suppression)
        }))
    }
}
