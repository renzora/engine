use std::fmt;

use crate::context::AttributeContext;
use crate::attribute_id::{Interner, AttributeId};
use crate::tags::{TagMask, TagResolver};

// ---------------------------------------------------------------------------
// Op — bytecode instructions
// ---------------------------------------------------------------------------

/// A single bytecode instruction for the expression VM.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Push a literal constant onto the stack.
    Const(f32),
    /// Load the current value of a local attribute from the context.
    Load(AttributeId),
    /// Load a attribute value from a cross-entity source.
    ///
    /// At eval time, reads from the local context using `cache_key`
    /// (a composite AttributeId like `"Strength@Wielder"`). The caller is
    /// responsible for caching source values under that key.
    LoadSource {
        /// The interned alias name (e.g., "Wielder").
        alias: AttributeId,
        /// The attribute to read from the source entity.
        attribute: AttributeId,
        /// Pre-computed composite key: `"AttributeName@Alias"`, interned.
        /// Used for local context lookup during evaluation.
        cache_key: AttributeId,
    },
    /// Load a tag-filtered attribute value from a cross-entity source.
    ///
    /// Like `LoadSource` but carries a [`TagMask`] so the caller can
    /// read the source entity's value via `get_tagged` instead of `get`.
    LoadSourceTagged {
        alias: AttributeId,
        attribute: AttributeId,
        mask: TagMask,
        cache_key: AttributeId,
    },
    // Binary arithmetic (pops two, pushes one)
    Add,
    Sub,
    Mul,
    Div,
    // Unary (pops one, pushes one)
    Neg,
    // Comparison (pops two, pushes 1.0 or 0.0)
    /// a > b → 1.0 if true, else 0.0
    Gt,
    /// a < b → 1.0 if true, else 0.0
    Lt,
    /// a >= b → 1.0 if true, else 0.0
    Ge,
    /// a <= b → 1.0 if true, else 0.0
    Le,
    /// a == b → 1.0 if equal within f32::EPSILON, else 0.0
    Eq,
    /// a != b → 1.0 if not equal within f32::EPSILON, else 0.0
    Ne,
    // Logical (pops two, pushes 1.0 or 0.0)
    /// a && b → 1.0 if both non-zero, else 0.0
    And,
    /// a || b → 1.0 if either non-zero, else 0.0
    Or,
    // Built-in functions
    /// max(a, b) — pops two, pushes one.
    Max,
    /// min(a, b) — pops two, pushes one.
    Min,
    /// abs(x) — pops one, pushes one.
    Abs,
    /// clamp(x, lo, hi) — pops three, pushes one.
    Clamp,
}

// ---------------------------------------------------------------------------
// Expr — compiled expression
// ---------------------------------------------------------------------------

/// A compiled bytecode expression.
///
/// Created via `Expr::compile()` from a string expression like `"Strength / 10.0"`.
/// Evaluated via `Expr::evaluate()` against a `AttributeContext`.
#[derive(Clone, Debug)]
pub struct Expr {
    /// The bytecode ops.
    pub(crate) ops: Vec<Op>,
    /// AttributeIds this expression depends on (for dependency tracking).
    /// Includes both local and cross-entity dependencies.
    pub(crate) dependencies: Vec<Dependency>,
    /// Original source string (kept for debugging and modifier identity).
    pub(crate) source: String,
}

/// A dependency extracted from an expression at compile time.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Dependency {
    /// A local attribute reference (e.g., `Strength`).
    Local(AttributeId),
    /// A cross-entity attribute reference (e.g., `Strength@Wielder`).
    Source { alias: AttributeId, attribute: AttributeId },
    /// A cross-entity attribute reference filtered by tags (e.g., `Damage{FIRE}@weapon`).
    SourceTagQuery { alias: AttributeId, attribute: AttributeId, mask: TagMask },
    /// A local attribute reference filtered by tags (e.g., `Damage.Added{FIRE|SPELL}`).
    ///
    /// The expression depends on the synthetic tag-query node. The synthetic
    /// node itself depends on the parent attribute and is set up by
    /// `AttributesMut::ensure_tag_query`.
    TagQuery {
        /// The parent attribute being queried.
        attribute: AttributeId,
        /// The tag mask for the query.
        mask: TagMask,
        /// The synthetic AttributeId for the materialized query node.
        synthetic: AttributeId,
    },
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

// ---------------------------------------------------------------------------
// CompileError
// ---------------------------------------------------------------------------

/// Errors that can occur during expression compilation.
#[derive(Clone, Debug, PartialEq)]
pub enum CompileError {
    /// Unexpected character in input.
    UnexpectedChar(char, usize),
    /// Unexpected end of input.
    UnexpectedEof,
    /// Expected a specific token but got something else.
    Expected(String),
    /// Unknown function name.
    UnknownFunction(String),
    /// Unknown tag name in a `{TAG}` expression.
    UnknownTag(String),
    /// Empty expression.
    EmptyExpression,
    /// A [`TagMask`](crate::tags::TagMask) could not be decomposed into named
    /// tags for expression generation (some bits have no registered name in
    /// the [`TagResolver`](crate::tags::TagResolver)).
    UnresolvableTagMask(crate::tags::TagMask),
    /// A tag name is ambiguous — it was registered by multiple namespaces.
    /// The `Vec<String>` contains the fully-qualified alternatives.
    AmbiguousTag(String, Vec<String>),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::UnexpectedChar(c, pos) => {
                write!(f, "unexpected character '{}' at position {}", c, pos)
            }
            CompileError::UnexpectedEof => write!(f, "unexpected end of expression"),
            CompileError::Expected(msg) => write!(f, "expected {}", msg),
            CompileError::UnknownFunction(name) => write!(f, "unknown function '{}'", name),
            CompileError::UnknownTag(name) => write!(f, "unknown tag '{}' (is it registered in TagResolver?)", name),
            CompileError::EmptyExpression => write!(f, "empty expression"),
            CompileError::UnresolvableTagMask(mask) => write!(
                f,
                "cannot decompose TagMask({}) into named tags — \
                 some bits are not registered in TagResolver",
                mask.0
            ),
            CompileError::AmbiguousTag(name, alternatives) => write!(
                f,
                "ambiguous tag '{}' — registered by multiple namespaces, use one of: {}",
                name,
                alternatives.join(", ")
            ),
        }
    }
}

impl std::error::Error for CompileError {}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f32),
    Ident(String), // attribute name or function name
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    At,           // @ for cross-entity references
    Dot,          // . for attribute path separators
    LBrace,       // { for tag query open
    RBrace,       // } for tag query close
    Pipe,         // | for tag OR within braces
    ColonColon,   // :: for namespaced tags
    // Comparison
    GreaterThan,  // >
    LessThan,     // <
    GreaterEqual, // >=
    LessEqual,    // <=
    EqualEqual,   // ==
    BangEqual,    // !=
    // Logical
    AmpAmp,       // &&
    PipePipe,     // ||
    Eof,
}

struct Tokenizer {
    chars: Vec<char>,
    pos: usize,
}

impl Tokenizer {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn next_token(&mut self) -> Result<Token, CompileError> {
        self.skip_whitespace();

        if self.pos >= self.chars.len() {
            return Ok(Token::Eof);
        }

        let ch = self.chars[self.pos];
        match ch {
            '+' => { self.pos += 1; Ok(Token::Plus) }
            '-' => { self.pos += 1; Ok(Token::Minus) }
            '*' => { self.pos += 1; Ok(Token::Star) }
            '/' => { self.pos += 1; Ok(Token::Slash) }
            '(' => { self.pos += 1; Ok(Token::LParen) }
            ')' => { self.pos += 1; Ok(Token::RParen) }
            ',' => { self.pos += 1; Ok(Token::Comma) }
            '@' => { self.pos += 1; Ok(Token::At) }
            '{' => { self.pos += 1; Ok(Token::LBrace) }
            '}' => { self.pos += 1; Ok(Token::RBrace) }
            '|' if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '|' => {
                self.pos += 2; Ok(Token::PipePipe)
            }
            '|' => { self.pos += 1; Ok(Token::Pipe) }
            '&' if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '&' => {
                self.pos += 2; Ok(Token::AmpAmp)
            }
            '>' if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' => {
                self.pos += 2; Ok(Token::GreaterEqual)
            }
            '>' => { self.pos += 1; Ok(Token::GreaterThan) }
            '<' if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' => {
                self.pos += 2; Ok(Token::LessEqual)
            }
            '<' => { self.pos += 1; Ok(Token::LessThan) }
            '=' if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' => {
                self.pos += 2; Ok(Token::EqualEqual)
            }
            '!' if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' => {
                self.pos += 2; Ok(Token::BangEqual)
            }
            ':' if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == ':' => {
                self.pos += 2; Ok(Token::ColonColon)
            }
            '.' if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1].is_ascii_digit() => {
                // Decimal number starting with '.' like .5
                self.read_number()
            }
            '.' => { self.pos += 1; Ok(Token::Dot) }
            c if c.is_ascii_digit() => self.read_number(),
            c if c.is_ascii_alphabetic() || c == '_' => self.read_ident(),
            c => Err(CompileError::UnexpectedChar(c, self.pos)),
        }
    }

    fn read_number(&mut self) -> Result<Token, CompileError> {
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_digit() || self.chars[self.pos] == '.')
        {
            self.pos += 1;
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        let val: f32 = s
            .parse()
            .map_err(|_| CompileError::Expected(format!("valid number, got '{}'", s)))?;
        Ok(Token::Number(val))
    }

    fn read_ident(&mut self) -> Result<Token, CompileError> {
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_alphanumeric() || self.chars[self.pos] == '_')
        {
            self.pos += 1;
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        Ok(Token::Ident(s))
    }
}

// ---------------------------------------------------------------------------
// Parser (Pratt / precedence climbing)
// ---------------------------------------------------------------------------

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    interner: &'a Interner,
    tags: Option<&'a TagResolver>,
    ops: Vec<Op>,
    dependencies: Vec<Dependency>,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token>, interner: &'a Interner, tags: Option<&'a TagResolver>) -> Self {
        Self {
            tokens,
            pos: 0,
            interner,
            tags,
            ops: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), CompileError> {
        let tok = self.advance();
        if &tok == expected {
            Ok(())
        } else {
            Err(CompileError::Expected(format!("{:?}, got {:?}", expected, tok)))
        }
    }

    /// Parse a full expression.
    ///
    /// Precedence (lowest to highest):
    /// - `||`              l_bp=1,  r_bp=2
    /// - `&&`              l_bp=3,  r_bp=4
    /// - `> < >= <= == !=` l_bp=5,  r_bp=6
    /// - `+ -`            l_bp=7,  r_bp=8
    /// - `* /`            l_bp=9,  r_bp=10
    /// - unary `-`        bp=11
    fn parse_expression(&mut self, min_bp: u8) -> Result<(), CompileError> {
        // Prefix (atom or unary)
        self.parse_prefix()?;

        // Infix loop
        loop {
            let op = match self.peek() {
                // Logical
                Token::PipePipe => (Op::Or, 1, 2),
                Token::AmpAmp => (Op::And, 3, 4),
                // Comparison
                Token::GreaterThan => (Op::Gt, 5, 6),
                Token::LessThan => (Op::Lt, 5, 6),
                Token::GreaterEqual => (Op::Ge, 5, 6),
                Token::LessEqual => (Op::Le, 5, 6),
                Token::EqualEqual => (Op::Eq, 5, 6),
                Token::BangEqual => (Op::Ne, 5, 6),
                // Arithmetic
                Token::Plus => (Op::Add, 7, 8),
                Token::Minus => (Op::Sub, 7, 8),
                Token::Star => (Op::Mul, 9, 10),
                Token::Slash => (Op::Div, 9, 10),
                _ => break,
            };

            let (op_code, l_bp, r_bp) = op;
            if l_bp < min_bp {
                break;
            }

            self.advance(); // consume operator
            self.parse_expression(r_bp)?;
            self.ops.push(op_code);
        }

        Ok(())
    }

    /// Parse a prefix expression (atom, unary minus, parenthesized, function call).
    fn parse_prefix(&mut self) -> Result<(), CompileError> {
        match self.peek().clone() {
            Token::Number(val) => {
                self.advance();
                self.ops.push(Op::Const(val));
                Ok(())
            }
            Token::Minus => {
                self.advance();
                self.parse_expression(11)?; // unary minus binds tightest
                self.ops.push(Op::Neg);
                Ok(())
            }
            Token::LParen => {
                self.advance();
                self.parse_expression(0)?;
                self.expect(&Token::RParen)?;
                Ok(())
            }
            Token::Ident(name) => {
                self.advance();
                // Check if this is a function call
                if self.peek() == &Token::LParen {
                    self.parse_function_call(&name)
                } else {
                    // Attribute reference — may have dot-separated parts and/or @alias
                    self.parse_attribute_reference(name)
                }
            }
            Token::Eof => Err(CompileError::UnexpectedEof),
            _ => {
                let tok = self.advance();
                Err(CompileError::Expected(format!(
                    "number, identifier, or '(', got {:?}",
                    tok
                )))
            }
        }
    }

    /// Parse a attribute reference like `Strength`, `Damage.current`, `Strength@Wielder`,
    /// or `Damage.Added{FIRE|SPELL}`.
    fn parse_attribute_reference(&mut self, first_part: String) -> Result<(), CompileError> {
        // Accumulate dot-separated parts: Damage.current.etc
        let mut full_name = first_part;
        while self.peek() == &Token::Dot {
            self.advance(); // consume dot
            match self.advance() {
                Token::Ident(part) => {
                    full_name.push('.');
                    full_name.push_str(&part);
                }
                other => {
                    return Err(CompileError::Expected(format!(
                        "identifier after '.', got {:?}",
                        other
                    )));
                }
            }
        }

        // Check for {TAG|TAG} (tag query)
        let tag_mask = if self.peek() == &Token::LBrace {
            Some(self.parse_tag_query()?)
        } else {
            None
        };

        // Check for @Alias (cross-entity reference)
        if self.peek() == &Token::At {
            self.advance(); // consume @
            match self.advance() {
                Token::Ident(alias_name) => {
                    let attribute_id = self.interner.get_or_intern(&full_name);
                    let alias_id = self.interner.get_or_intern(&alias_name);

                    if let Some(mask) = tag_mask {
                        let tagged_composite = format!("\0tag:{}@{}:{}", full_name, alias_name, mask.0);
                        let cache_key = self.interner.get_or_intern(&tagged_composite);
                        self.dependencies.push(Dependency::SourceTagQuery {
                            alias: alias_id,
                            attribute: attribute_id,
                            mask,
                        });
                        self.ops.push(Op::LoadSourceTagged {
                            alias: alias_id,
                            attribute: attribute_id,
                            mask,
                            cache_key,
                        });
                    } else {
                        let composite = format!("{}@{}", full_name, alias_name);
                        let cache_key = self.interner.get_or_intern(&composite);
                        self.dependencies.push(Dependency::Source {
                            alias: alias_id,
                            attribute: attribute_id,
                        });
                        self.ops.push(Op::LoadSource {
                            alias: alias_id,
                            attribute: attribute_id,
                            cache_key,
                        });
                    }
                }
                other => {
                    return Err(CompileError::Expected(format!(
                        "alias name after '@', got {:?}",
                        other
                    )));
                }
            }
        } else if let Some(mask) = tag_mask {
            // Local tagged reference: Damage.Added{FIRE|SPELL}
            let attribute_id = self.interner.get_or_intern(&full_name);
            let synthetic_name = format!("\0tag:{}:{}", full_name, mask.0);
            let synthetic_id = self.interner.get_or_intern(&synthetic_name);
            self.dependencies.push(Dependency::TagQuery {
                attribute: attribute_id,
                mask,
                synthetic: synthetic_id,
            });
            // Emit a plain Load on the synthetic ID — by the time the expression
            // evaluates, the synthetic tag-query node will have been materialized
            // by AttributesMut and its value cached in the AttributeContext.
            self.ops.push(Op::Load(synthetic_id));
        } else {
            // Local attribute reference
            let attribute_id = self.interner.get_or_intern(&full_name);
            self.dependencies.push(Dependency::Local(attribute_id));
            self.ops.push(Op::Load(attribute_id));
        }

        Ok(())
    }

    /// Parse the contents of a `{TAG1|TAG2|...}` tag query.
    /// The opening `{` must be the current token.
    fn parse_tag_query(&mut self) -> Result<TagMask, CompileError> {
        self.advance(); // consume {

        let tags = self.tags.ok_or_else(|| {
            CompileError::Expected(
                "TagResolver required for {TAG} syntax — pass Some(&resolver) to Expr::compile".to_string(),
            )
        })?;

        let mut mask = TagMask::NONE;
        loop {
            match self.advance() {
                Token::Ident(name) => {
                    // Check for namespaced form: Namespace::TAG
                    let full_name = if self.peek() == &Token::ColonColon {
                        self.advance(); // consume ::
                        match self.advance() {
                            Token::Ident(tag_part) => format!("{}::{}", name, tag_part),
                            other => {
                                return Err(CompileError::Expected(format!(
                                    "tag name after '::', got {:?}",
                                    other
                                )));
                            }
                        }
                    } else {
                        name
                    };

                    let tag = match tags.resolve(&full_name) {
                        Some(m) => m,
                        None => {
                            if let Some(alts) = tags.ambiguous_alternatives(&full_name) {
                                return Err(CompileError::AmbiguousTag(full_name, alts));
                            }
                            return Err(CompileError::UnknownTag(full_name));
                        }
                    };
                    mask = mask | tag;
                }
                other => {
                    return Err(CompileError::Expected(format!(
                        "tag name inside {{}}, got {:?}",
                        other
                    )));
                }
            }
            match self.peek() {
                Token::Pipe => {
                    self.advance(); // consume | and continue
                }
                Token::RBrace => {
                    self.advance(); // consume }
                    break;
                }
                _ => {
                    return Err(CompileError::Expected(
                        "'|' or '}' inside tag query".to_string(),
                    ));
                }
            }
        }

        Ok(mask)
    }

    /// Parse a function call like `max(a, b)`, `min(a, b)`, `abs(x)`, `clamp(x, lo, hi)`.
    fn parse_function_call(&mut self, name: &str) -> Result<(), CompileError> {
        self.advance(); // consume '('

        match name {
            "max" => {
                self.parse_expression(0)?;
                self.expect(&Token::Comma)?;
                self.parse_expression(0)?;
                self.expect(&Token::RParen)?;
                self.ops.push(Op::Max);
                Ok(())
            }
            "min" => {
                self.parse_expression(0)?;
                self.expect(&Token::Comma)?;
                self.parse_expression(0)?;
                self.expect(&Token::RParen)?;
                self.ops.push(Op::Min);
                Ok(())
            }
            "abs" => {
                self.parse_expression(0)?;
                self.expect(&Token::RParen)?;
                self.ops.push(Op::Abs);
                Ok(())
            }
            "clamp" => {
                self.parse_expression(0)?;
                self.expect(&Token::Comma)?;
                self.parse_expression(0)?;
                self.expect(&Token::Comma)?;
                self.parse_expression(0)?;
                self.expect(&Token::RParen)?;
                self.ops.push(Op::Clamp);
                Ok(())
            }
            _ => Err(CompileError::UnknownFunction(name.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// Expr implementation
// ---------------------------------------------------------------------------

impl Expr {
    /// Compile an expression string into bytecode.
    ///
    /// Attribute name strings are resolved to [`AttributeId`] via the global
    /// [`Interner`] at compile time.  When `tags` is `Some`, expressions may
    /// use `{TAG|TAG}` syntax to reference tag-filtered attribute values
    /// (e.g., `Damage.Added{FIRE|SPELL} * 2`). Tag names are resolved via
    /// the provided [`TagResolver`].
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let expr = Expr::compile("Strength / 10.0", None)?;
    /// let expr = Expr::compile("Strength@Wielder * 2.0", None)?;
    /// let expr = Expr::compile("max(Health, 0.0)", None)?;
    /// let expr = Expr::compile(
    ///     "Damage.Added{FIRE|SPELL} * 2.0",
    ///     Some(&tag_resolver),
    /// )?;
    /// ```
    pub fn compile(
        source: &str,
        tags: Option<&TagResolver>,
    ) -> Result<Self, CompileError> {
        let interner = Interner::global();
        let trimmed = source.trim();
        if trimmed.is_empty() {
            return Err(CompileError::EmptyExpression);
        }

        // Tokenize
        let mut tokenizer = Tokenizer::new(trimmed);
        let mut tokens = Vec::new();
        loop {
            let tok = tokenizer.next_token()?;
            let is_eof = tok == Token::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }

        // Parse
        let mut parser = Parser::new(tokens, &interner, tags);
        parser.parse_expression(0)?;

        if parser.peek() != &Token::Eof {
            return Err(CompileError::Expected(format!(
                "end of expression, got {:?}",
                parser.peek()
            )));
        }

        Ok(Self {
            ops: parser.ops,
            dependencies: parser.dependencies,
            source: source.to_string(),
        })
    }

    /// Evaluate this expression against a attribute context.
    ///
    /// Cross-entity `LoadSource` ops read from the local context via their
    /// pre-computed `cache_key`. The caller must ensure source values are
    /// cached under those composite keys (e.g., `"Strength@Wielder"`).
    pub fn evaluate(&self, context: &AttributeContext) -> f32 {
        let mut stack = [0.0f32; 16];
        let mut sp: usize = 0;

        for op in &self.ops {
            match op {
                Op::Const(val) => {
                    stack[sp] = *val;
                    sp += 1;
                }
                Op::Load(id) => {
                    stack[sp] = context.get(*id);
                    sp += 1;
                }
                Op::LoadSource { cache_key, .. } | Op::LoadSourceTagged { cache_key, .. } => {
                    stack[sp] = context.get(*cache_key);
                    sp += 1;
                }
                Op::Add => {
                    sp -= 1;
                    let b = stack[sp];
                    sp -= 1;
                    stack[sp] = stack[sp] + b;
                    sp += 1;
                }
                Op::Sub => {
                    sp -= 1;
                    let b = stack[sp];
                    sp -= 1;
                    stack[sp] = stack[sp] - b;
                    sp += 1;
                }
                Op::Mul => {
                    sp -= 1;
                    let b = stack[sp];
                    sp -= 1;
                    stack[sp] = stack[sp] * b;
                    sp += 1;
                }
                Op::Div => {
                    sp -= 1;
                    let b = stack[sp];
                    sp -= 1;
                    stack[sp] = if b.abs() < f32::EPSILON {
                        0.0
                    } else {
                        stack[sp] / b
                    };
                    sp += 1;
                }
                Op::Neg => {
                    stack[sp - 1] = -stack[sp - 1];
                }
                // Comparison
                Op::Gt => {
                    sp -= 1; let b = stack[sp];
                    sp -= 1; stack[sp] = if stack[sp] > b { 1.0 } else { 0.0 };
                    sp += 1;
                }
                Op::Lt => {
                    sp -= 1; let b = stack[sp];
                    sp -= 1; stack[sp] = if stack[sp] < b { 1.0 } else { 0.0 };
                    sp += 1;
                }
                Op::Ge => {
                    sp -= 1; let b = stack[sp];
                    sp -= 1; stack[sp] = if stack[sp] >= b { 1.0 } else { 0.0 };
                    sp += 1;
                }
                Op::Le => {
                    sp -= 1; let b = stack[sp];
                    sp -= 1; stack[sp] = if stack[sp] <= b { 1.0 } else { 0.0 };
                    sp += 1;
                }
                Op::Eq => {
                    sp -= 1; let b = stack[sp];
                    sp -= 1; stack[sp] = if (stack[sp] - b).abs() < f32::EPSILON { 1.0 } else { 0.0 };
                    sp += 1;
                }
                Op::Ne => {
                    sp -= 1; let b = stack[sp];
                    sp -= 1; stack[sp] = if (stack[sp] - b).abs() >= f32::EPSILON { 1.0 } else { 0.0 };
                    sp += 1;
                }
                // Logical
                Op::And => {
                    sp -= 1; let b = stack[sp];
                    sp -= 1; stack[sp] = if stack[sp] != 0.0 && b != 0.0 { 1.0 } else { 0.0 };
                    sp += 1;
                }
                Op::Or => {
                    sp -= 1; let b = stack[sp];
                    sp -= 1; stack[sp] = if stack[sp] != 0.0 || b != 0.0 { 1.0 } else { 0.0 };
                    sp += 1;
                }
                Op::Max => {
                    sp -= 1;
                    let b = stack[sp];
                    sp -= 1;
                    stack[sp] = stack[sp].max(b);
                    sp += 1;
                }
                Op::Min => {
                    sp -= 1;
                    let b = stack[sp];
                    sp -= 1;
                    stack[sp] = stack[sp].min(b);
                    sp += 1;
                }
                Op::Abs => {
                    stack[sp - 1] = stack[sp - 1].abs();
                }
                Op::Clamp => {
                    sp -= 1;
                    let hi = stack[sp];
                    sp -= 1;
                    let lo = stack[sp];
                    sp -= 1;
                    stack[sp] = stack[sp].clamp(lo, hi);
                    sp += 1;
                }
            }
        }

        if sp == 0 { 0.0 } else { stack[sp - 1] }
    }

    /// Get the dependencies this expression reads from.
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    /// Iterate over source cache entries: `(alias, attribute, cache_key, tag_mask)`.
    ///
    /// Used by `AttributesMut` to know which composite keys to populate
    /// in the local context when a source alias is set/changed. When
    /// `tag_mask` is `Some`, the value should be read via `get_tagged`.
    pub fn source_cache_keys(&self) -> impl Iterator<Item = (AttributeId, AttributeId, AttributeId, Option<TagMask>)> + '_ {
        self.ops
            .iter()
            .filter_map(|op| match op {
                Op::LoadSource { alias, attribute, cache_key } => {
                    Some((*alias, *attribute, *cache_key, None))
                }
                Op::LoadSourceTagged { alias, attribute, cache_key, mask } => {
                    Some((*alias, *attribute, *cache_key, Some(*mask)))
                }
                _ => None,
            })
    }

    /// Get the source string this expression was compiled from.
    pub fn source(&self) -> &str {
        &self.source
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute_id::Interner;

    fn test_interner() -> Interner {
        let i = Interner::new();
        i.set_global();
        Interner::global()
    }

    fn eval(source: &str, ctx: &AttributeContext) -> f32 {
        let expr = Expr::compile(source, None).unwrap();
        expr.evaluate(ctx)
    }

    #[test]
    fn literal() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("42.0", &ctx), 42.0);
    }

    #[test]
    fn simple_arithmetic() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("2.0 + 3.0", &ctx), 5.0);
        assert_eq!(eval("10.0 - 4.0", &ctx), 6.0);
        assert_eq!(eval("3.0 * 4.0", &ctx), 12.0);
        assert_eq!(eval("10.0 / 4.0", &ctx), 2.5);
    }

    #[test]
    fn precedence() {
        test_interner();
        let ctx = AttributeContext::new();
        // 2 + 3 * 4 = 14, not 20
        assert_eq!(eval("2.0 + 3.0 * 4.0", &ctx), 14.0);
        // (2 + 3) * 4 = 20
        assert_eq!(eval("(2.0 + 3.0) * 4.0", &ctx), 20.0);
    }

    #[test]
    fn unary_neg() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("-5.0", &ctx), -5.0);
        assert_eq!(eval("-(3.0 + 2.0)", &ctx), -5.0);
    }

    #[test]
    fn attribute_reference() {
        let interner = test_interner();
        let mut ctx = AttributeContext::new();
        let str_id = interner.get_or_intern("Strength");
        ctx.set(str_id, 25.0);

        assert_eq!(eval("Strength / 10.0", &ctx), 2.5);
    }

    #[test]
    fn dotted_attribute_reference() {
        let interner = test_interner();
        let mut ctx = AttributeContext::new();
        let id = interner.get_or_intern("Damage.current");
        ctx.set(id, 80.0);

        assert_eq!(eval("Damage.current / 100.0", &ctx), 0.8);
    }

    #[test]
    fn cross_entity_reference_compiles() {
        let interner = test_interner();
        let expr = Expr::compile("Strength@Wielder * 2.0", None).unwrap();
        assert_eq!(expr.dependencies.len(), 1);
        match &expr.dependencies[0] {
            Dependency::Source { alias, attribute } => {
                assert_eq!(interner.resolve(*alias), "Wielder");
                assert_eq!(interner.resolve(*attribute), "Strength");
            }
            _ => panic!("expected Source dependency"),
        }
    }

    #[test]
    fn builtin_max() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("max(3.0, 7.0)", &ctx), 7.0);
        assert_eq!(eval("max(-1.0, 0.0)", &ctx), 0.0);
    }

    #[test]
    fn builtin_min() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("min(3.0, 7.0)", &ctx), 3.0);
    }

    #[test]
    fn builtin_abs() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("abs(-5.0)", &ctx), 5.0);
        assert_eq!(eval("abs(5.0)", &ctx), 5.0);
    }

    #[test]
    fn builtin_clamp() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("clamp(15.0, 0.0, 10.0)", &ctx), 10.0);
        assert_eq!(eval("clamp(-5.0, 0.0, 10.0)", &ctx), 0.0);
        assert_eq!(eval("clamp(5.0, 0.0, 10.0)", &ctx), 5.0);
    }

    #[test]
    fn complex_expression() {
        let interner = test_interner();
        let mut ctx = AttributeContext::new();
        let base = interner.get_or_intern("base");
        let increased = interner.get_or_intern("increased");
        let more = interner.get_or_intern("more");
        ctx.set(base, 100.0);
        ctx.set(increased, 0.5);
        ctx.set(more, 1.3);

        // PoE-style: base * (1 + increased) * more = 100 * 1.5 * 1.3 = 195
        let result = eval("base * (1.0 + increased) * more", &ctx);
        assert!((result - 195.0).abs() < 0.001);
    }

    #[test]
    fn division_by_zero_returns_zero() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("1.0 / 0.0", &ctx), 0.0);
    }

    #[test]
    fn empty_expression_error() {
        test_interner();
        assert!(Expr::compile("", None).is_err());
        assert!(Expr::compile("   ", None).is_err());
    }

    #[test]
    fn unknown_function_error() {
        test_interner();
        assert!(matches!(
            Expr::compile("foo(1.0)", None),
            Err(CompileError::UnknownFunction(_))
        ));
    }

    #[test]
    fn equilibrium_decay_pattern() {
        let interner = test_interner();
        let mut ctx = AttributeContext::new();
        let eq_id = interner.get_or_intern("equilibrium");
        let cur_id = interner.get_or_intern("current");
        ctx.set(eq_id, 70.0);
        ctx.set(cur_id, 40.0);

        // (equilibrium - current) * 0.05 = (70 - 40) * 0.05 = 1.5
        let result = eval("(equilibrium - current) * 0.05", &ctx);
        assert!((result - 1.5).abs() < 0.001);

        // When current > equilibrium, result is negative (natural pressure downward)
        ctx.set(cur_id, 90.0);
        let result = eval("(equilibrium - current) * 0.05", &ctx);
        assert!((result - -1.0).abs() < 0.001);
    }

    // --- Tag expression tests ---

    #[test]
    fn tag_query_compiles() {
        let interner = test_interner();
        let mut tags = TagResolver::new();
        let fire = TagMask::bit(0);
        let spell = TagMask::bit(3);
        tags.register("FIRE", fire);
        tags.register("SPELL", spell);

        let expr = Expr::compile(
            "Damage.Added{FIRE|SPELL} * 2.0",
            Some(&tags),
        )
        .unwrap();

        // Should have one TagQuery dependency
        assert_eq!(expr.dependencies.len(), 1);
        match &expr.dependencies[0] {
            Dependency::TagQuery { attribute, mask, synthetic } => {
                assert_eq!(interner.resolve(*attribute), "Damage.Added");
                assert_eq!(*mask, fire | spell);
                // Synthetic ID should be interned
                let expected_name = format!("\0tag:Damage.Added:{}", (fire | spell).0);
                assert_eq!(interner.resolve(*synthetic), expected_name);
            }
            other => panic!("expected TagQuery dependency, got {:?}", other),
        }
    }

    #[test]
    fn tag_query_evaluates_with_synthetic_context() {
        let interner = test_interner();
        let mut tags = TagResolver::new();
        let fire = TagMask::bit(0);
        tags.register("FIRE", fire);

        let expr = Expr::compile(
            "Damage.Added{FIRE} * 2.0",
            Some(&tags),
        )
        .unwrap();

        // Pre-populate the synthetic node's value in the context
        let synthetic_name = format!("\0tag:Damage.Added:{}", fire.0);
        let synthetic_id = interner.get_or_intern(&synthetic_name);

        let mut ctx = AttributeContext::new();
        ctx.set(synthetic_id, 25.0); // Damage.Added{FIRE} = 25

        assert_eq!(expr.evaluate(&ctx), 50.0); // 25 * 2.0
    }

    #[test]
    fn tag_query_without_resolver_errors() {
        test_interner();
        // compile (without tags) should error on {
        let result = Expr::compile("Damage{FIRE}", None);
        assert!(result.is_err());
    }

    #[test]
    fn tag_query_unknown_tag_errors() {
        test_interner();
        let tags = TagResolver::new(); // empty — no tags registered
        let result = Expr::compile(
            "Damage{FIRE}",
            Some(&tags),
        );
        assert!(matches!(result, Err(CompileError::UnknownTag(_))));
    }

    // --- Comparison and logical operator tests ---

    #[test]
    fn comparison_greater_than() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("5.0 > 3.0", &ctx), 1.0);
        assert_eq!(eval("3.0 > 5.0", &ctx), 0.0);
        assert_eq!(eval("3.0 > 3.0", &ctx), 0.0);
    }

    #[test]
    fn comparison_less_than() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("3.0 < 5.0", &ctx), 1.0);
        assert_eq!(eval("5.0 < 3.0", &ctx), 0.0);
    }

    #[test]
    fn comparison_greater_equal() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("5.0 >= 3.0", &ctx), 1.0);
        assert_eq!(eval("3.0 >= 3.0", &ctx), 1.0);
        assert_eq!(eval("2.0 >= 3.0", &ctx), 0.0);
    }

    #[test]
    fn comparison_less_equal() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("3.0 <= 5.0", &ctx), 1.0);
        assert_eq!(eval("3.0 <= 3.0", &ctx), 1.0);
        assert_eq!(eval("5.0 <= 3.0", &ctx), 0.0);
    }

    #[test]
    fn comparison_equal() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("3.0 == 3.0", &ctx), 1.0);
        assert_eq!(eval("3.0 == 4.0", &ctx), 0.0);
    }

    #[test]
    fn comparison_not_equal() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("3.0 != 4.0", &ctx), 1.0);
        assert_eq!(eval("3.0 != 3.0", &ctx), 0.0);
    }

    #[test]
    fn logical_and() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("1.0 && 1.0", &ctx), 1.0);
        assert_eq!(eval("1.0 && 0.0", &ctx), 0.0);
        assert_eq!(eval("0.0 && 1.0", &ctx), 0.0);
        assert_eq!(eval("0.0 && 0.0", &ctx), 0.0);
    }

    #[test]
    fn logical_or() {
        test_interner();
        let ctx = AttributeContext::new();
        assert_eq!(eval("1.0 || 1.0", &ctx), 1.0);
        assert_eq!(eval("1.0 || 0.0", &ctx), 1.0);
        assert_eq!(eval("0.0 || 1.0", &ctx), 1.0);
        assert_eq!(eval("0.0 || 0.0", &ctx), 0.0);
    }

    #[test]
    fn comparison_with_attribute() {
        let interner = test_interner();
        let mut ctx = AttributeContext::new();
        let stealth = interner.get_or_intern("Stealth");
        ctx.set(stealth, 50.0);
        assert_eq!(eval("Stealth > 30.0", &ctx), 1.0);
        assert_eq!(eval("Stealth < 30.0", &ctx), 0.0);
    }

    #[test]
    fn compound_logical_expression() {
        let interner = test_interner();
        let mut ctx = AttributeContext::new();
        let stealth = interner.get_or_intern("Stealth");
        let enshad = interner.get_or_intern("Enshadowment");
        ctx.set(stealth, 50.0);
        ctx.set(enshad, 40.0);
        // Both true
        assert_eq!(eval("Stealth > 30.0 && Enshadowment < 90.0", &ctx), 1.0);
        // First true, second false
        ctx.set(enshad, 95.0);
        assert_eq!(eval("Stealth > 30.0 && Enshadowment < 90.0", &ctx), 0.0);
        // OR: one true is enough
        assert_eq!(eval("Stealth > 30.0 || Enshadowment < 90.0", &ctx), 1.0);
    }

    #[test]
    fn comparison_precedence() {
        test_interner();
        let ctx = AttributeContext::new();
        // a + b > c * d  →  (a + b) > (c * d)  →  (2+3) > (1*4)  →  5 > 4  →  1.0
        assert_eq!(eval("2.0 + 3.0 > 1.0 * 4.0", &ctx), 1.0);
    }

    #[test]
    fn logical_precedence() {
        test_interner();
        let ctx = AttributeContext::new();
        // 1 && 0 || 1  →  (1 && 0) || 1  →  0 || 1  →  1.0
        assert_eq!(eval("1.0 && 0.0 || 1.0", &ctx), 1.0);
        // 1 || 0 && 0  →  1 || (0 && 0)  →  1 || 0  →  1.0
        assert_eq!(eval("1.0 || 0.0 && 0.0", &ctx), 1.0);
    }

    #[test]
    fn single_tag_query() {
        test_interner();
        let mut tags = TagResolver::new();
        let physical = TagMask::bit(1);
        tags.register("PHYSICAL", physical);

        let expr = Expr::compile(
            "Damage{PHYSICAL}",
            Some(&tags),
        )
        .unwrap();

        match &expr.dependencies[0] {
            Dependency::TagQuery { mask, .. } => {
                assert_eq!(*mask, physical);
            }
            other => panic!("expected TagQuery, got {:?}", other),
        }
    }

    // --- Cross-entity tagged ref tests ---

    #[test]
    fn cross_entity_tagged_ref_compiles() {
        let interner = test_interner();
        let mut tags = TagResolver::new();
        let fire = TagMask::bit(0);
        tags.register("FIRE", fire);

        let expr = Expr::compile(
            "Damage{FIRE}@weapon * 2.0",
            Some(&tags),
        )
        .unwrap();

        assert_eq!(expr.dependencies.len(), 1);
        match &expr.dependencies[0] {
            Dependency::SourceTagQuery { alias, attribute, mask } => {
                assert_eq!(interner.resolve(*alias), "weapon");
                assert_eq!(interner.resolve(*attribute), "Damage");
                assert_eq!(*mask, fire);
            }
            other => panic!("expected SourceTagQuery, got {:?}", other),
        }
    }

    #[test]
    fn cross_entity_tagged_ref_cache_key_encodes_tag() {
        let interner = test_interner();
        let mut tags = TagResolver::new();
        let fire = TagMask::bit(0);
        tags.register("FIRE", fire);

        let expr = Expr::compile(
            "Damage{FIRE}@weapon",
            Some(&tags),
        )
        .unwrap();

        let entries: Vec<_> = expr.source_cache_keys().collect();
        assert_eq!(entries.len(), 1);
        let (alias, attribute, cache_key, tag_mask) = entries[0];
        assert_eq!(interner.resolve(alias), "weapon");
        assert_eq!(interner.resolve(attribute), "Damage");
        assert_eq!(tag_mask, Some(fire));
        let expected_key = format!("\0tag:Damage@weapon:{}", fire.0);
        assert_eq!(interner.resolve(cache_key), expected_key);
    }

    #[test]
    fn cross_entity_tagged_ref_evaluates() {
        let interner = test_interner();
        let mut tags = TagResolver::new();
        let fire = TagMask::bit(0);
        tags.register("FIRE", fire);

        let expr = Expr::compile(
            "Damage{FIRE}@weapon * 2.0",
            Some(&tags),
        )
        .unwrap();

        let cache_key_str = format!("\0tag:Damage@weapon:{}", fire.0);
        let cache_key_id = interner.get_or_intern(&cache_key_str);

        let mut ctx = AttributeContext::new();
        ctx.set(cache_key_id, 35.0);

        assert_eq!(expr.evaluate(&ctx), 70.0);
    }

    #[test]
    fn cross_entity_tagged_and_untagged_coexist() {
        test_interner();
        let mut tags = TagResolver::new();
        let fire = TagMask::bit(0);
        tags.register("FIRE", fire);

        let expr = Expr::compile(
            "Damage{FIRE}@weapon + Strength@attacker",
            Some(&tags),
        )
        .unwrap();

        assert_eq!(expr.dependencies.len(), 2);
        assert!(matches!(&expr.dependencies[0], Dependency::SourceTagQuery { .. }));
        assert!(matches!(&expr.dependencies[1], Dependency::Source { .. }));

        let entries: Vec<_> = expr.source_cache_keys().collect();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].3.is_some()); // tagged
        assert!(entries[1].3.is_none()); // untagged
    }

    #[test]
    fn cross_entity_multi_tag_ref() {
        test_interner();
        let mut tags = TagResolver::new();
        let fire = TagMask::bit(0);
        let spell = TagMask::bit(3);
        tags.register("FIRE", fire);
        tags.register("SPELL", spell);

        let expr = Expr::compile(
            "Damage{FIRE|SPELL}@weapon",
            Some(&tags),
        )
        .unwrap();

        match &expr.dependencies[0] {
            Dependency::SourceTagQuery { mask, .. } => {
                assert_eq!(*mask, fire | spell);
            }
            other => panic!("expected SourceTagQuery, got {:?}", other),
        }
    }

    #[test]
    fn ambiguous_tag_errors_in_expression() {
        test_interner();
        let mut tags = TagResolver::new();
        tags.register_namespaced("Element", "FIRE", TagMask::bit(0));
        tags.register_namespaced("Weapon", "FIRE", TagMask::bit(4));

        let result = Expr::compile("Damage{FIRE}", Some(&tags));
        assert!(matches!(result, Err(CompileError::AmbiguousTag(_, _))));

        if let Err(CompileError::AmbiguousTag(name, alts)) = result {
            assert_eq!(name, "FIRE");
            assert!(alts.contains(&"ELEMENT::FIRE".to_string()));
            assert!(alts.contains(&"WEAPON::FIRE".to_string()));
        }
    }

    #[test]
    fn namespaced_tag_resolves_in_expression() {
        test_interner();
        let mut tags = TagResolver::new();
        tags.register_namespaced("Element", "FIRE", TagMask::bit(0));
        tags.register_namespaced("Weapon", "FIRE", TagMask::bit(4));

        let expr = Expr::compile(
            "Damage{Element::FIRE}",
            Some(&tags),
        )
        .unwrap();

        match &expr.dependencies[0] {
            Dependency::TagQuery { mask, .. } => {
                assert_eq!(*mask, TagMask::bit(0));
            }
            other => panic!("expected TagQuery, got {:?}", other),
        }
    }
}
