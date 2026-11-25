//! Declarations: what a plugin says it emits and provides.
//!
//! A declaration is the nearest the bus gets to a type signature. It is what
//! lets a call with a missing argument fail with a message naming it, instead
//! of deep inside a handler someone else wrote, and it is the list a panel or a
//! script binding reads to show what the installed plugins offer. Declaring is
//! optional: an undeclared event still delivers and an undeclared service still
//! answers, it just goes unchecked.

use crate::{BusKind, BusValue};

/// One named field of an event payload or a service's arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct BusField {
    pub name: String,
    pub kind: BusKind,
    /// A missing optional field passes the check; one that is present must
    /// still be the right kind.
    pub optional: bool,
}

/// An event a plugin emits.
#[derive(Clone, Debug, PartialEq)]
pub struct EventDecl {
    pub name: String,
    pub description: String,
    pub fields: Vec<BusField>,
}

impl EventDecl {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), description: String::new(), fields: Vec::new() }
    }

    pub fn describe(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn field(mut self, name: impl Into<String>, kind: BusKind) -> Self {
        self.fields.push(BusField { name: name.into(), kind, optional: false });
        self
    }

    pub fn optional_field(mut self, name: impl Into<String>, kind: BusKind) -> Self {
        self.fields.push(BusField { name: name.into(), kind, optional: true });
        self
    }
}

impl From<&str> for EventDecl {
    fn from(name: &str) -> Self {
        EventDecl::new(name)
    }
}

/// A service a plugin answers.
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceDecl {
    pub name: String,
    pub description: String,
    pub args: Vec<BusField>,
    /// Documentation only: an answer is not checked against it, because the
    /// caller is the one who has to cope with a surprise and a log line from
    /// the bus would not help them.
    pub returns: BusKind,
}

impl ServiceDecl {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            args: Vec::new(),
            returns: BusKind::Any,
        }
    }

    pub fn describe(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn arg(mut self, name: impl Into<String>, kind: BusKind) -> Self {
        self.args.push(BusField { name: name.into(), kind, optional: false });
        self
    }

    pub fn optional_arg(mut self, name: impl Into<String>, kind: BusKind) -> Self {
        self.args.push(BusField { name: name.into(), kind, optional: true });
        self
    }

    pub fn returns(mut self, kind: BusKind) -> Self {
        self.returns = kind;
        self
    }
}

impl From<&str> for ServiceDecl {
    fn from(name: &str) -> Self {
        ServiceDecl::new(name)
    }
}

/// Checks a payload against declared fields. No fields declared means nothing
/// to check, so any payload passes; keys nobody declared are allowed, so a
/// provider can add a field without breaking callers that already send it.
pub(crate) fn check(fields: &[BusField], value: &BusValue) -> Result<(), String> {
    if fields.is_empty() {
        return Ok(());
    }
    let BusValue::Map(map) = value else {
        return Err(format!("expected a map of fields, got {}", value.kind()));
    };
    for field in fields {
        match map.get(&field.name) {
            Some(v) if field.kind.accepts(v) => {}
            Some(BusValue::Null) if field.optional => {}
            Some(v) => {
                return Err(format!("`{}` should be {}, got {}", field.name, field.kind, v.kind()));
            }
            None if field.optional => {}
            None => return Err(format!("`{}` is missing", field.name)),
        }
    }
    Ok(())
}
