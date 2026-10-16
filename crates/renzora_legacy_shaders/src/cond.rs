//! Boolean conditions over shader-def flags, and their WESL spelling.
//!
//! naga_oil kept conditions as nested `#ifdef` blocks. WESL puts a whole
//! condition on each declaration or statement it gates, so one statement deep
//! inside three `#ifdef`s needs all three joined into one `@if(A && B && !C)`.

use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cond {
    True,
    False,
    Flag(String),
    Not(Box<Cond>),
    And(Vec<Cond>),
    Or(Vec<Cond>),
}

impl Cond {
    pub fn flag(name: impl Into<String>) -> Self {
        Cond::Flag(name.into())
    }

    pub fn negate(c: Cond) -> Self {
        match c {
            Cond::True => Cond::False,
            Cond::False => Cond::True,
            Cond::Not(inner) => *inner,
            other => Cond::Not(Box::new(other)),
        }
    }

    pub fn and(parts: impl IntoIterator<Item = Cond>) -> Self {
        let mut out: Vec<Cond> = Vec::new();
        for part in parts {
            match part {
                Cond::True => {}
                Cond::False => return Cond::False,
                Cond::And(inner) => {
                    for c in inner {
                        if !out.contains(&c) {
                            out.push(c);
                        }
                    }
                }
                other => {
                    if !out.contains(&other) {
                        out.push(other);
                    }
                }
            }
        }
        // `X && !X` is unreachable code, not an error: a branch that can never
        // be taken is still a branch the author wrote.
        if out
            .iter()
            .any(|c| out.contains(&Cond::negate(c.clone())))
        {
            return Cond::False;
        }
        match out.len() {
            0 => Cond::True,
            1 => out.pop().unwrap_or(Cond::True),
            _ => Cond::And(out),
        }
    }

    pub fn or(parts: impl IntoIterator<Item = Cond>) -> Self {
        let mut out: Vec<Cond> = Vec::new();
        for part in parts {
            match part {
                Cond::False => {}
                Cond::True => return Cond::True,
                Cond::Or(inner) => {
                    for c in inner {
                        if !out.contains(&c) {
                            out.push(c);
                        }
                    }
                }
                other => {
                    if !out.contains(&other) {
                        out.push(other);
                    }
                }
            }
        }
        match out.len() {
            0 => Cond::False,
            1 => out.pop().unwrap_or(Cond::False),
            _ => Cond::Or(out),
        }
    }

    pub fn is_true(&self) -> bool {
        matches!(self, Cond::True)
    }

    /// Every flag the condition reads.
    pub fn flags(&self, out: &mut BTreeSet<String>) {
        match self {
            Cond::True | Cond::False => {}
            Cond::Flag(f) => {
                out.insert(f.clone());
            }
            Cond::Not(c) => c.flags(out),
            Cond::And(cs) | Cond::Or(cs) => cs.iter().for_each(|c| c.flags(out)),
        }
    }

    /// The condition's value with every flag in `set` true and every other flag
    /// false.
    pub fn eval(&self, set: &dyn Fn(&str) -> bool) -> bool {
        match self {
            Cond::True => true,
            Cond::False => false,
            Cond::Flag(f) => set(f),
            Cond::Not(c) => !c.eval(set),
            Cond::And(cs) => cs.iter().all(|c| c.eval(set)),
            Cond::Or(cs) => cs.iter().any(|c| c.eval(set)),
        }
    }

    /// Rename flags, for shader defs Bevy renamed between versions.
    pub fn rename(&self, map: &dyn Fn(&str) -> Option<String>) -> Cond {
        match self {
            Cond::Flag(f) => Cond::Flag(map(f).unwrap_or_else(|| f.clone())),
            Cond::Not(c) => Cond::negate(c.rename(map)),
            Cond::And(cs) => Cond::and(cs.iter().map(|c| c.rename(map))),
            Cond::Or(cs) => Cond::or(cs.iter().map(|c| c.rename(map))),
            other => other.clone(),
        }
    }
}

impl fmt::Display for Cond {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cond::True => write!(f, "true"),
            Cond::False => write!(f, "false"),
            Cond::Flag(name) => write!(f, "{name}"),
            Cond::Not(c) => match **c {
                Cond::Flag(_) | Cond::True | Cond::False => write!(f, "!{c}"),
                _ => write!(f, "!({c})"),
            },
            Cond::And(cs) => {
                for (i, c) in cs.iter().enumerate() {
                    if i > 0 {
                        write!(f, " && ")?;
                    }
                    match c {
                        Cond::Or(_) => write!(f, "({c})")?,
                        _ => write!(f, "{c}")?,
                    }
                }
                Ok(())
            }
            Cond::Or(cs) => {
                for (i, c) in cs.iter().enumerate() {
                    if i > 0 {
                        write!(f, " || ")?;
                    }
                    match c {
                        Cond::And(_) => write!(f, "({c})")?,
                        _ => write!(f, "{c}")?,
                    }
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(n: &str) -> Cond {
        Cond::flag(n)
    }

    #[test]
    fn and_flattens_and_detects_contradictions() {
        assert_eq!(Cond::and([f("A"), Cond::True, f("B")]).to_string(), "A && B");
        assert_eq!(Cond::and([f("A"), Cond::negate(f("A"))]), Cond::False);
        assert_eq!(Cond::and([]), Cond::True);
    }

    #[test]
    fn display_parenthesizes_mixed_operators() {
        let c = Cond::and([Cond::or([f("A"), f("B")]), Cond::negate(f("C"))]);
        assert_eq!(c.to_string(), "(A || B) && !C");
        assert_eq!(Cond::negate(Cond::and([f("A"), f("B")])).to_string(), "!(A && B)");
    }
}
