use log::{info, error};
use crate::file_sync::read_file_content;
use crate::project_manager::get_base_path;
use std::fmt;

use crate::modules::renscript_mappings::get_api_method_mappings;

// Comprehensive error types for RenScript compilation
#[derive(Debug, Clone)]
pub enum RenScriptError {
    // File/IO errors
    FileNotFound { path: String },
    
    // Lexer errors
    UnexpectedCharacter { char: char, line: usize, column: usize },
    UnterminatedString { line: usize, column: usize },
    InvalidNumber { text: String, line: usize, column: usize },
    InvalidEscape { escape: String, line: usize, column: usize },
    
    // Parser errors
    InvalidSyntax { message: String, line: usize, column: usize },
    DuplicateProperty { name: String, line: usize, column: usize },
    DuplicateFunction { name: String, line: usize, column: usize },
    
    // Semantic errors
    UndefinedFunction { name: String, line: usize, column: usize, suggestions: Vec<String> },
    
    // Script structure errors
    MissingScriptDeclaration,
    EmptyScript { path: String },
}

impl fmt::Display for RenScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenScriptError::FileNotFound { path } => {
                write!(f, "File not found: {}", path)
            }
            RenScriptError::UnexpectedCharacter { char, line, column } => {
                write!(f, "Unexpected character '{}' at line {}, column {}", char, line, column)
            }
            RenScriptError::UnterminatedString { line, column } => {
                write!(f, "Unterminated string literal at line {}, column {}", line, column)
            }
            RenScriptError::InvalidNumber { text, line, column } => {
                write!(f, "Invalid number '{}' at line {}, column {}", text, line, column)
            }
            RenScriptError::InvalidEscape { escape, line, column } => {
                write!(f, "Invalid escape sequence '{}' at line {}, column {}", escape, line, column)
            }
            RenScriptError::InvalidSyntax { message, line, column } => {
                write!(f, "Syntax error: {} at line {}, column {}", message, line, column)
            }
            RenScriptError::DuplicateProperty { name, line, column } => {
                write!(f, "Duplicate property '{}' at line {}, column {}", name, line, column)
            }
            RenScriptError::DuplicateFunction { name, line, column } => {
                write!(f, "Duplicate function '{}' at line {}, column {}", name, line, column)
            }
            RenScriptError::UndefinedFunction { name, line, column, suggestions } => {
                let suggestion_text = if suggestions.is_empty() {
                    String::new()
                } else {
                    format!(" Did you mean: {}?", suggestions.join(", "))
                };
                write!(f, "Undefined function '{}' at line {}, column {}.{}", name, line, column, suggestion_text)
            }
            RenScriptError::MissingScriptDeclaration => {
                write!(f, "Missing script declaration. RenScript files must start with 'script ScriptName {{'")
            }
            RenScriptError::EmptyScript { path } => {
                write!(f, "Empty script file: {}", path)
            }
        }
    }
}

impl std::error::Error for RenScriptError {}

// Helper function to convert legacy string errors to RenScriptError
impl From<String> for RenScriptError {
    fn from(message: String) -> Self {
        RenScriptError::InvalidSyntax { 
            message, 
            line: 0, 
            column: 0 
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Literals
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    
    // Identifiers and Keywords
    Identifier(String),
    Script,
    Props,
    Start,
    Update,
    Destroy,
    Once,
    If,
    Else,
    While,
    For,
    Return,
    Break,
    Switch,
    Case,
    Default,
    ObjectType(String), // mesh, camera, light, scene, transform
    
    // Operators
    Assign,
    Plus,
    Minus,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    LogicalAnd,
    LogicalOr,
    LogicalNot,
    PlusPlus,
    
    // Delimiters
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Semicolon,
    Colon,
    Question,
    
    // Special
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub line: usize,
    pub column: usize,
}

pub struct RenScriptLexer {
    source: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
}

impl RenScriptLexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
        }
    }
    
    pub fn tokenize(&mut self) -> Result<Vec<Token>, RenScriptError> {
        let mut tokens = Vec::new();
        
        while self.position < self.source.len() {
            self.skip_whitespace();
            if self.position >= self.source.len() {
                break;
            }
            
            let ch = self.current_char();
            
            // Comments
            if ch == '#' || (ch == '/' && self.peek() == Some('/')) {
                self.skip_comment();
                continue;
            }
            
            // String literals
            if ch == '"' || ch == '\'' {
                let string_val = self.read_string()?;
                tokens.push(Token {
                    token_type: TokenType::String(string_val),
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            // Numbers
            if ch.is_ascii_digit() {
                let number_val = self.read_number()?;
                tokens.push(Token {
                    token_type: TokenType::Number(number_val),
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            // Identifiers and keywords
            if ch.is_alphabetic() || ch == '_' {
                let identifier = self.read_identifier();
                let token_type = self.keyword_or_identifier(&identifier);
                tokens.push(Token {
                    token_type,
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            // Multi-character operators
            if ch == '&' && self.peek() == Some('&') {
                self.advance();
                self.advance();
                tokens.push(Token {
                    token_type: TokenType::LogicalAnd,
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            if ch == '|' && self.peek() == Some('|') {
                self.advance();
                self.advance();
                tokens.push(Token {
                    token_type: TokenType::LogicalOr,
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            if ch == '!' && self.peek() == Some('=') {
                self.advance();
                self.advance();
                tokens.push(Token {
                    token_type: TokenType::NotEqual,
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            if ch == '=' && self.peek() == Some('=') {
                self.advance();
                self.advance();
                tokens.push(Token {
                    token_type: TokenType::Equal,
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            if ch == '<' && self.peek() == Some('=') {
                self.advance();
                self.advance();
                tokens.push(Token {
                    token_type: TokenType::LessEqual,
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            if ch == '>' && self.peek() == Some('=') {
                self.advance();
                self.advance();
                tokens.push(Token {
                    token_type: TokenType::GreaterEqual,
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            if ch == '+' && self.peek() == Some('+') {
                self.advance();
                self.advance();
                tokens.push(Token {
                    token_type: TokenType::PlusPlus,
                    line: self.line,
                    column: self.column,
                });
                continue;
            }
            
            // Single character tokens
            let token_type = match ch {
                '{' => TokenType::LeftBrace,
                '}' => TokenType::RightBrace,
                '(' => TokenType::LeftParen,
                ')' => TokenType::RightParen,
                '[' => TokenType::LeftBracket,
                ']' => TokenType::RightBracket,
                ',' => TokenType::Comma,
                '.' => TokenType::Dot,
                ';' => TokenType::Semicolon,
                ':' => TokenType::Colon,
                '?' => TokenType::Question,
                '=' => TokenType::Assign,
                '+' => TokenType::Plus,
                '-' => TokenType::Minus,
                '*' => TokenType::Multiply,
                '/' => TokenType::Divide,
                '<' => TokenType::Less,
                '>' => TokenType::Greater,
                '!' => TokenType::LogicalNot,
                '&' => TokenType::LogicalAnd,
                '|' => TokenType::LogicalOr,
                _ => return Err(RenScriptError::UnexpectedCharacter { 
                    char: ch, 
                    line: self.line, 
                    column: self.column 
                }),
            };
            
            tokens.push(Token {
                token_type,
                line: self.line,
                column: self.column,
            });
            self.advance();
        }
        
        tokens.push(Token {
            token_type: TokenType::Eof,
            line: self.line,
            column: self.column,
        });
        
        Ok(tokens)
    }
    
    fn current_char(&self) -> char {
        self.source[self.position]
    }
    
    fn peek(&self) -> Option<char> {
        if self.position + 1 < self.source.len() {
            Some(self.source[self.position + 1])
        } else {
            None
        }
    }
    
    fn advance(&mut self) {
        if self.position < self.source.len() {
            if self.source[self.position] == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            self.position += 1;
        }
    }
    
    fn skip_whitespace(&mut self) {
        while self.position < self.source.len() && self.current_char().is_whitespace() {
            self.advance();
        }
    }
    
    fn skip_comment(&mut self) {
        while self.position < self.source.len() && self.current_char() != '\n' {
            self.advance();
        }
    }
    
    fn read_string(&mut self) -> Result<String, RenScriptError> {
        let quote_char = self.current_char();
        let start_line = self.line;
        let start_column = self.column;
        self.advance();
        
        let mut value = String::new();
        while self.position < self.source.len() && self.current_char() != quote_char {
            if self.current_char() == '\\' {
                self.advance();
                if self.position < self.source.len() {
                    match self.current_char() {
                        'n' => value.push('\n'),
                        't' => value.push('\t'),
                        'r' => value.push('\r'),
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        '\'' => value.push('\''),
                        escape_char => {
                            return Err(RenScriptError::InvalidEscape { 
                                escape: format!("\\{}", escape_char), 
                                line: self.line, 
                                column: self.column 
                            });
                        }
                    }
                } else {
                    return Err(RenScriptError::UnterminatedString { 
                        line: start_line, 
                        column: start_column 
                    });
                }
            } else {
                value.push(self.current_char());
            }
            self.advance();
        }
        
        if self.position >= self.source.len() {
            return Err(RenScriptError::UnterminatedString { 
                line: start_line, 
                column: start_column 
            });
        }
        
        self.advance(); // Skip closing quote
        Ok(value)
    }
    
    fn read_number(&mut self) -> Result<f64, RenScriptError> {
        let mut value = String::new();
        let start_line = self.line;
        let start_column = self.column;
        let mut has_dot = false;
        
        while self.position < self.source.len() {
            let ch = self.current_char();
            if ch.is_ascii_digit() {
                value.push(ch);
                self.advance();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        
        value.parse::<f64>().map_err(|_| RenScriptError::InvalidNumber { 
            text: value, 
            line: start_line, 
            column: start_column 
        })
    }
    
    fn read_identifier(&mut self) -> String {
        let mut value = String::new();
        
        while self.position < self.source.len() && (self.current_char().is_alphanumeric() || self.current_char() == '_') {
            value.push(self.current_char());
            self.advance();
        }
        
        value
    }
    
    fn keyword_or_identifier(&self, identifier: &str) -> TokenType {
        match identifier {
            "script" => TokenType::Script,
            "props" => TokenType::Props,
            "start" => TokenType::Start,
            "update" => TokenType::Update,
            "destroy" => TokenType::Destroy,
            "once" => TokenType::Once,
            "if" => TokenType::If,
            "else" => TokenType::Else,
            "while" => TokenType::While,
            "for" => TokenType::For,
            "return" => TokenType::Return,
            "break" => TokenType::Break,
            "switch" => TokenType::Switch,
            "case" => TokenType::Case,
            "default" => TokenType::Default,
            "true" => TokenType::Boolean(true),
            "false" => TokenType::Boolean(false),
            "null" => TokenType::Null,
            "mesh" | "camera" | "light" | "scene" | "transform" => TokenType::ObjectType(identifier.to_string()),
            _ => TokenType::Identifier(identifier.to_string()),
        }
    }
}

pub fn compile_renscript(script_name: &str) -> Result<String, String> {
    match compile_renscript_internal(script_name) {
        Ok(result) => Ok(result),
        Err(e) => Err(e.to_string())
    }
}

fn compile_renscript_internal(script_path_or_name: &str) -> Result<String, RenScriptError> {
    info!("📜 Compiling RenScript: {}", script_path_or_name);
    
    let (ren_path_str, renp_path_str, script_name) = if script_path_or_name.ends_with(".ren") {
        // Full path provided - extract script name and derive renp path
        let script_name = std::path::Path::new(script_path_or_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let renp_path_str = script_path_or_name.replace(".ren", ".renp");
        info!("🔍 Using provided full path: {}", script_path_or_name);
        (script_path_or_name.to_string(), renp_path_str, script_name)
    } else {
        // Script name provided - construct paths as before
        let current_dir = std::env::current_dir().unwrap_or_else(|_| get_base_path());
        let engine_root = if current_dir.file_name().and_then(|n| n.to_str()) == Some("bridge") {
            current_dir.parent().unwrap_or(&current_dir).to_path_buf()
        } else {
            current_dir
        };
        
        // Try subdirectory structure first: renscripts/{script_name}/{script_name}.ren
        let subdir_ren_path = engine_root.join("renscripts").join(&script_path_or_name).join(format!("{}.ren", script_path_or_name));
        let subdir_renp_path = engine_root.join("renscripts").join(&script_path_or_name).join(format!("{}.renp", script_path_or_name));
        
        // Fallback to old structure: renscripts/{script_name}.ren  
        let old_ren_path = engine_root.join("renscripts").join(format!("{}.ren", script_path_or_name));
        let old_renp_path = engine_root.join("renscripts").join(format!("{}.renp", script_path_or_name));
        
        // Determine which paths to use
        let (ren_path, renp_path) = if subdir_ren_path.exists() {
            info!("🔍 Found script in subdirectory: {}", subdir_ren_path.display());
            (subdir_ren_path, subdir_renp_path)
        } else {
            info!("🔍 Looking for script in root: {}", old_ren_path.display());
            (old_ren_path, old_renp_path)
        };
        
        (ren_path.to_string_lossy().to_string(), renp_path.to_string_lossy().to_string(), script_path_or_name.to_string())
    };
    
    // Read the main .ren file
    let ren_content = match read_file_content(&ren_path_str) {
        Ok(content) => {
            if content.trim().is_empty() {
                error!("❌ Script file is empty: {}", ren_path_str);
                return Err(RenScriptError::EmptyScript { path: ren_path_str.to_string() });
            }
            content
        }
        Err(e) => {
            error!("❌ Failed to read .ren file '{}': {}", ren_path_str, e);
            return Err(RenScriptError::FileNotFound { path: ren_path_str.to_string() });
        }
    };
    
    // Try to read the optional .renp file and merge it with the .ren content
    let merged_content = if let Ok(renp_content) = read_file_content(&renp_path_str) {
        if !renp_content.trim().is_empty() {
            info!("✅ Found .renp file, merging with .ren file for: {}", script_name);
            
            // Simple merge: props first, then script
            let merged = format!("{}\n\n{}", renp_content, ren_content);
            info!("✅ Properties merged successfully - external props + script structure");
            info!("📊 Merged content length: {} characters, {} lines", merged.len(), merged.lines().count());
            
            // Debug output disabled
            merged
        } else {
            ren_content
        }
    } else {
        info!("ℹ️ No .renp file found for '{}' (optional)", script_name);
        ren_content
    };
    
    // Step 1: Tokenize
    let mut lexer = RenScriptLexer::new(&merged_content);
    let tokens = match lexer.tokenize() {
        Ok(tokens) => tokens,
        Err(e) => {
            error!("❌ Tokenization failed: {}", e);
            return Err(e);
        }
    };
    
    info!("✅ Tokenized {} tokens", tokens.len());
    
    // TODO: Step 2: Parse tokens into AST
    // TODO: Step 3: Generate JavaScript code
    
    // For now, return a simple placeholder with token count
    let _compiled_js = format!(
        "// Compiled from {}.ren + {}.renp\n// Tokenized {} tokens successfully\n\nfunction createRenScript(scene, api) {{\n  return {{\n    // TODO: Generated script methods will go here\n  }};\n}}", 
        script_name, script_name, tokens.len()
    );
    
    // Step 2: Parse tokens into AST
    let mut parser = RenScriptParser::new(tokens);
    let ast = match parser.parse() {
        Ok(ast) => ast,
        Err(e) => {
            error!("❌ Parsing failed: {}", e);
            return Err(e);
        }
    };
    
    info!("✅ AST generated successfully for script '{}'", ast.name);
    
    // Step 3: Validate API functions and generate JavaScript code
    let code_generator = RenScriptCodeGenerator::new(&ast);
    
    // Validate API functions before generating code
    let used_functions = code_generator.analyze_used_functions();
    if let Err(e) = code_generator.validate_api_functions(&used_functions) {
        error!("❌ API validation failed: {}", e);
        return Err(e);
    }
    
    let compiled_js = code_generator.generate();
    
    info!("✅ JavaScript code generated successfully ({} chars)", compiled_js.len());
    
    Ok(compiled_js)
}

// AST Node types
#[derive(Debug, Clone)]
pub struct ScriptAst {
    pub name: String,
    pub object_type: String,
    pub variables: Vec<VariableDeclaration>,
    pub methods: Vec<MethodDeclaration>,
    pub properties: Vec<PropertyDeclaration>,
    pub functions: Vec<FunctionDeclaration>,
}

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub name: String,
    pub value: Expression,
}

#[derive(Debug, Clone)]
pub struct MethodDeclaration {
    pub method_type: String, // start, update, destroy, once
    pub parameters: Vec<String>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct FunctionDeclaration {
    pub name: String,
    pub parameters: Vec<String>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct PropertyDeclaration {
    pub section: String,
    pub name: String,
    pub prop_type: String, // boolean, range, select
    pub default_value: Option<Expression>,
    pub min: Option<Expression>,
    pub max: Option<Expression>,
    pub options: Option<Vec<String>>,
    pub description: Option<String>,
    pub once: bool,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assignment { name: String, value: Expression },
    Expression(Expression),
    If { condition: Expression, then_statements: Vec<Statement>, else_statements: Option<Vec<Statement>> },
    For { init: Box<Statement>, condition: Expression, update: Box<Statement>, statements: Vec<Statement> },
    Return { value: Option<Expression> },
    Break,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Literal(LiteralValue),
    Identifier(String),
    Binary { left: Box<Expression>, operator: String, right: Box<Expression> },
    Unary { operator: String, operand: Box<Expression> },
    Call { callee: Box<Expression>, arguments: Vec<Expression> },
    Member { object: Box<Expression>, property: Box<Expression>, computed: bool },
    Array(Vec<Expression>),
    Object(Vec<(String, Expression)>),
}

#[derive(Debug, Clone)]
pub enum LiteralValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

pub struct RenScriptParser {
    tokens: Vec<Token>,
    current: usize,
}

impl RenScriptParser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }
    
    
    // Suggest similar function names using edit distance
    fn suggest_similar_functions_static(target: &str, available: &[String]) -> Vec<String> {
        let mut suggestions = Vec::new();
        let target_lower = target.to_lowercase();
        
        for func in available {
            let func_lower = func.to_lowercase();
            
            // Exact case-insensitive match
            if func_lower == target_lower {
                suggestions.push(func.clone());
                continue;
            }
            
            // Check if target is contained in function name
            if func_lower.contains(&target_lower) || target_lower.contains(&func_lower) {
                suggestions.push(func.clone());
                continue;
            }
            
            // Simple edit distance for similar names
            if Self::edit_distance_static(&target_lower, &func_lower) <= 3 {
                suggestions.push(func.clone());
            }
        }
        
        suggestions.sort();
        suggestions.truncate(5); // Limit to 5 suggestions
        suggestions
    }
    
    // Simple edit distance calculation
    fn edit_distance_static(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();
        
        if a_len == 0 { return b_len; }
        if b_len == 0 { return a_len; }
        
        let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];
        
        for i in 0..=a_len { matrix[i][0] = i; }
        for j in 0..=b_len { matrix[0][j] = j; }
        
        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i-1] == b_chars[j-1] { 0 } else { 1 };
                matrix[i][j] = std::cmp::min(
                    std::cmp::min(matrix[i-1][j] + 1, matrix[i][j-1] + 1),
                    matrix[i-1][j-1] + cost
                );
            }
        }
        
        matrix[a_len][b_len]
    }
    
    pub fn parse(&mut self) -> Result<ScriptAst, RenScriptError> {
        self.parse_file()
    }
    
    // Parse file structure: external props + script block
    fn parse_file(&mut self) -> Result<ScriptAst, RenScriptError> {
        let mut external_properties = Vec::new();
        let mut property_names = std::collections::HashSet::new();
        
        // Parse external props sections first
        while self.check(&TokenType::Props) && !self.is_at_end() {
            let new_props = self.parse_props_declaration()?;
            // Check for duplicate properties
            for prop in &new_props {
                if property_names.contains(&prop.name) {
                    return Err(RenScriptError::DuplicateProperty { 
                        name: prop.name.clone(), 
                        line: self.peek()?.line, 
                        column: self.peek()?.column 
                    });
                }
                property_names.insert(prop.name.clone());
            }
            external_properties.extend(new_props);
        }
        
        // Now parse the script block
        let mut script_ast = self.parse_script()?;
        
        // Merge external properties with script properties
        script_ast.properties.extend(external_properties);
        
        Ok(script_ast)
    }
    
    fn parse_script(&mut self) -> Result<ScriptAst, RenScriptError> {
        // Handle both old syntax (script Name) and new syntax (camera Name, light Name, etc.)
        let mut object_type = "script".to_string();
        let name;
        
        if self.check(&TokenType::Script) {
            self.advance(); // consume 'script'
            name = self.consume_identifier("Expected script name")?;
        } else if let TokenType::ObjectType(obj_type) = &self.peek()?.token_type {
            object_type = obj_type.clone();
            self.advance(); // consume object type
            name = self.consume_identifier("Expected object name")?;
        } else {
            return Err(RenScriptError::MissingScriptDeclaration);
        }
        
        self.consume(&TokenType::LeftBrace, "Expected '{'")?;
        
        let mut variables = Vec::new();
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut functions = Vec::new();
        
        // Track names for duplicate detection
        let mut function_names = std::collections::HashSet::new();
        let mut property_names = std::collections::HashSet::new();
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            if self.check(&TokenType::Props) {
                let new_props = self.parse_props_declaration()?;
                // Check for duplicate properties
                for prop in &new_props {
                    if property_names.contains(&prop.name) {
                        return Err(RenScriptError::DuplicateProperty { 
                            name: prop.name.clone(), 
                            line: self.peek()?.line, 
                            column: self.peek()?.column 
                        });
                    }
                    property_names.insert(prop.name.clone());
                }
                properties.extend(new_props);
            } else if let TokenType::Identifier(_) = &self.peek()?.token_type {
                if self.peek_next().map(|t| &t.token_type) == Some(&TokenType::Assign) {
                    variables.push(self.parse_variable_declaration()?);
                } else if self.peek_next().map(|t| &t.token_type) == Some(&TokenType::LeftParen) {
                    let func = self.parse_function_declaration()?;
                    // Check for duplicate functions
                    if function_names.contains(&func.name) {
                        return Err(RenScriptError::DuplicateFunction { 
                            name: func.name.clone(), 
                            line: self.peek()?.line, 
                            column: self.peek()?.column 
                        });
                    }
                    function_names.insert(func.name.clone());
                    functions.push(func);
                } else {
                    let token = self.peek()?;
                    return Err(RenScriptError::InvalidSyntax { 
                        message: "Unexpected identifier".to_string(), 
                        line: token.line, 
                        column: token.column 
                    });
                }
            } else if matches!(&self.peek()?.token_type, TokenType::Start | TokenType::Update | TokenType::Destroy | TokenType::Once) {
                methods.push(self.parse_method_declaration()?);
            } else {
                let token = self.peek()?;
                return Err(RenScriptError::InvalidSyntax { 
                    message: format!("Unexpected token: {:?}", token.token_type), 
                    line: token.line, 
                    column: token.column 
                });
            }
        }
        
        self.consume(&TokenType::RightBrace, "Expected '}'")?;
        
        Ok(ScriptAst {
            name,
            object_type,
            variables,
            methods,
            properties,
            functions,
        })
    }
    
    fn parse_variable_declaration(&mut self) -> Result<VariableDeclaration, String> {
        let name = self.consume_identifier("Expected variable name")?;
        self.consume(&TokenType::Assign, "Expected '='")?;
        let value = self.parse_expression()?;
        Ok(VariableDeclaration { name, value })
    }
    
    fn parse_method_declaration(&mut self) -> Result<MethodDeclaration, String> {
        let method_type = match &self.advance().token_type {
            TokenType::Start => "start".to_string(),
            TokenType::Update => "update".to_string(),
            TokenType::Destroy => "destroy".to_string(),
            TokenType::Once => "once".to_string(),
            _ => return Err("Expected method type".to_string()),
        };
        
        let mut parameters = Vec::new();
        if self.check(&TokenType::LeftParen) {
            self.advance(); // consume '('
            while !self.check(&TokenType::RightParen) && !self.is_at_end() {
                parameters.push(self.consume_identifier("Expected parameter name")?);
                if self.check(&TokenType::Comma) {
                    self.advance();
                }
            }
            self.consume(&TokenType::RightParen, "Expected ')'")?;
        }
        
        self.consume(&TokenType::LeftBrace, "Expected '{'")?;
        let statements = self.parse_statements()?;
        self.consume(&TokenType::RightBrace, "Expected '}'")?;
        
        Ok(MethodDeclaration {
            method_type,
            parameters,
            statements,
        })
    }
    
    fn parse_function_declaration(&mut self) -> Result<FunctionDeclaration, String> {
        let name = self.consume_identifier("Expected function name")?;
        self.consume(&TokenType::LeftParen, "Expected '('")?;
        
        let mut parameters = Vec::new();
        while !self.check(&TokenType::RightParen) && !self.is_at_end() {
            parameters.push(self.consume_identifier("Expected parameter name")?);
            if self.check(&TokenType::Comma) {
                self.advance();
            }
        }
        self.consume(&TokenType::RightParen, "Expected ')'")?;
        self.consume(&TokenType::LeftBrace, "Expected '{'")?;
        let statements = self.parse_statements()?;
        self.consume(&TokenType::RightBrace, "Expected '}'")?;
        
        Ok(FunctionDeclaration {
            name,
            parameters,
            statements,
        })
    }
    
    fn parse_props_declaration(&mut self) -> Result<Vec<PropertyDeclaration>, String> {
        self.advance(); // consume 'props'
        
        // Check if there's a section name (optional)
        let section = if self.check(&TokenType::LeftBrace) {
            "default".to_string() // No section name, use default
        } else {
            self.consume_identifier("Expected props section name")?
        };
        
        self.consume(&TokenType::LeftBrace, "Expected '{'")?;
        
        let mut properties = Vec::new();
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            properties.push(self.parse_property_declaration(&section)?);
        }
        
        self.consume(&TokenType::RightBrace, "Expected '}'")?;
        Ok(properties)
    }
    
    fn parse_property_declaration(&mut self, section: &str) -> Result<PropertyDeclaration, String> {
        let name = self.consume_identifier("Expected property name")?;
        self.consume(&TokenType::Colon, "Expected ':'")?;
        let prop_type = self.consume_identifier("Expected property type")?;
        self.consume(&TokenType::LeftBrace, "Expected '{'")?;
        
        let mut default_value = None;
        let mut min = None;
        let mut max = None;
        let mut options = None;
        let mut description = None;
        let mut once = false;
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            let key = match &self.tokens[self.current].token_type {
                TokenType::Default => {
                    self.advance();
                    "default".to_string()
                },
                TokenType::Once => {
                    self.advance();
                    "once".to_string()
                },
                _ => self.consume_identifier("Expected property key")?
            };
            self.consume(&TokenType::Colon, "Expected ':'")?;
            
            match key.as_str() {
                "default" => default_value = Some(self.parse_expression()?),
                "min" => min = Some(self.parse_expression()?),
                "max" => max = Some(self.parse_expression()?),
                "description" => {
                    if let Expression::Literal(LiteralValue::String(desc)) = self.parse_expression()? {
                        description = Some(desc);
                    }
                },
                "once" => {
                    if let Expression::Literal(LiteralValue::Boolean(val)) = self.parse_expression()? {
                        once = val;
                    }
                },
                "options" => {
                    if let Expression::Array(items) = self.parse_expression()? {
                        let string_options: Result<Vec<String>, String> = items.into_iter().map(|expr| {
                            if let Expression::Literal(LiteralValue::String(s)) = expr {
                                Ok(s)
                            } else {
                                Err("Options must be strings".to_string())
                            }
                        }).collect();
                        options = Some(string_options?);
                    }
                },
                _ => {
                    // Skip unknown property keys by consuming the value
                    let _ = self.parse_expression()?;
                }
            }
            
            // Properties can be separated by commas OR newlines - be flexible
            if self.check(&TokenType::Comma) {
                self.advance();
            }
        }
        
        self.consume(&TokenType::RightBrace, "Expected '}'")?;
        
        Ok(PropertyDeclaration {
            section: section.to_string(),
            name,
            prop_type,
            default_value,
            min,
            max,
            options,
            description,
            once,
        })
    }
    
    fn parse_statements(&mut self) -> Result<Vec<Statement>, String> {
        let mut statements = Vec::new();
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }
        
        Ok(statements)
    }
    
    fn parse_statement(&mut self) -> Result<Statement, String> {
        if self.check(&TokenType::If) {
            self.parse_if_statement()
        } else if self.check(&TokenType::For) {
            self.parse_for_statement()
        } else if self.check(&TokenType::Return) {
            self.parse_return_statement()
        } else if self.check(&TokenType::Break) {
            self.advance();
            Ok(Statement::Break)
        } else if let TokenType::Identifier(_) = &self.peek()?.token_type {
            if self.peek_next().map(|t| &t.token_type) == Some(&TokenType::Assign) {
                let name = self.consume_identifier("Expected variable name")?;
                self.consume(&TokenType::Assign, "Expected '='")?;
                let value = self.parse_expression()?;
                Ok(Statement::Assignment { name, value })
            } else if self.peek_next().map(|t| &t.token_type) == Some(&TokenType::PlusPlus) {
                // Handle i++ as i = i + 1
                let name = self.consume_identifier("Expected variable name")?;
                self.advance(); // consume ++
                Ok(Statement::Assignment {
                    name: name.clone(),
                    value: Expression::Binary {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "+".to_string(),
                        right: Box::new(Expression::Literal(LiteralValue::Number(1.0)))
                    }
                })
            } else {
                // Expression statement
                let expr = self.parse_expression()?;
                Ok(Statement::Expression(expr))
            }
        } else {
            let expr = self.parse_expression()?;
            Ok(Statement::Expression(expr))
        }
    }
    
    fn parse_if_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // consume 'if'
        self.consume(&TokenType::LeftParen, "Expected '('")?;
        let condition = self.parse_expression()?;
        self.consume(&TokenType::RightParen, "Expected ')'")?;
        self.consume(&TokenType::LeftBrace, "Expected '{'")?;
        let then_statements = self.parse_statements()?;
        self.consume(&TokenType::RightBrace, "Expected '}'")?;
        
        let else_statements = if self.check(&TokenType::Else) {
            self.advance(); // consume 'else'
            if self.check(&TokenType::If) {
                // else if - parse as another if statement
                Some(vec![self.parse_if_statement()?])
            } else {
                // else block
                self.consume(&TokenType::LeftBrace, "Expected '{'")?;
                let stmts = self.parse_statements()?;
                self.consume(&TokenType::RightBrace, "Expected '}'")?;
                Some(stmts)
            }
        } else {
            None
        };
        
        Ok(Statement::If { condition, then_statements, else_statements })
    }
    
    fn parse_for_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // consume 'for'
        self.consume(&TokenType::LeftParen, "Expected '('")?;
        let init = Box::new(self.parse_statement()?);
        self.consume(&TokenType::Semicolon, "Expected ';'")?;
        let condition = self.parse_expression()?;
        self.consume(&TokenType::Semicolon, "Expected ';'")?;
        
        // Handle increment expressions like i++ or regular assignments
        let update = if let TokenType::Identifier(name) = &self.peek()?.token_type.clone() {
            if self.peek_next().map(|t| &t.token_type) == Some(&TokenType::PlusPlus) {
                // Handle i++ as i = i + 1
                let var_name = name.clone();
                self.advance(); // consume identifier
                self.advance(); // consume ++
                Box::new(Statement::Assignment {
                    name: var_name.clone(),
                    value: Expression::Binary {
                        left: Box::new(Expression::Identifier(var_name)),
                        operator: "+".to_string(),
                        right: Box::new(Expression::Literal(LiteralValue::Number(1.0)))
                    }
                })
            } else {
                Box::new(self.parse_statement()?)
            }
        } else {
            Box::new(self.parse_statement()?)
        };
        
        self.consume(&TokenType::RightParen, "Expected ')'")?;
        self.consume(&TokenType::LeftBrace, "Expected '{'")?;
        let statements = self.parse_statements()?;
        self.consume(&TokenType::RightBrace, "Expected '}'")?;
        
        Ok(Statement::For { init, condition, update, statements })
    }
    
    fn parse_return_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // consume 'return'
        let value = if self.check(&TokenType::RightBrace) || self.check(&TokenType::Eof) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        Ok(Statement::Return { value })
    }
    
    fn parse_expression(&mut self) -> Result<Expression, String> {
        self.parse_logical_or()
    }
    
    fn parse_logical_or(&mut self) -> Result<Expression, String> {
        let mut expr = self.parse_logical_and()?;
        
        while self.check(&TokenType::LogicalOr) {
            let operator = "||".to_string();
            self.advance();
            let right = self.parse_logical_and()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(expr)
    }
    
    fn parse_logical_and(&mut self) -> Result<Expression, String> {
        let mut expr = self.parse_equality()?;
        
        while self.check(&TokenType::LogicalAnd) {
            let operator = "&&".to_string();
            self.advance();
            let right = self.parse_equality()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(expr)
    }
    
    fn parse_equality(&mut self) -> Result<Expression, String> {
        let mut expr = self.parse_comparison()?;
        
        while matches!(&self.peek()?.token_type, TokenType::Equal | TokenType::NotEqual) {
            let operator = match &self.advance().token_type {
                TokenType::Equal => "==".to_string(),
                TokenType::NotEqual => "!=".to_string(),
                _ => unreachable!(),
            };
            let right = self.parse_comparison()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(expr)
    }
    
    fn parse_comparison(&mut self) -> Result<Expression, String> {
        let mut expr = self.parse_term()?;
        
        while matches!(&self.peek()?.token_type, TokenType::Greater | TokenType::GreaterEqual | TokenType::Less | TokenType::LessEqual) {
            let operator = match &self.advance().token_type {
                TokenType::Greater => ">".to_string(),
                TokenType::GreaterEqual => ">=".to_string(),
                TokenType::Less => "<".to_string(),
                TokenType::LessEqual => "<=".to_string(),
                _ => unreachable!(),
            };
            let right = self.parse_term()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(expr)
    }
    
    fn parse_term(&mut self) -> Result<Expression, String> {
        let mut expr = self.parse_factor()?;
        
        while matches!(&self.peek()?.token_type, TokenType::Minus | TokenType::Plus) {
            let operator = match &self.advance().token_type {
                TokenType::Minus => "-".to_string(),
                TokenType::Plus => "+".to_string(),
                _ => unreachable!(),
            };
            let right = self.parse_factor()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(expr)
    }
    
    fn parse_factor(&mut self) -> Result<Expression, String> {
        let mut expr = self.parse_unary()?;
        
        while matches!(&self.peek()?.token_type, TokenType::Divide | TokenType::Multiply) {
            let operator = match &self.advance().token_type {
                TokenType::Divide => "/".to_string(),
                TokenType::Multiply => "*".to_string(),
                _ => unreachable!(),
            };
            let right = self.parse_unary()?;
            expr = Expression::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(expr)
    }
    
    fn parse_unary(&mut self) -> Result<Expression, String> {
        if matches!(&self.peek()?.token_type, TokenType::LogicalNot | TokenType::Minus) {
            let operator = match &self.advance().token_type {
                TokenType::LogicalNot => "!".to_string(),
                TokenType::Minus => "-".to_string(),
                _ => unreachable!(),
            };
            let operand = self.parse_unary()?;
            return Ok(Expression::Unary {
                operator,
                operand: Box::new(operand),
            });
        }
        
        self.parse_call()
    }
    
    fn parse_call(&mut self) -> Result<Expression, String> {
        let mut expr = self.parse_primary()?;
        
        loop {
            if self.check(&TokenType::LeftParen) {
                self.advance(); // consume '('
                let mut arguments = Vec::new();
                while !self.check(&TokenType::RightParen) && !self.is_at_end() {
                    arguments.push(self.parse_expression()?);
                    if self.check(&TokenType::Comma) {
                        self.advance();
                    }
                }
                self.consume(&TokenType::RightParen, "Expected ')'")?;
                expr = Expression::Call {
                    callee: Box::new(expr),
                    arguments,
                };
            } else if self.check(&TokenType::Dot) {
                self.advance(); // consume '.'
                let property = self.consume_identifier("Expected property name")?;
                expr = Expression::Member {
                    object: Box::new(expr),
                    property: Box::new(Expression::Identifier(property)),
                    computed: false,
                };
            } else if self.check(&TokenType::LeftBracket) {
                self.advance(); // consume '['
                let property = self.parse_expression()?;
                self.consume(&TokenType::RightBracket, "Expected ']'")?;
                expr = Expression::Member {
                    object: Box::new(expr),
                    property: Box::new(property),
                    computed: true,
                };
            } else {
                break;
            }
        }
        
        Ok(expr)
    }
    
    fn parse_primary(&mut self) -> Result<Expression, String> {
        let token = self.advance();
        
        match &token.token_type {
            TokenType::Boolean(val) => Ok(Expression::Literal(LiteralValue::Boolean(*val))),
            TokenType::Null => Ok(Expression::Literal(LiteralValue::Null)),
            TokenType::Number(val) => Ok(Expression::Literal(LiteralValue::Number(*val))),
            TokenType::String(val) => Ok(Expression::Literal(LiteralValue::String(val.clone()))),
            TokenType::Identifier(name) => Ok(Expression::Identifier(name.clone())),
            TokenType::LeftParen => {
                let expr = self.parse_expression()?;
                self.consume(&TokenType::RightParen, "Expected ')'")?;
                Ok(expr)
            },
            TokenType::LeftBracket => {
                let mut elements = Vec::new();
                while !self.check(&TokenType::RightBracket) && !self.is_at_end() {
                    elements.push(self.parse_expression()?);
                    if self.check(&TokenType::Comma) {
                        self.advance();
                    }
                }
                self.consume(&TokenType::RightBracket, "Expected ']'")?;
                Ok(Expression::Array(elements))
            },
            TokenType::LeftBrace => {
                // Parse object literal {key: value, key2: value2}
                let mut properties = Vec::new();
                while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
                    // Parse key (identifier or string)
                    let key = match &self.advance().token_type {
                        TokenType::Identifier(name) => name.clone(),
                        TokenType::String(s) => s.clone(),
                        _ => return Err("Expected property name in object literal".to_string()),
                    };
                    
                    self.consume(&TokenType::Colon, "Expected ':' after object property name")?;
                    let value = self.parse_expression()?;
                    properties.push((key, value));
                    
                    if self.check(&TokenType::Comma) {
                        self.advance();
                    } else if !self.check(&TokenType::RightBrace) {
                        return Err("Expected ',' or '}' in object literal".to_string());
                    }
                }
                self.consume(&TokenType::RightBrace, "Expected '}'")?;
                Ok(Expression::Object(properties))
            },
            _ => Err(format!("Unexpected token {:?} at line {}", token.token_type, token.line)),
        }
    }
    
    // Helper methods
    fn peek(&self) -> Result<&Token, String> {
        self.tokens.get(self.current).ok_or("Unexpected end of input".to_string())
    }
    
    fn peek_next(&self) -> Option<&Token> {
        self.tokens.get(self.current + 1)
    }
    
    
    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        &self.tokens[self.current - 1]
    }
    
    fn check(&self, token_type: &TokenType) -> bool {
        if let Ok(token) = self.peek() {
            std::mem::discriminant(&token.token_type) == std::mem::discriminant(token_type)
        } else {
            false
        }
    }
    
    fn consume(&mut self, expected: &TokenType, message: &str) -> Result<&Token, String> {
        if self.check(expected) {
            Ok(self.advance())
        } else {
            Err(format!("{} at line {}", message, self.peek().map_or(0, |t| t.line)))
        }
    }
    
    fn consume_identifier(&mut self, message: &str) -> Result<String, String> {
        if let TokenType::Identifier(name) = &self.peek()?.token_type {
            let name = name.clone();
            self.advance();
            Ok(name)
        } else {
            Err(format!("{} at line {}", message, self.peek()?.line))
        }
    }
    
    fn is_at_end(&self) -> bool {
        matches!(self.peek().map(|t| &t.token_type), Ok(TokenType::Eof)) || self.current >= self.tokens.len()
    }
}

pub struct RenScriptCodeGenerator<'a> {
    ast: &'a ScriptAst,
}

impl<'a> RenScriptCodeGenerator<'a> {
    pub fn new(ast: &'a ScriptAst) -> Self {
        Self { ast }
    }
    
    pub fn generate(&self) -> String {
        let used_functions = self.analyze_used_functions();
        let api_bindings = self.generate_api_bindings(&used_functions);
        let math_bindings = self.generate_math_bindings(&used_functions);
        let variables = self.generate_variables();
        let methods = self.generate_methods();
        let functions = self.generate_functions();
        let properties_metadata = self.generate_properties_metadata();
        
        format!(
            r#"// Generated JavaScript from RenScript: {}
function createRenScript(scene, api) {{
{}{}
  function ScriptInstance() {{
    // Script variables
{}{}
  }}
  
  // Script methods
{}{}
  
  return ScriptInstance;
}}"#,
            self.ast.name,
            api_bindings,
            math_bindings,
            variables,
            properties_metadata,
            methods,
            if functions.is_empty() { String::new() } else { format!("\n\n  // Custom functions\n{}", functions) }
        )
    }
    
    fn generate_variables(&self) -> String {
        if self.ast.variables.is_empty() {
            return String::new();
        }
        
        let vars: Vec<String> = self.ast.variables.iter()
            .map(|var| format!("    this.{} = {};", var.name, self.generate_expression(&var.value)))
            .collect();
        
        format!("\n{}\n", vars.join("\n"))
    }
    
    fn generate_methods(&self) -> String {
        let methods: Vec<String> = self.ast.methods.iter()
            .map(|method| self.generate_method(method))
            .collect();
        
        if methods.is_empty() {
            return String::new();
        }
        
        format!("{}", methods.join("\n\n"))
    }
    
    fn generate_method(&self, method: &MethodDeclaration) -> String {
        let method_name = match method.method_type.as_str() {
            "start" => "onStart",
            "update" => "onUpdate", 
            "destroy" => "onDestroy",
            "once" => "onOnce",
            _ => &method.method_type,
        };
        
        let params = method.parameters.join(", ");
        let body = self.generate_statements_with_params(&method.statements, 1, &method.parameters);
        
        format!("  ScriptInstance.prototype.{} = function({}) {{\n{}\n  }};", method_name, params, body)
    }
    
    fn generate_functions(&self) -> String {
        let functions: Vec<String> = self.ast.functions.iter()
            .map(|func| self.generate_function(func))
            .collect();
        
        if functions.is_empty() {
            return String::new();
        }
        
        functions.join("\n\n")
    }
    
    fn generate_function(&self, func: &FunctionDeclaration) -> String {
        let params = func.parameters.join(", ");
        let body = self.generate_statements_with_params(&func.statements, 1, &func.parameters);
        
        format!("  ScriptInstance.prototype.{} = function({}) {{\n{}\n  }};", func.name, params, body)
    }
    
    
    fn generate_statements_with_params(&self, statements: &[Statement], indent_level: usize, parameters: &[String]) -> String {
        let indent = "  ".repeat(indent_level);
        let statements_code: Vec<String> = statements.iter()
            .map(|stmt| format!("{}{}", indent, self.generate_statement_with_params(stmt, indent_level, parameters)))
            .collect();
        
        statements_code.join("\n")
    }

    fn generate_statement_with_params(&self, statement: &Statement, indent_level: usize, parameters: &[String]) -> String {
        match statement {
            Statement::Assignment { name, value } => {
                format!("this.{} = {};", name, self.generate_expression_with_params(value, parameters))
            },
            Statement::Expression(expr) => {
                format!("{};", self.generate_expression_with_params(expr, parameters))
            },
            Statement::If { condition, then_statements, else_statements } => {
                let cond = self.generate_expression_with_params(condition, parameters);
                let then_body = self.generate_statements_with_params(then_statements, indent_level + 1, parameters);
                
                if let Some(else_stmts) = else_statements {
                    let else_body = self.generate_statements_with_params(else_stmts, indent_level + 1, parameters);
                    let indent = "  ".repeat(indent_level);
                    format!("if ({}) {{\n{}\n{}}} else {{\n{}\n{}}}", cond, then_body, indent, else_body, indent)
                } else {
                    let indent = "  ".repeat(indent_level);
                    format!("if ({}) {{\n{}\n{}}}", cond, then_body, indent)
                }
            },
            Statement::For { init, condition, update, statements } => {
                let init_code = self.generate_statement_with_params(init, indent_level, parameters).trim_end_matches(';').to_string();
                let cond_code = self.generate_expression_with_params(condition, parameters);
                let update_code = self.generate_statement_with_params(update, indent_level, parameters).trim_end_matches(';').to_string();
                let body = self.generate_statements_with_params(statements, indent_level + 1, parameters);
                let indent = "  ".repeat(indent_level);
                
                format!("for ({}; {}; {}) {{\n{}\n{}}}", init_code, cond_code, update_code, body, indent)
            },
            Statement::Return { value } => {
                if let Some(val) = value {
                    format!("return {};", self.generate_expression_with_params(val, parameters))
                } else {
                    "return;".to_string()
                }
            },
            Statement::Break => "break;".to_string(),
        }
    }
    
    fn generate_expression(&self, expression: &Expression) -> String {
        match expression {
            Expression::Literal(literal) => self.generate_literal(literal),
            Expression::Identifier(name) => {
                // Don't prefix math constants with 'this.'
                if matches!(name.as_str(), "PI" | "E") {
                    name.clone()
                } else {
                    format!("this.{}", name)
                }
            },
            Expression::Binary { left, operator, right } => {
                format!("({} {} {})", self.generate_expression(left), operator, self.generate_expression(right))
            },
            Expression::Unary { operator, operand } => {
                format!("({}{})", operator, self.generate_expression(operand))
            },
            Expression::Call { callee, arguments } => {
                let args: Vec<String> = arguments.iter()
                    .map(|arg| self.generate_expression(arg))
                    .collect();
                
                // Handle function calls - don't prefix API calls with 'this.'
                let callee_str = match callee.as_ref() {
                    Expression::Identifier(name) => {
                        // Check if it's an API function (common ones)
                        if self.is_api_function(name) {
                            name.clone()
                        } else {
                            format!("this.{}", name)
                        }
                    },
                    _ => self.generate_expression(callee),
                };
                
                format!("{}({})", callee_str, args.join(", "))
            },
            Expression::Member { object, property, computed } => {
                let obj = self.generate_expression(object);
                let prop = self.generate_expression(property);
                
                if *computed {
                    format!("{}[{}]", obj, prop)
                } else {
                    // For member expressions, property should not have 'this.' prefix
                    let prop_str = match property.as_ref() {
                        Expression::Identifier(name) => name.clone(),
                        _ => prop,
                    };
                    format!("{}.{}", obj, prop_str)
                }
            },
            Expression::Array(elements) => {
                let items: Vec<String> = elements.iter()
                    .map(|elem| self.generate_expression(elem))
                    .collect();
                format!("[{}]", items.join(", "))
            },
            Expression::Object(properties) => {
                let props: Vec<String> = properties.iter()
                    .map(|(key, value)| format!("{}: {}", key, self.generate_expression(value)))
                    .collect();
                format!("{{{}}}", props.join(", "))
            },
        }
    }
    
    fn generate_expression_with_params(&self, expression: &Expression, parameters: &[String]) -> String {
        match expression {
            Expression::Literal(literal) => self.generate_literal(literal),
            Expression::Identifier(name) => {
                // Don't prefix math constants with 'this.'
                if matches!(name.as_str(), "PI" | "E") {
                    name.clone()
                } 
                // Don't prefix function parameters with 'this.'
                else if parameters.contains(name) {
                    name.clone()
                } else {
                    format!("this.{}", name)
                }
            },
            Expression::Binary { left, operator, right } => {
                format!("({} {} {})", self.generate_expression_with_params(left, parameters), operator, self.generate_expression_with_params(right, parameters))
            },
            Expression::Unary { operator, operand } => {
                format!("({}{})", operator, self.generate_expression_with_params(operand, parameters))
            },
            Expression::Call { callee, arguments } => {
                let args: Vec<String> = arguments.iter()
                    .map(|arg| self.generate_expression_with_params(arg, parameters))
                    .collect();
                
                // Handle function calls - don't prefix API calls with 'this.'
                let callee_str = match callee.as_ref() {
                    Expression::Identifier(name) => {
                        // Check if it's an API function (common ones)
                        if self.is_api_function(name) {
                            name.clone()
                        } else {
                            format!("this.{}", name)
                        }
                    },
                    _ => self.generate_expression_with_params(callee, parameters),
                };
                
                format!("{}({})", callee_str, args.join(", "))
            },
            Expression::Member { object, property, computed } => {
                let obj = self.generate_expression_with_params(object, parameters);
                let prop = self.generate_expression_with_params(property, parameters);
                
                if *computed {
                    format!("{}[{}]", obj, prop)
                } else {
                    // For member expressions, property should not have 'this.' prefix
                    let prop_str = match property.as_ref() {
                        Expression::Identifier(name) => name.clone(),
                        _ => prop,
                    };
                    format!("{}.{}", obj, prop_str)
                }
            },
            Expression::Array(elements) => {
                let items: Vec<String> = elements.iter()
                    .map(|elem| self.generate_expression_with_params(elem, parameters))
                    .collect();
                format!("[{}]", items.join(", "))
            },
            Expression::Object(properties) => {
                let props: Vec<String> = properties.iter()
                    .map(|(key, value)| format!("{}: {}", key, self.generate_expression_with_params(value, parameters)))
                    .collect();
                format!("{{{}}}", props.join(", "))
            },
        }
    }
    
    fn generate_literal(&self, literal: &LiteralValue) -> String {
        match literal {
            LiteralValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            LiteralValue::Number(n) => n.to_string(),
            LiteralValue::Boolean(b) => b.to_string(),
            LiteralValue::Null => "null".to_string(),
        }
    }
    
    fn generate_properties_metadata(&self) -> String {
        if self.ast.properties.is_empty() {
            return String::new();
        }
        
        let props: Vec<String> = self.ast.properties.iter()
            .map(|prop| self.generate_property_metadata(prop))
            .collect();
        
        format!(
            "\n    // Script properties metadata\n    this._scriptProperties = [\n{}\n    ];\n    \n    // Script object type metadata\n    this._scriptObjectType = \"{}\";",
            props.join(",\n"),
            self.ast.object_type
        )
    }
    
    fn generate_property_metadata(&self, prop: &PropertyDeclaration) -> String {
        let default_val = prop.default_value.as_ref()
            .map(|v| self.generate_expression(v))
            .unwrap_or_else(|| "null".to_string());
        
        let min_val = prop.min.as_ref()
            .map(|v| self.generate_expression(v))
            .unwrap_or_else(|| "null".to_string());
        
        let max_val = prop.max.as_ref()
            .map(|v| self.generate_expression(v))
            .unwrap_or_else(|| "null".to_string());
        
        let options_val = prop.options.as_ref()
            .map(|opts| format!("[{}]", opts.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ")))
            .unwrap_or_else(|| "null".to_string());
        
        let description_val = prop.description.as_ref()
            .map(|desc| format!("\"{}\"", desc))
            .unwrap_or_else(|| "null".to_string());
        
        format!(
            r#"      {{
        name: "{}",
        type: "{}",
        section: "{}",
        defaultValue: {},
        min: {},
        max: {},
        options: {},
        description: {},
        triggerOnce: {}
      }}"#,
            prop.name,
            prop.prop_type,
            prop.section,
            default_val,
            min_val,
            max_val,
            options_val,
            description_val,
            prop.once
        )
    }
    
    fn analyze_used_functions(&self) -> std::collections::HashSet<String> {
        let mut used_functions = std::collections::HashSet::new();
        
        // Analyze variables
        for var in &self.ast.variables {
            self.analyze_expression_usage(&var.value, &mut used_functions);
        }
        
        // Analyze methods
        for method in &self.ast.methods {
            for statement in &method.statements {
                self.analyze_statement_usage(statement, &mut used_functions);
            }
        }
        
        // Analyze functions
        for function in &self.ast.functions {
            for statement in &function.statements {
                self.analyze_statement_usage(statement, &mut used_functions);
            }
        }
        
        used_functions
    }
    
    // Validate used functions and provide suggestions for undefined ones
    fn validate_api_functions(&self, used_functions: &std::collections::HashSet<String>) -> Result<(), RenScriptError> {
        let api_mappings = self.get_api_method_mappings();
        let available_functions: Vec<String> = api_mappings.iter().map(|(k, _)| k.clone()).collect();
        
        // Get user-defined function names from the AST
        let user_functions: std::collections::HashSet<String> = self.ast.functions.iter()
            .map(|f| f.name.clone())
            .collect();
        
        // Math functions that are always available
        let math_functions = ["sin", "cos", "tan", "sqrt", "abs", "floor", "ceil", "round", "min", "max", "atan2", "log", "exp", "pow"];
        
        // Built-in JavaScript functions that are always available
        let builtin_functions = ["String", "Number", "Boolean", "Array", "Object"];
        
        for func_name in used_functions {
            // Skip math functions
            if math_functions.contains(&func_name.as_str()) {
                continue;
            }
            
            // Skip built-in JavaScript functions
            if builtin_functions.contains(&func_name.as_str()) {
                continue;
            }
            
            // Skip user-defined functions
            if user_functions.contains(func_name) {
                continue;
            }
            
            // Check if function exists in API mappings
            if !available_functions.contains(func_name) {
                let suggestions = RenScriptParser::suggest_similar_functions_static(func_name, &available_functions);
                return Err(RenScriptError::UndefinedFunction { 
                    name: func_name.clone(), 
                    line: 0, // TODO: track line numbers in usage analysis
                    column: 0, 
                    suggestions 
                });
            }
        }
        
        Ok(())
    }
    
    fn analyze_statement_usage(&self, statement: &Statement, used_functions: &mut std::collections::HashSet<String>) {
        match statement {
            Statement::Assignment { value, .. } => {
                self.analyze_expression_usage(value, used_functions);
            },
            Statement::If { condition, then_statements, else_statements } => {
                self.analyze_expression_usage(condition, used_functions);
                for stmt in then_statements {
                    self.analyze_statement_usage(stmt, used_functions);
                }
                if let Some(else_stmts) = else_statements {
                    for stmt in else_stmts {
                        self.analyze_statement_usage(stmt, used_functions);
                    }
                }
            },
            Statement::For { init, condition, update, statements } => {
                self.analyze_statement_usage(init, used_functions);
                self.analyze_expression_usage(condition, used_functions);
                self.analyze_statement_usage(update, used_functions);
                for stmt in statements {
                    self.analyze_statement_usage(stmt, used_functions);
                }
            },
            Statement::Return { value: Some(expr) } => {
                self.analyze_expression_usage(expr, used_functions);
            },
            Statement::Expression(expr) => {
                self.analyze_expression_usage(expr, used_functions);
            },
            _ => {}
        }
    }
    
    fn analyze_expression_usage(&self, expression: &Expression, used_functions: &mut std::collections::HashSet<String>) {
        match expression {
            Expression::Call { callee, arguments } => {
                if let Expression::Identifier(name) = callee.as_ref() {
                    // Track all function calls for validation
                    used_functions.insert(name.clone());
                }
                for arg in arguments {
                    self.analyze_expression_usage(arg, used_functions);
                }
            },
            Expression::Binary { left, right, .. } => {
                self.analyze_expression_usage(left, used_functions);
                self.analyze_expression_usage(right, used_functions);
            },
            Expression::Unary { operand, .. } => {
                self.analyze_expression_usage(operand, used_functions);
            },
            Expression::Member { object, .. } => {
                self.analyze_expression_usage(object, used_functions);
            },
            Expression::Array(elements) => {
                for element in elements {
                    self.analyze_expression_usage(element, used_functions);
                }
            },
            Expression::Object(properties) => {
                for (_, value) in properties {
                    self.analyze_expression_usage(value, used_functions);
                }
            },
            _ => {}
        }
    }
    
    fn generate_api_bindings(&self, used_functions: &std::collections::HashSet<String>) -> String {
        let api_methods = self.get_api_method_mappings();
        let mut bindings = Vec::new();
        
        bindings.push("  // SMART: Only binding methods actually used in script".to_string());
        
        for (renscript_name, api_name) in api_methods {
            if used_functions.contains(&renscript_name) {
                bindings.push(format!(
                    "  if (!api.{}) throw new Error('RenScript API Error: Method \"{}\" not found in API for function \"{}\". Available methods: ' + Object.keys(api).join(', '));", 
                    api_name, api_name, renscript_name
                ));
                bindings.push(format!("  const {} = api.{}.bind(api);", renscript_name, api_name));
            }
        }
        
        if bindings.len() > 1 {
            format!("{}\n\n", bindings.join("\n"))
        } else {
            String::new()
        }
    }
    
    fn generate_math_bindings(&self, used_functions: &std::collections::HashSet<String>) -> String {
        let math_functions = vec![
            "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
            "sqrt", "abs", "floor", "ceil", "round", "pow", "exp",
            "min", "max", "random"
        ];
        
        let _builtin_functions = vec![
            "String", "Number", "Boolean", "Array", "Object"
        ];
        
        let api_methods = self.get_api_method_mappings();
        let api_function_names: std::collections::HashSet<String> = api_methods.iter()
            .map(|(name, _)| name.clone())
            .collect();
        
        let mut bindings = Vec::new();
        
        for func in math_functions {
            // Only bind math functions if they're not already handled by API
            if used_functions.contains(func) && !api_function_names.contains(func) {
                bindings.push(format!("  const {} = Math.{};", func, func));
            }
        }
        
        // Built-in JavaScript functions are already available globally, no need to bind them
        
        // Add PI and E constants if used (check for usage as identifiers)
        if used_functions.contains("PI") || self.script_contains_identifier("PI") {
            bindings.push("  const PI = Math.PI;".to_string());
        }
        if used_functions.contains("E") || self.script_contains_identifier("E") {
            bindings.push("  const E = Math.E;".to_string());
        }
        
        if !bindings.is_empty() {
            format!("{}\n\n", bindings.join("\n"))
        } else {
            String::new()
        }
    }
    
    fn get_api_method_mappings(&self) -> Vec<(String, String)> {
        get_api_method_mappings()
    }
    
    fn script_contains_identifier(&self, identifier: &str) -> bool {
        // Check if the script contains usage of this identifier anywhere
        for var in &self.ast.variables {
            if self.expression_contains_identifier(&var.value, identifier) {
                return true;
            }
        }
        
        for method in &self.ast.methods {
            for statement in &method.statements {
                if self.statement_contains_identifier(statement, identifier) {
                    return true;
                }
            }
        }
        
        for function in &self.ast.functions {
            for statement in &function.statements {
                if self.statement_contains_identifier(statement, identifier) {
                    return true;
                }
            }
        }
        
        false
    }
    
    fn statement_contains_identifier(&self, statement: &Statement, identifier: &str) -> bool {
        match statement {
            Statement::Assignment { value, .. } => {
                self.expression_contains_identifier(value, identifier)
            },
            Statement::If { condition, then_statements, else_statements } => {
                self.expression_contains_identifier(condition, identifier) ||
                then_statements.iter().any(|stmt| self.statement_contains_identifier(stmt, identifier)) ||
                else_statements.as_ref().map_or(false, |stmts| stmts.iter().any(|stmt| self.statement_contains_identifier(stmt, identifier)))
            },
            Statement::For { init, condition, update, statements } => {
                self.statement_contains_identifier(init, identifier) ||
                self.expression_contains_identifier(condition, identifier) ||
                self.statement_contains_identifier(update, identifier) ||
                statements.iter().any(|stmt| self.statement_contains_identifier(stmt, identifier))
            },
            Statement::Return { value: Some(expr) } => {
                self.expression_contains_identifier(expr, identifier)
            },
            Statement::Expression(expr) => {
                self.expression_contains_identifier(expr, identifier)
            },
            _ => false
        }
    }
    
    fn expression_contains_identifier(&self, expression: &Expression, identifier: &str) -> bool {
        match expression {
            Expression::Identifier(name) => name == identifier,
            Expression::Call { callee, arguments } => {
                self.expression_contains_identifier(callee, identifier) ||
                arguments.iter().any(|arg| self.expression_contains_identifier(arg, identifier))
            },
            Expression::Binary { left, right, .. } => {
                self.expression_contains_identifier(left, identifier) ||
                self.expression_contains_identifier(right, identifier)
            },
            Expression::Unary { operand, .. } => {
                self.expression_contains_identifier(operand, identifier)
            },
            Expression::Member { object, property, .. } => {
                self.expression_contains_identifier(object, identifier) ||
                self.expression_contains_identifier(property, identifier)
            },
            Expression::Array(elements) => {
                elements.iter().any(|element| self.expression_contains_identifier(element, identifier))
            },
            Expression::Object(properties) => {
                properties.iter().any(|(_, value)| self.expression_contains_identifier(value, identifier))
            },
            _ => false
        }
    }
    
    fn is_api_function(&self, name: &str) -> bool {
        // Check if this function name exists in our API mappings
        let api_methods = self.get_api_method_mappings();
        api_methods.iter().any(|(renscript_name, _)| renscript_name == name) ||
        // Also include math functions and utility functions
        matches!(name, 
            "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2" |
            "sqrt" | "abs" | "floor" | "ceil" | "round" | "pow" | "exp" | "log" |
            "min" | "max" | "random" | "PI" | "E" | "String" | "Number" | "Boolean" | "Array" | "Object"
        )
    }
}