/**
 * RenScript Compiler - Compiles RenScript to JavaScript
 * 
 * RenScript is a simple, clean scripting language for game objects
 * 
 * Example:
 * script MyScript {
 *   speed = 1.0
 *   
 *   start {
 *     log("Started!")
 *   }
 *   
 *   update(dt) {
 *     rotate_by(0, dt * speed, 0)
 *   }
 * }
 */

class RenScriptError extends Error {
  constructor(message, line = null, column = null, token = null, context = null) {
    super(message);
    this.name = 'RenScriptError';
    this.line = line;
    this.column = column;
    this.token = token;
    this.context = context;
  }

  toString() {
    let result = `RenScript Error: ${this.message}`;
    if (this.line !== null) {
      result += ` at line ${this.line}`;
      if (this.column !== null) {
        result += `, column ${this.column}`;
      }
    }
    if (this.token) {
      result += ` (token: ${this.token.type}="${this.token.value}")`;
    }
    if (this.context) {
      result += `\nContext: ${this.context}`;
    }
    return result;
  }
}

class RenScriptLexer {
  constructor(source) {
    this.source = source;
    this.position = 0;
    this.line = 1;
    this.column = 1;
    this.tokens = [];
  }
  
  tokenize() {
    while (this.position < this.source.length) {
      this.skipWhitespace();
      if (this.position >= this.source.length) break;
      
      const char = this.source[this.position];
      
      // Comments - support both # and //
      if (char === '#') {
        this.skipComment();
        continue;
      }
      
      // Check for // comments
      if (char === '/' && this.position + 1 < this.source.length && this.source[this.position + 1] === '/') {
        this.skipComment();
        continue;
      }
      
      // String literals
      if (char === '"' || char === "'") {
        this.readString();
        continue;
      }
      
      // Numbers
      if (this.isDigit(char)) {
        this.readNumber();
        continue;
      }
      
      // Identifiers and keywords
      if (this.isAlpha(char) || char === '_') {
        this.readIdentifier();
        continue;
      }
      
      // Check for multi-character operators
      if (char === '&') {
        if (this.position + 1 < this.source.length && this.source[this.position + 1] === '&') {
          this.addToken('LOGICAL_AND');
          this.advance(); // Skip second &
        } else {
          this.addToken('LOGICAL_AND'); // Single & also means logical AND
        }
        this.advance();
        continue;
      }
      
      if (char === '|') {
        if (this.position + 1 < this.source.length && this.source[this.position + 1] === '|') {
          this.addToken('LOGICAL_OR');
          this.advance(); // Skip second |
        } else {
          this.addToken('LOGICAL_OR'); // Single | also means logical OR
        }
        this.advance();
        continue;
      }
      
      if (char === '!') {
        if (this.position + 1 < this.source.length && this.source[this.position + 1] === '=') {
          this.addToken('NOT_EQUAL');
          this.advance(); // Skip =
        } else {
          this.addToken('LOGICAL_NOT');
        }
        this.advance();
        continue;
      }
      
      if (char === '=') {
        if (this.position + 1 < this.source.length && this.source[this.position + 1] === '=') {
          this.addToken('EQUAL');
          this.advance(); // Skip second =
          this.advance();
          continue;
        } else {
          this.addToken('ASSIGN');
          this.advance();
          continue;
        }
      }
      
      if (char === '<') {
        if (this.position + 1 < this.source.length && this.source[this.position + 1] === '=') {
          this.addToken('LESS_EQUAL');
          this.advance(); // Skip =
        } else {
          this.addToken('LESS');
        }
        this.advance();
        continue;
      }
      
      if (char === '>') {
        if (this.position + 1 < this.source.length && this.source[this.position + 1] === '=') {
          this.addToken('GREATER_EQUAL');
          this.advance(); // Skip =
        } else {
          this.addToken('GREATER');
        }
        this.advance();
        continue;
      }

      // Single character tokens
      switch (char) {
        case '{': this.addToken('LBRACE'); break;
        case '}': this.addToken('RBRACE'); break;
        case '(': this.addToken('LPAREN'); break;
        case ')': this.addToken('RPAREN'); break;
        case '[': this.addToken('LBRACKET'); break;
        case ']': this.addToken('RBRACKET'); break;
        case ',': this.addToken('COMMA'); break;
        case '.': this.addToken('DOT'); break;
        case '+':
          if (this.position + 1 < this.source.length && this.source[this.position + 1] === '+') {
            this.addToken('PLUSPLUS');
            this.advance(); // Skip second +
          } else {
            this.addToken('PLUS');
          }
          break;
        case '-': this.addToken('MINUS'); break;
        case '*': this.addToken('MULTIPLY'); break;
        case '/': this.addToken('DIVIDE'); break;
        case ';': this.addToken('SEMICOLON'); break;
        case ':': this.addToken('COLON'); break;
        case '?': this.addToken('QUESTION'); break;
        default:
          throw new RenScriptError(`Unexpected character '${char}'`, this.line, this.column, null, `Found '${char}' while tokenizing`);
      }
      
      this.advance();
    }
    
    this.addToken('EOF');
    return this.tokens;
  }
  
  skipWhitespace() {
    while (this.position < this.source.length) {
      const char = this.source[this.position];
      if (char === ' ' || char === '\t' || char === '\r') {
        this.advance();
      } else if (char === '\n') {
        this.line++;
        this.column = 1;
        this.advance();
      } else {
        break;
      }
    }
  }
  
  skipComment() {
    // Skip the comment start characters (# or //)
    if (this.source[this.position] === '#') {
      this.advance(); // Skip #
    } else if (this.source[this.position] === '/' && this.source[this.position + 1] === '/') {
      this.advance(); // Skip first /
      this.advance(); // Skip second /
    }
    
    // Skip the rest of the line
    while (this.position < this.source.length && this.source[this.position] !== '\n') {
      this.advance();
    }
  }
  
  readString() {
    const quote = this.source[this.position];
    this.advance(); // Skip opening quote
    
    let value = '';
    while (this.position < this.source.length && this.source[this.position] !== quote) {
      if (this.source[this.position] === '\\') {
        this.advance();
        if (this.position < this.source.length) {
          const escaped = this.source[this.position];
          switch (escaped) {
            case 'n': value += '\n'; break;
            case 't': value += '\t'; break;
            case 'r': value += '\r'; break;
            case '\\': value += '\\'; break;
            case '"': value += '"'; break;
            case "'": value += "'"; break;
            default: value += escaped; break;
          }
        }
      } else {
        value += this.source[this.position];
      }
      this.advance();
    }
    
    if (this.position >= this.source.length) {
      throw new Error(`Unterminated string at line ${this.line}`);
    }
    
    this.advance(); // Skip closing quote
    this.addToken('STRING', value);
  }
  
  readNumber() {
    let value = '';
    while (this.position < this.source.length && (this.isDigit(this.source[this.position]) || this.source[this.position] === '.')) {
      value += this.source[this.position];
      this.advance();
    }
    this.addToken('NUMBER', parseFloat(value));
  }
  
  readIdentifier() {
    let value = '';
    while (this.position < this.source.length && (this.isAlphaNumeric(this.source[this.position]) || this.source[this.position] === '_')) {
      value += this.source[this.position];
      this.advance();
    }
    
    // Check for keywords
    const keywords = {
      'script': 'SCRIPT',
      'mesh': 'OBJECT_TYPE',
      'camera': 'OBJECT_TYPE', 
      'light': 'OBJECT_TYPE',
      'scene': 'OBJECT_TYPE',
      'transform': 'OBJECT_TYPE',
      'props': 'PROPS',
      'start': 'START',
      'update': 'UPDATE',
      'destroy': 'DESTROY',
      'once': 'ONCE',
      'if': 'IF',
      'else': 'ELSE',
      'while': 'WHILE',
      'for': 'FOR',
      'true': 'TRUE',
      'false': 'FALSE',
      'null': 'NULL',
      'and': 'LOGICAL_AND',
      'or': 'LOGICAL_OR',
      'not': 'LOGICAL_NOT',
      'function': 'FUNCTION',
      'return': 'RETURN',
      'switch': 'SWITCH',
      'case': 'CASE',
      'break': 'BREAK'
    };
    
    const tokenType = keywords[value] || 'IDENTIFIER';
    const tokenValue = (tokenType === 'TRUE') ? true : 
                      (tokenType === 'FALSE') ? false :
                      (tokenType === 'NULL') ? null : value;
    
    this.addToken(tokenType, tokenValue);
  }
  
  isDigit(char) {
    return char >= '0' && char <= '9';
  }
  
  isAlpha(char) {
    return (char >= 'a' && char <= 'z') || (char >= 'A' && char <= 'Z');
  }
  
  isAlphaNumeric(char) {
    return this.isAlpha(char) || this.isDigit(char);
  }
  
  advance() {
    this.position++;
    this.column++;
  }
  
  addToken(type, value = null) {
    this.tokens.push({
      type,
      value,
      line: this.line,
      column: this.column
    });
  }
}

class RenScriptParser {
  constructor(tokens) {
    this.tokens = tokens;
    this.current = 0;
  }
  
  parse() {
    return this.script();
  }
  
  script() {
    // Handle both old syntax (script Name) and new syntax (camera Name, light Name, etc.)
    let objectType = 'script';
    let name;
    
    if (this.check('SCRIPT')) {
      this.advance(); // consume 'script'
      name = this.consume('IDENTIFIER', "Expected script name").value;
    } else if (this.check('OBJECT_TYPE')) {
      objectType = this.advance().value; // consume object type (camera, light, etc.)
      name = this.consume('IDENTIFIER', "Expected object name").value;
    } else {
      throw new Error("Expected 'script', 'camera', 'light', 'mesh', 'scene', or 'transform'");
    }
    
    this.consume('LBRACE', "Expected '{'");
    
    const variables = [];
    const methods = [];
    const properties = [];
    const functions = [];
    
    while (!this.check('RBRACE') && !this.isAtEnd()) {
      if (this.check('PROPS')) {
        properties.push(...this.propsDeclaration());
      } else if (this.check('IDENTIFIER') && this.peekNext()?.type === 'ASSIGN') {
        variables.push(this.variableDeclaration());
      } else if (this.check('START') || this.check('UPDATE') || this.check('DESTROY') || this.check('ONCE')) {
        methods.push(this.methodDeclaration());
      } else if (this.check('FUNCTION')) {
        functions.push(this.functionDeclaration());
      } else if (this.check('IDENTIFIER') && this.peekNext()?.type === 'LPAREN') {
        // Support function declaration without 'function' keyword: functionName() { ... }
        functions.push(this.functionDeclaration());
      } else {
        throw new Error(`Unexpected token ${this.peek().type} at line ${this.peek().line}`);
      }
    }
    
    this.consume('RBRACE', "Expected '}'");
    
    return {
      type: 'Script',
      name,
      objectType,
      variables,
      methods,
      properties,
      functions
    };
  }
  
  propsDeclaration() {
    this.consume('PROPS', "Expected 'props'");
    
    // Check if there's a section name (e.g., props rotation)
    let sectionName = 'General';
    if (this.check('IDENTIFIER')) {
      sectionName = this.advance().value;
    }
    
    this.consume('LBRACE', "Expected '{'");
    
    const properties = [];
    
    while (!this.check('RBRACE') && !this.isAtEnd()) {
      const prop = this.propertyDeclaration();
      prop.section = sectionName; // Add section metadata to each property
      properties.push(prop);
    }
    
    this.consume('RBRACE', "Expected '}'");
    return properties;
  }
  
  propertyDeclaration() {
    const name = this.consume('IDENTIFIER', "Expected property name").value;
    this.consume('COLON', "Expected ':'");
    
    const propType = this.consume('IDENTIFIER', "Expected property type").value;
    
    // Validate property type - only allow known types
    const validPropertyTypes = ['number', 'float', 'boolean', 'string', 'range', 'select'];
    if (!validPropertyTypes.includes(propType)) {
      throw new Error(`RenScript Error: Unknown property type '${propType}' in property '${name}' at line ${this.peek().line}.

Valid property types:
  • number    - Integer values with number input
  • float     - Decimal values with number input  
  • boolean   - True/false values with toggle switch
  • string    - Text values with text input
  • range     - Numeric values with slider control
  • select    - Choice from predefined options with dropdown

Please use one of these supported types instead.`);
    }
    
    let defaultValue = null;
    let min = null;
    let max = null;
    let options = null;
    let description = null;
    let triggerOnce = false;
    
    // Parse property options
    if (this.match('LBRACE')) {
      while (!this.check('RBRACE') && !this.isAtEnd()) {
        // Handle keywords as identifiers in property option context
        let optionName;
        if (this.check('IDENTIFIER')) {
          optionName = this.advance().value;
        } else if (this.check('ONCE')) {
          // Allow 'once' keyword to be used as property option
          optionName = this.advance().value || 'once';
        } else {
          throw new Error(`Expected option name at line ${this.peek().line}, got ${this.peek().type}`);
        }
        this.consume('COLON', "Expected ':'");
        
        switch (optionName) {
          case 'default':
            defaultValue = this.expression();
            break;
          case 'min':
            min = this.expression();
            break;
          case 'max':
            max = this.expression();
            break;
          case 'options':
            options = this.expression();
            break;
          case 'description':
            description = this.expression();
            break;
          case 'once':
            const onceValue = this.expression();
            if (onceValue.type === 'Literal' && onceValue.value === true) {
              triggerOnce = true;
            }
            break;
          default:
            throw new Error(`Unknown property option: ${optionName}`);
        }
        
        // Optional comma
        this.match('COMMA');
      }
      this.consume('RBRACE', "Expected '}'");
    }
    
    return {
      type: 'PropertyDeclaration',
      name,
      propType,
      defaultValue,
      min,
      max,
      options,
      description,
      triggerOnce
    };
  }
  
  variableDeclaration() {
    const name = this.consume('IDENTIFIER', "Expected variable name").value;
    this.consume('ASSIGN', "Expected '='");
    const value = this.expression();
    return {
      type: 'VariableDeclaration',
      name,
      value
    };
  }
  
  methodDeclaration() {
    const methodType = this.advance().type; // START, UPDATE, DESTROY, or ONCE
    let parameters = [];
    
    if (methodType === 'UPDATE' && this.check('LPAREN')) {
      this.advance(); // consume '('
      if (!this.check('RPAREN')) {
        parameters.push(this.consume('IDENTIFIER', "Expected parameter name").value);
      }
      this.consume('RPAREN', "Expected ')'");
    }
    
    this.consume('LBRACE', "Expected '{'");
    const statements = [];
    
    while (!this.check('RBRACE') && !this.isAtEnd()) {
      statements.push(this.statement());
    }
    
    this.consume('RBRACE', "Expected '}'");
    
    return {
      type: 'MethodDeclaration',
      methodType: methodType.toLowerCase(),
      parameters,
      statements
    };
  }
  
  functionDeclaration() {
    // Handle both 'function name()' and 'name()' syntax
    let functionName;
    
    if (this.check('FUNCTION')) {
      this.advance(); // consume 'function'
      functionName = this.consume('IDENTIFIER', "Expected function name").value;
    } else if (this.check('IDENTIFIER')) {
      functionName = this.advance().value; // consume function name
    } else {
      throw new Error("Expected function name");
    }
    
    // Parse parameters
    this.consume('LPAREN', "Expected '(' after function name");
    const parameters = [];
    
    if (!this.check('RPAREN')) {
      do {
        parameters.push(this.consume('IDENTIFIER', "Expected parameter name").value);
      } while (this.match('COMMA'));
    }
    
    this.consume('RPAREN', "Expected ')' after parameters");
    
    // Parse function body
    this.consume('LBRACE', "Expected '{' before function body");
    const statements = [];
    
    while (!this.check('RBRACE') && !this.isAtEnd()) {
      statements.push(this.statement());
    }
    
    this.consume('RBRACE', "Expected '}' after function body");
    
    return {
      type: 'FunctionDeclaration',
      name: functionName,
      parameters,
      statements
    };
  }
  
  statement() {
    if (this.check('IF')) {
      return this.ifStatement();
    }
    
    if (this.check('SWITCH')) {
      return this.switchStatement();
    }
    
    if (this.check('FOR')) {
      return this.forStatement();
    }
    
    if (this.check('RETURN')) {
      return this.returnStatement();
    }
    
    if (this.check('BREAK')) {
      return this.breakStatement();
    }
    
    if (this.check('IDENTIFIER') && this.peekNext()?.type === 'ASSIGN') {
      return this.assignment();
    }
    
    return this.expressionStatement();
  }

  ifStatement() {
    this.consume('IF', "Expected 'if'");
    this.consume('LPAREN', "Expected '(' after 'if'");
    const condition = this.expression();
    this.consume('RPAREN', "Expected ')' after if condition");
    
    // Handle both braced and single statement forms
    const thenStatements = [];
    if (this.check('LBRACE')) {
      this.advance(); // consume '{'
      while (!this.check('RBRACE') && !this.isAtEnd()) {
        thenStatements.push(this.statement());
      }
      this.consume('RBRACE', "Expected '}' after if body");
    } else {
      // Single statement
      thenStatements.push(this.statement());
    }
    
    let elseStatements = null;
    if (this.match('ELSE')) {
      elseStatements = [];
      if (this.check('LBRACE')) {
        this.advance(); // consume '{'
        while (!this.check('RBRACE') && !this.isAtEnd()) {
          elseStatements.push(this.statement());
        }
        this.consume('RBRACE', "Expected '}' after else body");
      } else {
        // Single statement
        elseStatements.push(this.statement());
      }
    }
    
    return {
      type: 'IfStatement',
      condition,
      thenStatements,
      elseStatements
    };
  }

  returnStatement() {
    this.consume('RETURN', "Expected 'return'");
    
    let value = null;
    if (!this.check('RBRACE') && !this.isAtEnd()) {
      value = this.expression();
    }
    
    return {
      type: 'ReturnStatement',
      value
    };
  }

  switchStatement() {
    this.consume('SWITCH', "Expected 'switch'");
    this.consume('LPAREN', "Expected '(' after 'switch'");
    const discriminant = this.expression();
    this.consume('RPAREN', "Expected ')' after switch expression");
    this.consume('LBRACE', "Expected '{' after switch expression");
    
    const cases = [];
    
    while (!this.check('RBRACE') && !this.isAtEnd()) {
      if (this.check('CASE')) {
        this.advance(); // consume 'case'
        const test = this.expression();
        this.consume('COLON', "Expected ':' after case value");
        
        const statements = [];
        while (!this.check('CASE') && !this.check('RBRACE') && !this.isAtEnd()) {
          statements.push(this.statement());
        }
        
        cases.push({
          type: 'SwitchCase',
          test,
          statements
        });
      } else {
        throw new Error(`Unexpected token in switch: ${this.peek().type}`);
      }
    }
    
    this.consume('RBRACE', "Expected '}' after switch body");
    
    return {
      type: 'SwitchStatement',
      discriminant,
      cases
    };
  }

  breakStatement() {
    this.consume('BREAK', "Expected 'break'");
    return {
      type: 'BreakStatement'
    };
  }

  forStatement() {
    this.consume('FOR', "Expected 'for'");
    this.consume('LPAREN', "Expected '(' after 'for'");
    
    // Parse init (variable = value)
    const init = this.assignment();
    this.consume('SEMICOLON', "Expected ';' after for init");
    
    // Parse condition (variable < value)
    const condition = this.expression();
    this.consume('SEMICOLON', "Expected ';' after for condition");
    
    // Parse update (variable++)
    const update = this.forUpdate();
    this.consume('RPAREN', "Expected ')' after for update");
    
    // Parse body
    this.consume('LBRACE', "Expected '{' after for header");
    const statements = [];
    
    while (!this.check('RBRACE') && !this.isAtEnd()) {
      statements.push(this.statement());
    }
    
    this.consume('RBRACE', "Expected '}' after for body");
    
    return {
      type: 'ForStatement',
      init,
      condition,
      update,
      statements
    };
  }

  forUpdate() {
    // Handle simple increment: i++, i = i + 1, etc.
    if (this.check('IDENTIFIER')) {
      const variable = this.advance().value;
      
      if (this.match('PLUSPLUS')) {
        return {
          type: 'UpdateExpression',
          operator: '++',
          argument: { type: 'Identifier', name: variable },
          prefix: false
        };
      } else if (this.match('ASSIGN')) {
        const value = this.expression();
        return {
          type: 'Assignment',
          name: variable,
          value
        };
      }
    }
    
    return this.expression();
  }
  
  assignment() {
    const name = this.consume('IDENTIFIER', "Expected variable name").value;
    this.consume('ASSIGN', "Expected '='");
    const value = this.expression();
    
    return {
      type: 'Assignment',
      name,
      value
    };
  }
  
  expressionStatement() {
    const expr = this.expression();
    return {
      type: 'ExpressionStatement',
      expression: expr
    };
  }
  
  expression() {
    return this.logicalOr();
  }
  
  logicalOr() {
    let expr = this.logicalAnd();
    
    while (this.match('LOGICAL_OR')) {
      const operator = this.previous().type;
      const right = this.logicalAnd();
      expr = {
        type: 'BinaryExpression',
        operator: '||',
        left: expr,
        right
      };
    }
    
    return expr;
  }
  
  logicalAnd() {
    let expr = this.equality();
    
    while (this.match('LOGICAL_AND')) {
      const operator = this.previous().type;
      const right = this.equality();
      expr = {
        type: 'BinaryExpression',
        operator: '&&',
        left: expr,
        right
      };
    }
    
    return expr;
  }
  
  equality() {
    let expr = this.comparison();
    
    while (this.match('EQUAL', 'NOT_EQUAL')) {
      const operator = this.previous().type;
      const right = this.comparison();
      expr = {
        type: 'BinaryExpression',
        operator: operator === 'EQUAL' ? '==' : '!=',
        left: expr,
        right
      };
    }
    
    return expr;
  }
  
  comparison() {
    let expr = this.addition();
    
    while (this.match('GREATER', 'GREATER_EQUAL', 'LESS', 'LESS_EQUAL')) {
      const operator = this.previous().type;
      let op;
      switch (operator) {
        case 'GREATER': op = '>'; break;
        case 'GREATER_EQUAL': op = '>='; break;
        case 'LESS': op = '<'; break;
        case 'LESS_EQUAL': op = '<='; break;
      }
      const right = this.addition();
      expr = {
        type: 'BinaryExpression',
        operator: op,
        left: expr,
        right
      };
    }
    
    return expr;
  }
  
  addition() {
    let expr = this.multiplication();
    
    while (this.match('PLUS', 'MINUS')) {
      const operator = this.previous().type;
      const right = this.multiplication();
      expr = {
        type: 'BinaryExpression',
        operator: operator === 'PLUS' ? '+' : '-',
        left: expr,
        right
      };
    }
    
    return expr;
  }
  
  multiplication() {
    let expr = this.unary();
    
    while (this.match('MULTIPLY', 'DIVIDE')) {
      const operator = this.previous().type;
      const right = this.unary();
      expr = {
        type: 'BinaryExpression',
        operator: operator === 'MULTIPLY' ? '*' : '/',
        left: expr,
        right
      };
    }
    
    return expr;
  }
  
  unary() {
    if (this.match('MINUS', 'LOGICAL_NOT')) {
      const operator = this.previous().type;
      const right = this.unary();
      return {
        type: 'UnaryExpression',
        operator: operator === 'MINUS' ? '-' : '!',
        operand: right
      };
    }
    
    return this.call();
  }
  
  call() {
    let expr = this.primary();
    
    while (true) {
      if (this.match('LPAREN')) {
        expr = this.finishCall(expr);
      } else if (this.match('LBRACKET')) {
        expr = this.finishArrayAccess(expr);
      } else if (this.match('DOT')) {
        expr = this.finishPropertyAccess(expr);
      } else {
        break;
      }
    }
    
    return expr;
  }
  
  finishCall(callee) {
    const args = [];
    
    if (!this.check('RPAREN')) {
      do {
        args.push(this.expression());
      } while (this.match('COMMA'));
    }
    
    this.consume('RPAREN', "Expected ')' after arguments");
    
    return {
      type: 'CallExpression',
      callee,
      arguments: args
    };
  }

  finishArrayAccess(object) {
    const index = this.expression();
    this.consume('RBRACKET', "Expected ']' after array index");
    
    return {
      type: 'MemberExpression',
      object,
      property: index,
      computed: true
    };
  }

  finishPropertyAccess(object) {
    const property = this.consume('IDENTIFIER', "Expected property name after '.'").value;
    
    return {
      type: 'MemberExpression',
      object,
      property: { type: 'Identifier', name: property },
      computed: false
    };
  }
  
  primary() {
    if (this.match('TRUE', 'FALSE', 'NULL')) {
      return {
        type: 'Literal',
        value: this.previous().value
      };
    }
    
    if (this.match('NUMBER', 'STRING')) {
      return {
        type: 'Literal',
        value: this.previous().value
      };
    }
    
    if (this.match('IDENTIFIER')) {
      return {
        type: 'Identifier',
        name: this.previous().value
      };
    }
    
    if (this.match('LPAREN')) {
      const expr = this.expression();
      this.consume('RPAREN', "Expected ')' after expression");
      return expr;
    }

    if (this.match('LBRACKET')) {
      const elements = [];
      
      if (!this.check('RBRACKET')) {
        do {
          elements.push(this.expression());
        } while (this.match('COMMA'));
      }
      
      this.consume('RBRACKET', "Expected ']' after array elements");
      
      return {
        type: 'ArrayLiteral',
        elements
      };
    }

    if (this.match('LBRACE')) {
      const properties = [];
      
      if (!this.check('RBRACE')) {
        do {
          const key = this.consume('IDENTIFIER', "Expected property name").value;
          this.consume('COLON', "Expected ':' after property name");
          const value = this.expression();
          
          properties.push({
            key: key,
            value: value
          });
        } while (this.match('COMMA'));
      }
      
      this.consume('RBRACE', "Expected '}' after object properties");
      
      return {
        type: 'ObjectLiteral',
        properties
      };
    }
    
    throw new Error(`Unexpected token ${this.peek().type} at line ${this.peek().line}`);
  }
  
  
  // Utility methods
  match(...types) {
    for (const type of types) {
      if (this.check(type)) {
        this.advance();
        return true;
      }
    }
    return false;
  }
  
  check(type) {
    if (this.isAtEnd()) return false;
    return this.peek().type === type;
  }
  
  advance() {
    if (!this.isAtEnd()) this.current++;
    return this.previous();
  }
  
  isAtEnd() {
    return this.peek().type === 'EOF';
  }
  
  peek() {
    return this.tokens[this.current];
  }
  
  peekNext() {
    if (this.current + 1 >= this.tokens.length) return null;
    return this.tokens[this.current + 1];
  }
  
  previous() {
    return this.tokens[this.current - 1];
  }
  
  consume(type, message) {
    if (this.check(type)) return this.advance();
    throw new Error(`${message} at line ${this.peek().line}, got ${this.peek().type}`);
  }
}

class RenScriptCodeGenerator {
  constructor(ast) {
    this.ast = ast;
    this.usedFunctions = new Set();
    this.usedArrayMethods = new Set();
    this.customFunctions = new Set();
    
    // Extract custom function names from AST
    if (ast.functions) {
      ast.functions.forEach(func => {
        this.customFunctions.add(func.name);
      });
    }
  }
  
  generate() {
    // First pass: analyze what functions are used
    this.analyzeUsage(this.ast);
    return this.generateScript(this.ast);
  }

  analyzeUsage(node) {
    if (!node || typeof node !== 'object') return;
    
    if (node.type === 'CallExpression' && node.callee?.type === 'Identifier') {
      this.usedFunctions.add(node.callee.name);
    }
    
    // Recursively analyze child nodes
    Object.values(node).forEach(child => {
      if (Array.isArray(child)) {
        child.forEach(item => this.analyzeUsage(item));
      } else if (typeof child === 'object' && child !== null) {
        this.analyzeUsage(child);
      }
    });
  }

  generateUsedApiBindings() {
    // API method mappings (snake_case -> camelCase) - COMPREHENSIVE BABYLON.JS COVERAGE
    const apiMethods = {
      // === CORE TRANSFORM & UTILITY ===
      log: 'log',
      get_position: 'getPosition',
      set_position: 'setPosition',
      get_world_position: 'getWorldPosition',
      get_rotation: 'getRotation',
      set_rotation: 'setRotation',
      get_world_rotation: 'getWorldRotationQuaternion',
      get_scale: 'getScale',
      set_scale: 'setScale',
      rotate_by: 'rotateBy',
      move_by: 'moveBy',
      move_to: 'setPosition',
      look_at: 'lookAt',
      is_visible: 'isVisible',
      set_visible: 'setVisible',
      set_enabled: 'setEnabled',
      is_enabled: 'isEnabled',
      get_name: 'getName',
      set_name: 'setName',
      get_id: 'getId',
      
      // === TAGS & METADATA ===
      add_tag: 'addTag',
      remove_tag: 'removeTag',
      has_tag: 'hasTag',
      get_tags: 'getTags',
      set_metadata: 'setMetadata',
      get_metadata: 'getMetadata',
      has_metadata: 'hasMetadata',
      remove_metadata: 'removeMetadata',
      
      // === TIME & UTILITY ===
      get_delta_time: 'getDeltaTime',
      get_time: 'getTime',
      random: 'random',
      random_range: 'randomRange',
      clamp: 'clamp',
      lerp: 'lerp',
      distance: 'distance',
      normalize: 'normalize',
      dot: 'dot',
      cross: 'cross',
      to_radians: 'toRadians',
      to_degrees: 'toDegrees',
      
      // === MATERIAL & COLOR ===
      set_color: 'setColor',
      get_color: 'getColor',
      set_alpha: 'setAlpha',
      get_alpha: 'getAlpha',
      set_diffuse_color: 'setDiffuseColor',
      set_specular_color: 'setSpecularColor',
      set_emissive_color: 'setEmissiveColor',
      set_ambient_color: 'setAmbientColor',
      get_emissive_color: 'getEmissiveColor',
      set_specular_power: 'setSpecularPower',
      set_material_property: 'setMaterialProperty',
      get_material_property: 'getMaterialProperty',
      
      // === TEXTURE SYSTEM ===
      set_texture: 'setTexture',
      set_diffuse_texture: 'setDiffuseTexture',
      set_normal_texture: 'setNormalTexture',
      set_emissive_texture: 'setEmissiveTexture',
      set_specular_texture: 'setSpecularTexture',
      set_ambient_texture: 'setAmbientTexture',
      set_opacity_texture: 'setOpacityTexture',
      set_reflection_texture: 'setReflectionTexture',
      set_refraction_texture: 'setRefractionTexture',
      set_lightmap_texture: 'setLightmapTexture',
      set_metallic_texture: 'setMetallicTexture',
      set_roughness_texture: 'setRoughnessTexture',
      set_micro_roughness_texture: 'setMicroRoughnessTexture',
      set_displacement_texture: 'setDisplacementTexture',
      set_detail_texture: 'setDetailTexture',
      
      // === MATERIAL RENDERING ===
      set_back_face_culling: 'setBackFaceCulling',
      set_disable_lighting: 'setDisableLighting',
      set_wireframe: 'setWireframe',
      set_points_cloud: 'setPointsCloud',
      set_fill_mode: 'setFillMode',
      set_invert_normal_map_x: 'setInvertNormalMapX',
      set_invert_normal_map_y: 'setInvertNormalMapY',
      set_bump_level: 'setBumpLevel',
      set_parallax_scale_bias: 'setParallaxScaleBias',
      set_index_of_refraction: 'setIndexOfRefraction',
      set_fresnel_parameters: 'setFresnelParameters',
      create_standard_material: 'createStandardMaterial',
      create_pbr_material: 'createPBRMaterial',
      create_dynamic_texture: 'createDynamicTexture',
      create_render_target_texture: 'createRenderTargetTexture',
      
      // === ANIMATION SYSTEM ===
      animate: 'animate',
      stop_animation: 'stopAnimation',
      pause_animation: 'pauseAnimation',
      resume_animation: 'resumeAnimation',
      animate_position: 'animatePosition',
      animate_rotation: 'animateRotation',
      animate_scale: 'animateScale',
      animate_color: 'animateColor',
      animate_alpha: 'animateAlpha',
      animate_to: 'animateTo',
      create_animation_group: 'createAnimationGroup',
      add_to_animation_group: 'addToAnimationGroup',
      play_animation_group: 'playAnimationGroup',
      stop_animation_group: 'stopAnimationGroup',
      create_keyframe_animation: 'createKeyframeAnimation',
      blend_animations: 'blendAnimations',
      on_animation_complete: 'onAnimationComplete',
      on_animation_loop: 'onAnimationLoop',
      set_animation_speed: 'setAnimationSpeed',
      get_animation_speed: 'getAnimationSpeed',
      is_animating: 'isAnimating',
      get_active_animations: 'getActiveAnimations',
      get_animation_progress: 'getAnimationProgress',
      
      // === SKELETON & BONE ANIMATION ===
      has_skeleton: 'hasSkeleton',
      get_skeleton: 'getSkeleton',
      get_bone_count: 'getBoneCount',
      get_bone: 'getBone',
      get_bone_by_name: 'getBoneByName',
      set_bone_position: 'setBonePosition',
      set_bone_rotation: 'setBoneRotation',
      get_bone_position: 'getBonePosition',
      get_bone_rotation: 'getBoneRotation',
      
      // Animation range/clip management
      play_animation_range: 'playAnimationByName',
      stop_animation_range: 'stopAnimationRange',
      set_animation_range: 'setAnimationRange',
      get_animation_ranges: 'animation_getAllAnimations',
      
      // Character animation states
      play_walk_animation: 'playWalkAnimation',
      play_run_animation: 'playRunAnimation', 
      play_idle_animation: 'playIdleAnimation',
      play_jump_animation: 'playJumpAnimation',
      play_crouch_animation: 'playCrouchAnimation',
      play_custom_animation: 'playCustomAnimation',
      
      // Animation blending
      set_animation_weight: 'setAnimationWeight',
      blend_to_animation: 'blendToAnimation',
      crossfade_animation: 'crossfadeAnimation',
      
      // Animation state queries
      is_animation_playing: 'isAnimationPlaying',
      get_current_animation: 'getCurrentAnimation',
      get_animation_time: 'getAnimationTime',
      set_animation_time: 'setAnimationTime',
      
      // Basic animation creation
      animation_create_animation: 'animation_createAnimation',
      animation_create_vector_animation: 'animation_createVectorAnimation',
      animation_create_color_animation: 'animation_createColorAnimation',
      animation_create_quaternion_animation: 'animation_createQuaternionAnimation',
      
      // Animation keyframes
      animation_add_animation_keys: 'animation_addAnimationKeys',
      animation_parse_animation_value: 'animation_parseAnimationValue',
      
      // Animation playback
      animation_play_animation: 'animation_playAnimation',
      animation_stop_animation: 'animation_stopAnimation',
      animation_pause_animation: 'animation_pauseAnimation',
      animation_restart_animation: 'animation_restartAnimation',
      
      // Easing functions
      animation_create_bezier_ease: 'animation_createBezierEase',
      animation_create_circle_ease: 'animation_createCircleEase',
      animation_create_back_ease: 'animation_createBackEase',
      animation_create_bounce_ease: 'animation_createBounceEase',
      animation_create_elastic_ease: 'animation_createElasticEase',
      animation_create_exponential_ease: 'animation_createExponentialEase',
      animation_create_power_ease: 'animation_createPowerEase',
      animation_set_easing_mode: 'animation_setEasingMode',
      
      // Animation groups
      animation_create_animation_group: 'animation_createAnimationGroup',
      animation_add_animation_to_group: 'animation_addAnimationToGroup',
      animation_play_animation_group: 'animation_playAnimationGroup',
      animation_stop_animation_group: 'animation_stopAnimationGroup',
      animation_pause_animation_group: 'animation_pauseAnimationGroup',
      animation_reset_animation_group: 'animation_resetAnimationGroup',
      
      // Skeleton animation
      animation_create_skeleton: 'animation_createSkeleton',
      animation_play_skeleton_animation: 'animation_playSkeletonAnimation',
      animation_stop_skeleton_animation: 'animation_stopSkeletonAnimation',
      animation_create_animation_range: 'animation_createAnimationRange',
      animation_delete_animation_range: 'animation_deleteAnimationRange',
      animation_get_skeleton_animation_ranges: 'animation_getSkeletonAnimationRanges',
      
      // Bone manipulation
      animation_get_bone_by_name: 'animation_getBoneByName',
      animation_set_bone_transform: 'animation_setBoneTransform',
      animation_get_bone_world_matrix: 'animation_getBoneWorldMatrix',
      animation_attach_mesh_to_bone: 'animation_attachMeshToBone',
      
      // Morph target animation
      animation_create_morph_target_manager: 'animation_createMorphTargetManager',
      animation_add_morph_target: 'animation_addMorphTarget',
      animation_set_morph_target_influence: 'animation_setMorphTargetInfluence',
      animation_animate_morph_target: 'animation_animateMorphTarget',
      
      // Advanced animation features
      animation_blend_animations: 'animation_blendAnimations',
      animation_animate_along_path: 'animation_animateAlongPath',
      animation_animate_rotation_around_axis: 'animation_animateRotationAroundAxis',
      animation_animate_opacity: 'animation_animateOpacity',
      
      // Animation weight & blending
      animation_set_animation_weight: 'animation_setAnimationWeight',
      animation_blend_to_animation: 'animation_blendToAnimation',
      
      // Animation events
      animation_add_animation_event: 'animation_addAnimationEvent',
      animation_remove_animation_events: 'animation_removeAnimationEvents',
      
      // Animation utilities
      animation_get_animation_progress: 'animation_getAnimationProgress',
      animation_is_animation_playing: 'animation_isAnimationPlaying',
      animation_get_all_animations: 'animation_getAllAnimations',
      
      // Physics animation
      animation_animate_with_physics: 'animation_animateWithPhysics',
      
      // Animation curves
      animation_create_animation_curve: 'animation_createAnimationCurve',
      animation_get_curve_point: 'animation_getCurvePoint',
      animation_get_curve_tangent: 'animation_getCurveTangent',
      
      // Smart animation player
      animation_play_animation_by_name: 'animation_playAnimationByName',
      
      // Animation info
      animation_get_animation_info: 'animation_getAnimationInfo',
      
      // === SCENE QUERIES ===
      find_object_by_name: 'findObjectByName',
      find_objects_by_name: 'findObjectsByName',
      find_objects_by_tag: 'findObjectsByTag',
      find_objects_with_tag: 'findObjectsWithTag',
      get_all_meshes: 'getAllMeshes',
      get_all_lights: 'getAllLights',
      get_all_cameras: 'getAllCameras',
      
      // === CAMERA CONTROL ===
      get_active_camera: 'getActiveCamera',
      set_camera_position: 'setCameraPosition',
      get_camera_position: 'getCameraPosition',
      set_camera_target: 'setCameraTarget',
      get_camera_target: 'getCameraTarget',
      set_camera_rotation: 'setCameraRotation',
      get_camera_rotation: 'getCameraRotation',
      
      // === RAYCASTING & PICKING ===
      raycast: 'raycast',
      raycast_from_camera: 'raycastFromCamera',
      multi_raycast: 'multiRaycast',
      pick_object: 'pickObject',
      pick_objects: 'pickObjects',
      
      // === SPATIAL QUERIES ===
      get_objects_in_radius: 'getObjectsInRadius',
      get_objects_in_box: 'getObjectsInBox',
      get_closest_object: 'getClosestObject',
      intersects_mesh: 'intersectsMesh',
      intersects_point: 'intersectsPoint',
      get_bounding_info: 'getBoundingInfo',
      
      // === OBJECT MANAGEMENT ===
      dispose_object: 'disposeObject',
      clone_object: 'cloneObject',
      is_in_camera_view: 'isInCameraView',
      set_occlusion_query: 'setOcclusionQuery',
      add_lod_level: 'addLODLevel',
      remove_lod_level: 'removeLODLevel',
      
      // === PHYSICS SYSTEM ===
      enable_physics: 'enablePhysics',
      disable_physics: 'disablePhysics',
      is_physics_enabled: 'isPhysicsEnabled',
      set_gravity: 'setGravity',
      get_gravity: 'getGravity',
      set_physics_impostor: 'setPhysicsImpostor',
      remove_physics_impostor: 'removePhysicsImpostor',
      havok_update: 'havokUpdate',
      has_physics_impostor: 'hasPhysicsImpostor',
      apply_impulse: 'applyImpulse',
      apply_force: 'applyForce',
      set_linear_velocity: 'setLinearVelocity',
      get_linear_velocity: 'getLinearVelocity',
      set_angular_velocity: 'setAngularVelocity',
      get_angular_velocity: 'getAngularVelocity',
      set_mass: 'setMass',
      get_mass: 'getMass',
      set_friction: 'setFriction',
      get_friction: 'getFriction',
      set_restitution: 'setRestitution',
      get_restitution: 'getRestitution',
      create_physics_joint: 'createPhysicsJoint',
      remove_physics_joint: 'removePhysicsJoint',
      on_collision_enter: 'onCollisionEnter',
      on_collision_exit: 'onCollisionExit',
      physics_raycast: 'physicsRaycast',
      create_character_controller: 'createCharacterController',
      move_character: 'moveCharacter',
      jump_character: 'jumpCharacter',
      enable_ragdoll: 'enableRagdoll',
      disable_ragdoll: 'disableRagdoll',
      enable_soft_body: 'enableSoftBody',
      set_soft_body_properties: 'setSoftBodyProperties',
      create_physics_material: 'createPhysicsMaterial',
      set_physics_material: 'setPhysicsMaterial',
      pause_physics: 'pausePhysics',
      resume_physics: 'resumePhysics',
      set_physics_time_step: 'setPhysicsTimeStep',
      enable_physics_debug: 'enablePhysicsDebug',
      disable_physics_debug: 'disablePhysicsDebug',
      dispose_physics: 'disposePhysics',
      
      // === INPUT SYSTEM ===
      is_key_pressed: 'isKeyPressed',
      is_key_down: 'isKeyDown',
      is_any_key_pressed: 'isAnyKeyPressed',
      get_pressed_keys: 'getPressedKeys',
      is_key_combo_pressed: 'isKeyComboPressed',
      is_ctrl_pressed: 'isCtrlPressed',
      is_shift_pressed: 'isShiftPressed',
      is_alt_pressed: 'isAltPressed',
      is_mouse_button_pressed: 'isMouseButtonPressed',
      is_left_mouse_button_pressed: 'isLeftMouseButtonPressed',
      is_right_mouse_button_pressed: 'isRightMouseButtonPressed',
      is_middle_mouse_button_pressed: 'isMiddleMouseButtonPressed',
      get_mouse_position: 'getMousePosition',
      get_mouse_x: 'getMouseX',
      get_mouse_y: 'getMouseY',
      get_mouse_normalized: 'getMouseNormalized',
      get_touch_count: 'getTouchCount',
      get_touches: 'getTouches',
      get_touch: 'getTouch',
      is_touching: 'isTouching',
      get_pinch_distance: 'getPinchDistance',
      get_touch_center: 'getTouchCenter',
      get_gamepads: 'getGamepads',
      get_gamepad: 'getGamepad',
      is_gamepad_connected: 'isGamepadConnected',
      is_gamepad_button_pressed: 'isGamepadButtonPressed',
      get_gamepad_button_value: 'getGamepadButtonValue',
      get_left_stick: 'getLeftStick',
      get_right_stick: 'getRightStick',
      get_left_stick_x: 'getLeftStickX',
      get_left_stick_y: 'getLeftStickY',
      get_right_stick_x: 'getRightStickX',
      get_right_stick_y: 'getRightStickY',
      get_left_trigger: 'getLeftTrigger',
      get_right_trigger: 'getRightTrigger',
      get_gamepad_trigger: 'getGamepadTrigger',
      is_gamepad_button_a: 'isGamepadButtonA',
      is_gamepad_button_b: 'isGamepadButtonB',
      is_gamepad_button_x: 'isGamepadButtonX',
      is_gamepad_button_y: 'isGamepadButtonY',
      apply_deadzone: 'applyDeadzone',
      get_left_stick_with_deadzone: 'getLeftStickWithDeadzone',
      get_right_stick_with_deadzone: 'getRightStickWithDeadzone',
      on_key_down: 'onKeyDown',
      on_key_up: 'onKeyUp',
      on_mouse_down: 'onMouseDown',
      on_mouse_up: 'onMouseUp',
      request_pointer_lock: 'requestPointerLock',
      exit_pointer_lock: 'exitPointerLock',
      is_pointer_locked: 'isPointerLocked',
      create_virtual_joystick: 'createVirtualJoystick',
      vibrate_gamepad: 'vibrateGamepad',
      get_input_snapshot: 'getInputSnapshot',
      
      // === SCENE MANAGEMENT ===
      get_scene_info: 'getSceneInfo',
      enable_performance_monitor: 'enablePerformanceMonitor',
      disable_performance_monitor: 'disablePerformanceMonitor',
      get_performance_data: 'getPerformanceData',
      
      // === ALL MATERIAL TYPES ===
      create_standard_material: 'createStandardMaterial',
      create_pbr_material: 'createPBRMaterial',
      create_pbr_metallic_roughness_material: 'createPBRMetallicRoughnessMaterial',
      create_pbr_specular_glossiness_material: 'createPBRSpecularGlossinessMaterial',
      create_unlit_material: 'createUnlitMaterial',
      create_background_material: 'createBackgroundMaterial',
      create_node_material: 'createNodeMaterial',
      create_shader_material: 'createShaderMaterial',
      create_multi_material: 'createMultiMaterial',
      create_cell_material: 'createCellMaterial',
      create_custom_material: 'createCustomMaterial',
      create_pbr_custom_material: 'createPBRCustomMaterial',
      create_simple_material: 'createSimpleMaterial',
      create_shadow_only_material: 'createShadowOnlyMaterial',
      create_sky_material: 'createSkyMaterial',
      create_water_material: 'createWaterMaterial',
      create_terrain_material: 'createTerrainMaterial',
      create_grid_material: 'createGridMaterial',
      create_triplanar_material: 'createTriPlanarMaterial',
      create_mix_material: 'createMixMaterial',
      create_lava_material: 'createLavaMaterial',
      create_fire_material: 'createFireMaterial',
      create_fur_material: 'createFurMaterial',
      create_gradient_material: 'createGradientMaterial',
      
      // === ALL TEXTURE TYPES ===
      create_texture: 'createTexture',
      create_cube_texture: 'createCubeTexture',
      create_hdr_cube_texture: 'createHDRCubeTexture',
      create_video_texture: 'createVideoTexture',
      create_mirror_texture: 'createMirrorTexture',
      create_refraction_texture: 'createRefractionTexture',
      create_depth_texture: 'createDepthTexture',
      
      // === PROCEDURAL TEXTURES ===
      create_procedural_texture: 'createProceduralTexture',
      create_noise_texture: 'createNoiseTexture',
      create_wood_texture: 'createWoodTexture',
      create_marble_texture: 'createMarbleTexture',
      create_fire_texture: 'createFireTexture',
      create_cloud_texture: 'createCloudTexture',
      create_grass_texture: 'createGrassTexture',
      create_road_texture: 'createRoadTexture',
      create_brick_texture: 'createBrickTexture',
      create_perlin_noise_texture: 'createPerlinNoiseTexture',
      create_normal_map_texture: 'createNormalMapTexture',
      
      // === ALL MESH BUILDERS ===
      create_box: 'createBox',
      create_sphere: 'createSphere',
      create_cylinder: 'createCylinder',
      create_plane: 'createPlane',
      create_ground: 'createGround',
      create_torus: 'createTorus',
      create_tube: 'createTube',
      create_ribbon: 'createRibbon',
      create_lathe: 'createLathe',
      create_extrusion: 'createExtrusion',
      create_polygon: 'createPolygon',
      create_icosphere: 'createIcosphere',
      create_capsule: 'createCapsule',
      create_text: 'createText',
      create_decal: 'createDecal',
      create_line_system: 'createLineSystem',
      create_dashed_lines: 'createDashedLines',
      create_trail: 'createTrail',
      
      // === ALL CAMERA TYPES ===
      is_camera: 'isCamera',
      set_camera_fov: 'setCameraFOV',
      get_camera_fov: 'getCameraFOV',
      set_camera_type: 'setCameraType',
      orbit_camera: 'orbitCamera',
      detach_camera_controls: 'detachCameraControls',
      attach_camera_controls: 'attachCameraControls',
      set_camera_target: 'setCameraTarget',
      get_camera_target: 'getCameraTarget',
      set_camera_radius: 'setCameraRadius',
      get_camera_radius: 'getCameraRadius',
      create_arc_rotate_camera: 'createArcRotateCamera',
      create_free_camera: 'createFreeCamera',
      create_universal_camera: 'createUniversalCamera',
      create_fly_camera: 'createFlyCamera',
      create_follow_camera: 'createFollowCamera',
      create_device_orientation_camera: 'createDeviceOrientationCamera',
      create_virtual_joysticks_camera: 'createVirtualJoysticksCamera',
      create_webvr_free_camera: 'createWebVRFreeCamera',
      create_vr_device_orientation_camera: 'createVRDeviceOrientationCamera',
      
      // === ALL LIGHT TYPES ===
      is_light: 'isLight',
      set_light_intensity: 'setLightIntensity',
      get_light_intensity: 'getLightIntensity',
      set_light_color: 'setLightColor',
      get_light_color: 'getLightColor',
      set_light_range: 'setLightRange',
      get_light_range: 'getLightRange',
      ensure_light: 'ensureLight',
      set_light_position: 'setLightPosition',
      set_light_direction: 'setLightDirection',
      set_light_specular: 'setLightSpecular',
      set_hemispheric_ground_color: 'setHemisphericGroundColor',
      set_scene_exposure: 'setSceneExposure',
      set_shadow_enabled: 'setShadowEnabled',
      set_shadow_darkness: 'setShadowDarkness',
      set_shadow_bias: 'setShadowBias',
      set_shadow_quality: 'setShadowQuality',
      set_shadow_softness: 'setShadowSoftness',
      create_directional_light: 'createDirectionalLight',
      create_hemispheric_light: 'createHemisphericLight',
      create_point_light: 'createPointLight',
      create_spot_light: 'createSpotLight',
      
      // === SKYBOX API ===
      ensure_skybox: 'ensureSkybox',
      set_skybox_colors: 'setSkyboxColors',
      set_skybox_texture: 'setSkyboxTexture',
      set_skybox_size: 'setSkyboxSize',
      set_skybox_enabled: 'setSkyboxEnabled',
      set_skybox_infinite: 'setSkyboxInfinite',
      
      // === PARTICLE SYSTEMS ===
      create_particle_system: 'createParticleSystem',
      create_gpu_particle_system: 'createGPUParticleSystem',
      create_solid_particle_system: 'createSolidParticleSystem',
      create_points_cloud_system: 'createPointsCloudSystem',
      start_particles: 'startParticles',
      stop_particles: 'stopParticles',
      set_particle_emission_rate: 'setParticleEmissionRate',
      set_particle_life_time: 'setParticleLifeTime',
      set_particle_size: 'setParticleSize',
      set_particle_color: 'setParticleColor',
      set_particle_velocity: 'setParticleVelocity',
      set_particle_gravity: 'setParticleGravity',
      set_particle_texture: 'setParticleTexture',
      
      // === POST-PROCESSING PIPELINES ===
      create_default_rendering_pipeline: 'createDefaultRenderingPipeline',
      create_ssao_rendering_pipeline: 'createSSAORenderingPipeline',
      create_ssao2_rendering_pipeline: 'createSSAO2RenderingPipeline',
      create_standard_rendering_pipeline: 'createStandardRenderingPipeline',
      create_lens_rendering_pipeline: 'createLensRenderingPipeline',
      add_post_process: 'addPostProcess',
      remove_post_process: 'removePostProcess',
      create_blur_post_process: 'createBlurPostProcess',
      create_black_and_white_post_process: 'createBlackAndWhitePostProcess',
      create_convolution_post_process: 'createConvolutionPostProcess',
      create_filter_post_process: 'createFilterPostProcess',
      create_fxaa_post_process: 'createFxaaPostProcess',
      create_highlights_post_process: 'createHighlightsPostProcess',
      create_refraction_post_process: 'createRefractionPostProcess',
      create_volumetric_light_post_process: 'createVolumetricLightPostProcess',
      create_color_correction_post_process: 'createColorCorrectionPostProcess',
      create_tonemap_post_process: 'createTonemapPostProcess',
      create_image_processing_post_process: 'createImageProcessingPostProcess',
      
      // === GUI 2D SYSTEM ===
      create_gui_texture: 'createGUITexture',
      create_gui_button: 'createGUIButton',
      create_gui_text_block: 'createGUITextBlock',
      create_gui_stack_panel: 'createGUIStackPanel',
      create_gui_rectangle: 'createGUIRectangle',
      create_gui_ellipse: 'createGUIEllipse',
      create_gui_line: 'createGUILine',
      create_gui_slider: 'createGUISlider',
      create_gui_checkbox: 'createGUICheckBox',
      create_gui_radio_button: 'createGUIRadioButton',
      create_gui_input_text: 'createGUIInputText',
      create_gui_password: 'createGUIPassword',
      create_gui_scroll_viewer: 'createGUIScrollViewer',
      create_gui_virtual_keyboard: 'createGUIVirtualKeyboard',
      create_gui_image: 'createGUIImage',
      
      // === GUI 3D SYSTEM ===
      create_gui3d_manager: 'createGUI3DManager',
      create_cylinder_panel: 'createCylinderPanel',
      create_plane_panel: 'createPlanePanel',
      create_sphere_panel: 'createSpherePanel',
      create_stack_panel_3d: 'createStackPanel3D',
      create_button_3d: 'createButton3D',
      create_holographic_button: 'createHolographicButton',
      create_mesh_button_3d: 'createMeshButton3D',
      
      // === XR/VR/AR SYSTEM ===
      create_webxr_default_experience: 'createWebXRDefaultExperience',
      create_webxr_experience_helper: 'createWebXRExperienceHelper',
      enable_webxr: 'enableWebXR',
      disable_webxr: 'disableWebXR',
      is_webxr_available: 'isWebXRAvailable',
      is_webxr_session_active: 'isWebXRSessionActive',
      get_webxr_controllers: 'getWebXRControllers',
      get_webxr_input_sources: 'getWebXRInputSources',
      teleport_in_xr: 'teleportInXR',
      enable_hand_tracking: 'enableHandTracking',
      disable_hand_tracking: 'disableHandTracking',
      
      // === BEHAVIOR SYSTEM ===
      add_auto_rotation_behavior: 'addAutoRotationBehavior',
      add_bouncing_behavior: 'addBouncingBehavior',
      add_framing_behavior: 'addFramingBehavior',
      add_attach_to_box_behavior: 'addAttachToBoxBehavior',
      add_fade_in_out_behavior: 'addFadeInOutBehavior',
      add_multi_pointer_scale_behavior: 'addMultiPointerScaleBehavior',
      add_pointer_drag_behavior: 'addPointerDragBehavior',
      add_six_dof_drag_behavior: 'addSixDofDragBehavior',
      remove_behavior: 'removeBehavior',
      get_behaviors: 'getBehaviors',
      
      // === GIZMO SYSTEM ===
      create_gizmo_manager: 'createGizmoManager',
      create_position_gizmo: 'createPositionGizmo',
      create_rotation_gizmo: 'createRotationGizmo',
      create_scale_gizmo: 'createScaleGizmo',
      create_bounding_box_gizmo: 'createBoundingBoxGizmo',
      enable_gizmos: 'enableGizmos',
      disable_gizmos: 'disableGizmos',
      
      // === LAYER SYSTEM ===
      create_layer: 'createLayer',
      create_highlight_layer: 'createHighlightLayer',
      create_glow_layer: 'createGlowLayer',
      create_effect_layer: 'createEffectLayer',
      add_to_highlight_layer: 'addToHighlightLayer',
      remove_from_highlight_layer: 'removeFromHighlightLayer',
      add_to_glow_layer: 'addToGlowLayer',
      remove_from_glow_layer: 'removeFromGlowLayer',
      
      // === SPRITE SYSTEM ===
      create_sprite: 'createSprite',
      create_sprite_manager: 'createSpriteManager',
      create_sprite_map: 'createSpriteMap',
      set_sprite_texture: 'setSpriteTexture',
      set_sprite_frame: 'setSpriteFrame',
      animate_sprite: 'animateSprite',
      dispose_sprite: 'disposeSprite',
      
      // === MORPH TARGET SYSTEM ===
      create_morph_target: 'createMorphTarget',
      create_morph_target_manager: 'createMorphTargetManager',
      add_morph_target: 'addMorphTarget',
      remove_morph_target: 'removeMorphTarget',
      set_morph_target_influence: 'setMorphTargetInfluence',
      get_morph_target_influence: 'getMorphTargetInfluence',
      
      // === NAVIGATION & CROWD ===
      create_navigation_mesh: 'createNavigationMesh',
      find_path: 'findPath',
      create_crowd: 'createCrowd',
      add_agent_to_crowd: 'addAgentToCrowd',
      remove_agent_from_crowd: 'removeAgentFromCrowd',
      set_agent_destination: 'setAgentDestination',
      get_agent_position: 'getAgentPosition',
      get_agent_velocity: 'getAgentVelocity',
      
      // === BAKED VERTEX ANIMATION ===
      create_baked_vertex_animation: 'createBakedVertexAnimation',
      bake_vertex_animation: 'bakeVertexAnimation',
      play_baked_animation: 'playBakedAnimation',
      
      // === COMPUTE SHADERS ===
      create_compute_shader: 'createComputeShader',
      create_compute_effect: 'createComputeEffect',
      dispatch_compute: 'dispatchCompute',
      set_compute_uniform: 'setComputeUniform',
      get_compute_buffer: 'getComputeBuffer',
      
      // === FLOW GRAPH SYSTEM ===
      create_flow_graph: 'createFlowGraph',
      add_flow_graph_block: 'addFlowGraphBlock',
      connect_flow_graph_nodes: 'connectFlowGraphNodes',
      execute_flow_graph: 'executeFlowGraph',
      
      // === FRAME GRAPH SYSTEM ===
      create_frame_graph: 'createFrameGraph',
      add_frame_graph_task: 'addFrameGraphTask',
      execute_frame_graph: 'executeFrameGraph',
      
      // === DEBUG & VISUALIZATION ===
      create_axes_viewer: 'createAxesViewer',
      create_bone_axes_viewer: 'createBoneAxesViewer',
      create_skeleton_viewer: 'createSkeletonViewer',
      create_physics_viewer: 'createPhysicsViewer',
      create_ray_helper: 'createRayHelper',
      enable_debug_layer: 'enableDebugLayer',
      disable_debug_layer: 'disableDebugLayer',
      show_world_axes: 'showWorldAxes',
      hide_world_axes: 'hideWorldAxes',
      
      // === ASSET LOADING ===
      load_mesh: 'loadMesh',
      load_gltf: 'loadGLTF',
      load_asset_container: 'loadAssetContainer',
      import_mesh: 'importMesh',
      append_scene: 'appendScene',
      create_assets_manager: 'createAssetsManager',
      add_mesh_task: 'addMeshTask',
      add_texture_task: 'addTextureTask',
      load_all_assets: 'loadAllAssets',
      merge_model_with_skeleton: 'mergeModelWithSkeleton',
      load_and_merge_assets: 'loadAndMergeAssets',
      get_loaded_asset: 'getLoadedAsset',
      get_loaded_mesh: 'getLoadedMesh',
      get_loaded_animations: 'getLoadedAnimations',
      get_loaded_skeleton: 'getLoadedSkeleton',
      
      // === SERIALIZATION ===
      serialize_scene: 'serializeScene',
      export_gltf: 'exportGLTF',
      export_obj: 'exportOBJ',
      export_stl: 'exportSTL',
      export_usdz: 'exportUSDZ',
      export_splat: 'exportSplat',
      
      // === AUDIO V2 SYSTEM ===
      play_sound: 'playSound',
      stop_sound: 'stopSound',
      set_sound_volume: 'setSoundVolume',
      create_sound: 'createSound',
      create_sound_track: 'createSoundTrack',
      create_spatial_sound: 'createSpatialSound',
      set_sound_position: 'setSoundPosition',
      set_sound_max_distance: 'setSoundMaxDistance',
      set_sound_rolloff_factor: 'setSoundRolloffFactor',
      create_audio_analyser: 'createAudioAnalyser',
      get_audio_frequency_data: 'getAudioFrequencyData',
      get_audio_time_data: 'getAudioTimeData',
      
      // === ALL EASING FUNCTIONS ===
      create_circle_ease: 'createCircleEase',
      create_back_ease: 'createBackEase',
      create_bounce_ease: 'createBounceEase',
      create_cubic_ease: 'createCubicEase',
      create_elastic_ease: 'createElasticEase',
      create_exponential_ease: 'createExponentialEase',
      create_power_ease: 'createPowerEase',
      create_quadratic_ease: 'createQuadraticEase',
      create_quartic_ease: 'createQuarticEase',
      create_quintic_ease: 'createQuinticEase',
      create_sine_ease: 'createSineEase',
      create_bezier_curve_ease: 'createBezierCurveEase',
      
      // === CSG OPERATIONS ===
      create_csg: 'createCSG',
      csg_union: 'csgUnion',
      csg_subtract: 'csgSubtract',
      csg_intersect: 'csgIntersect',
      csg_to_mesh: 'csgToMesh',
      
      // === INSTANCING ===
      create_instances: 'createInstances',
      create_thin_instances: 'createThinInstances',
      update_instance_data: 'updateInstanceData',
      dispose_instances: 'disposeInstances',
      get_instance_count: 'getInstanceCount',
      
      // === RENDERING OPTIMIZATION ===
      freeze_world_matrix: 'freezeWorldMatrix',
      unfreeze_world_matrix: 'unfreezeWorldMatrix',
      set_rendering_group: 'setRenderingGroup',
      get_rendering_group: 'getRenderingGroup',
      set_layer_mask: 'setLayerMask',
      get_layer_mask: 'getLayerMask',
      enable_edges: 'enableEdges',
      disable_edges: 'disableEdges',
      enable_outline: 'enableOutline',
      disable_outline: 'disableOutline',
      set_outline_color: 'setOutlineColor',
      set_outline_width: 'setOutlineWidth',
      
      // === ENVIRONMENT & HELPERS ===
      create_environment_helper: 'createEnvironmentHelper',
      create_photo_dome: 'createPhotoDome',
      create_video_dome: 'createVideoDome',
      create_texture_dome: 'createTextureDome',
      
      // === ADVANCED RENDERING ===
      enable_depth_renderer: 'enableDepthRenderer',
      enable_geometry_buffer_renderer: 'enableGeometryBufferRenderer',
      enable_outline_renderer: 'enableOutlineRenderer',
      enable_edges_renderer: 'enableEdgesRenderer',
      enable_bounding_box_renderer: 'enableBoundingBoxRenderer',
      create_utility_layer_renderer: 'createUtilityLayerRenderer',
      
      // === DYNAMIC PROPERTIES ===
      add_dynamic_property: 'addDynamicProperty',
      update_property_options: 'updatePropertyOptions',
      remove_dynamic_property: 'removeDynamicProperty',
      get_property_value: 'getPropertyValue',
      set_property_value: 'setPropertyValue'
    };

    const usedMethods = [];
    
    for (const [renscriptName, apiName] of Object.entries(apiMethods)) {
      if (this.usedFunctions.has(renscriptName)) {
        usedMethods.push(`  if (!api.${apiName}) throw new Error('RenScript API Error: Method "${apiName}" not found in API for function "${renscriptName}". Available methods: ' + Object.keys(api).join(', '));`);
        usedMethods.push(`  const ${renscriptName} = api.${apiName}.bind(api);`);
      }
    }

    return usedMethods.length > 0 ? 
      `  // SMART: Only binding methods actually used in script\n${usedMethods.join('\n')}\n` : 
      '  // No API methods used in this script\n';
  }

  generateUsedMathMethods() {
    const mathMethods = ['sin', 'cos', 'tan', 'abs', 'sqrt', 'pow', 'floor', 'ceil', 'round', 'atan2'];
    const usedMath = [];
    
    for (const method of mathMethods) {
      if (this.usedFunctions.has(method)) {
        usedMath.push(`  const ${method} = Math.${method};`);
      }
    }

    // Handle min/max specially since they use spread operator
    if (this.usedFunctions.has('min')) {
      usedMath.push('  const min = (...args) => Math.min(...args);');
    }
    if (this.usedFunctions.has('max')) {
      usedMath.push('  const max = (...args) => Math.max(...args);');
    }
    
    // Handle Math constants
    if (this.usedFunctions.has('PI')) {
      usedMath.push('  const PI = Math.PI;');
    }
    if (this.usedFunctions.has('E')) {
      usedMath.push('  const E = Math.E;');
    }
    
    // Handle additional utility functions
    if (this.usedFunctions.has('isNaN')) {
      usedMath.push('  const isNaN = Number.isNaN;');
    }
    if (this.usedFunctions.has('parseFloat')) {
      usedMath.push('  const parseFloat = Number.parseFloat;');
    }
    if (this.usedFunctions.has('parseInt')) {
      usedMath.push('  const parseInt = Number.parseInt;');
    }

    return usedMath.length > 0 ? 
      `\n  // Math functions\n${usedMath.join('\n')}\n` : 
      '';
  }

  generateUsedArrayMethods() {
    const arrayMethods = [];

    // Array methods - these are handled differently since they're member methods
    // We don't need to bind them like API methods since they're native JavaScript
    if (this.usedArrayMethods && this.usedArrayMethods.size > 0) {
      arrayMethods.push('  // Array methods are native JavaScript - no binding needed');
    }

    return arrayMethods.length > 0 ? 
      `\n  // Array support\n${arrayMethods.join('\n')}\n` : 
      '';
  }
  
  generateScript(script) {
    const variables = script.variables.map(v => this.generateVariableDeclaration(v)).join('\n    ');
    const methods = script.methods.map(m => this.generateMethod(m)).join(',\n\n  ');
    const functions = script.functions ? script.functions.map(f => this.generateFunction(f)).join(',\n\n  ') : '';
    const properties = script.properties || [];
    
    // SMART: Only bind functions that are actually used
    const usedApiMethods = this.generateUsedApiBindings();
    const usedMathMethods = this.generateUsedMathMethods();
    const usedArrayMethods = this.generateUsedArrayMethods();
    const efficientAPI = `${usedApiMethods}${usedMathMethods}${usedArrayMethods}`;
    
    // Generate properties metadata
    const propertiesMetadata = properties.length > 0 ? `
  // Script properties metadata
  _scriptProperties: [
${properties.map(p => `    {
      name: ${JSON.stringify(p.name)},
      type: ${JSON.stringify(p.propType)},
      section: ${JSON.stringify(p.section || 'General')},
      defaultValue: ${p.defaultValue ? this.generateExpression(p.defaultValue) : null},
      min: ${p.min ? this.generateExpression(p.min) : null},
      max: ${p.max ? this.generateExpression(p.max) : null},
      options: ${p.options ? this.generateExpression(p.options) : null},
      description: ${p.description ? this.generateExpression(p.description) : null},
      triggerOnce: ${p.triggerOnce || false}
    }`).join(',\n')}
  ],
  
  // Script object type metadata
  _scriptObjectType: ${JSON.stringify(script.objectType || 'script')},` : `
  // Script object type metadata  
  _scriptObjectType: ${JSON.stringify(script.objectType || 'script')},`;

    return `
function createRenScript(scene, api) {
${efficientAPI}
  
  const scriptInstance = {
    // Script variables
    ${variables}
    ${propertiesMetadata}
    
    ${methods}${functions ? ',\n\n  ' + functions : ''}
  };
  
  return scriptInstance;
}`;
  }
  
  generateVariableDeclaration(variable) {
    return `${variable.name}: ${this.generateExpression(variable.value)},`;
  }
  
  generateMethod(method) {
    const params = method.parameters.length > 0 ? method.parameters.join(', ') : '';
    const context = { parameters: method.parameters };
    const statements = method.statements.map(s => this.generateStatement(s, context)).join('\n    ');
    
    const methodName = method.methodType === 'start' ? 'onStart' :
                      method.methodType === 'update' ? 'onUpdate' :
                      method.methodType === 'destroy' ? 'onDestroy' :
                      method.methodType === 'once' ? 'onOnce' :
                      method.methodType;
    
    return `${methodName}(${params}) {
    ${statements}
  }`;
  }

  generateFunction(func) {
    const params = func.parameters.length > 0 ? func.parameters.join(', ') : '';
    const context = { parameters: func.parameters };
    const statements = func.statements.map(s => this.generateStatement(s, context)).join('\n    ');
    
    return `${func.name}(${params}) {
    ${statements}
  }`;
  }
  
  generateStatement(statement, context = {}) {
    switch (statement.type) {
      case 'Assignment':
        return `this.${statement.name} = ${this.generateExpression(statement.value, context)};`;
      case 'ExpressionStatement':
        return `${this.generateExpression(statement.expression, context)};`;
      case 'IfStatement':
        const condition = this.generateExpression(statement.condition, context);
        const thenBody = statement.thenStatements.map(s => this.generateStatement(s, context)).join('\n      ');
        const elseBody = statement.elseStatements ? 
          statement.elseStatements.map(s => this.generateStatement(s, context)).join('\n      ') : null;
        
        return elseBody ? 
          `if (${condition}) {\n      ${thenBody}\n    } else {\n      ${elseBody}\n    }` :
          `if (${condition}) {\n      ${thenBody}\n    }`;
      case 'ReturnStatement':
        return statement.value ? 
          `return ${this.generateExpression(statement.value, context)};` :
          'return;';
      case 'SwitchStatement':
        const discriminant = this.generateExpression(statement.discriminant, context);
        const cases = statement.cases.map(c => {
          const test = this.generateExpression(c.test, context);
          const body = c.statements.map(s => this.generateStatement(s, context)).join('\n        ');
          return `      case ${test}:\n        ${body}`;
        }).join('\n');
        return `switch (${discriminant}) {\n${cases}\n    }`;
      case 'BreakStatement':
        return 'break;';
      case 'ForStatement':
        const forInit = this.generateStatement(statement.init, context);
        const forCondition = this.generateExpression(statement.condition, context);
        const forUpdate = statement.update.type === 'UpdateExpression' ? 
          this.generateExpression(statement.update, context) :
          this.generateStatement(statement.update, context);
        const forBody = statement.statements.map(s => this.generateStatement(s, context)).join('\n      ');
        return `for (${forInit.replace(/;$/, '')}; ${forCondition}; ${forUpdate.replace(/;$/, '')}) {\n      ${forBody}\n    }`;
      default:
        throw new Error(`Unknown statement type: ${statement.type}`);
    }
  }
  
  generateExpression(expression, context = {}) {
    switch (expression.type) {
      case 'Literal':
        return typeof expression.value === 'string' ? 
               `"${expression.value.replace(/"/g, '\\"')}"` : 
               String(expression.value);
      case 'Identifier':
        // Check if it's a parameter or local variable
        if (context.parameters && context.parameters.includes(expression.name)) {
          return expression.name; // Don't prefix parameters with 'this.'
        }
        return `this.${expression.name}`;
      case 'BinaryExpression':
        return `(${this.generateExpression(expression.left, context)} ${expression.operator} ${this.generateExpression(expression.right, context)})`;
      case 'UnaryExpression':
        return `(${expression.operator}${this.generateExpression(expression.operand, context)})`;
      case 'CallExpression':
        let callee;
        if (expression.callee.type === 'Identifier') {
          const functionName = expression.callee.name;
          // Check if this is a custom function - prefix with 'this.'
          callee = this.customFunctions.has(functionName) ? `this.${functionName}` : functionName;
        } else if (expression.callee.type === 'MemberExpression') {
          // Track array method usage
          if (expression.callee.property && expression.callee.property.name) {
            const methodName = expression.callee.property.name;
            const arrayMethods = ['push', 'pop', 'shift', 'unshift', 'splice', 'slice', 'indexOf', 'join', 'concat', 'reverse', 'sort'];
            if (arrayMethods.includes(methodName)) {
              this.usedArrayMethods.add(methodName);
            }
          }
          callee = this.generateExpression(expression.callee, context);
        } else {
          callee = this.generateExpression(expression.callee, context);
        }
        const args = expression.arguments.map(arg => this.generateExpression(arg, context)).join(', ');
        return `${callee}(${args})`;
      case 'MemberExpression':
        // Track array property usage
        if (expression.property && expression.property.name) {
          const propertyName = expression.property.name;
          const arrayProperties = ['length'];
          if (arrayProperties.includes(propertyName)) {
            this.usedArrayMethods.add(propertyName);
          }
        }
        const object = this.generateExpression(expression.object, context);
        // For member expressions, don't prefix property names with 'this.'
        const property = expression.computed 
          ? this.generateExpression(expression.property, context)
          : expression.property.name;
        return expression.computed ? `${object}[${property}]` : `${object}.${property}`;
      case 'UpdateExpression':
        const argument = this.generateExpression(expression.argument, context);
        return expression.prefix ? 
          `${expression.operator}${argument}` : 
          `${argument}${expression.operator}`;
      case 'ArrayLiteral':
        const elements = expression.elements.map(element => this.generateExpression(element, context)).join(', ');
        return `[${elements}]`;
      case 'ObjectLiteral':
        const properties = expression.properties.map(prop => {
          return `${prop.key}: ${this.generateExpression(prop.value, context)}`;
        }).join(', ');
        return `{${properties}}`;
      default:
        throw new Error(`Unknown expression type: ${expression.type}`);
    }
  }
}

class RenScriptCompiler {
  static compile(source) {
    try {
      console.log('🔧 RenScript: Starting efficient compilation');
      
      // Validate input
      if (!source || typeof source !== 'string' || source.trim() === '') {
        throw new Error('Source code is empty or invalid');
      }
      
      // Tokenize with validation
      console.log('🔧 RenScript: Tokenizing source');
      const lexer = new RenScriptLexer(source);
      const tokens = lexer.tokenize();
      
      if (!tokens || tokens.length === 0) {
        throw new Error('No tokens generated from source');
      }
      
      console.log(`🔧 RenScript: Generated ${tokens.length} tokens`);
      
      // Parse with validation
      console.log('🔧 RenScript: Parsing tokens');
      const parser = new RenScriptParser(tokens);
      const ast = parser.parse();
      
      if (!ast) {
        throw new Error('Failed to generate AST');
      }
      
      console.log('🔧 RenScript: AST generated successfully');
      
      // Generate efficient code
      console.log('🔧 RenScript: Generating efficient JavaScript');
      const generator = new RenScriptCodeGenerator(ast);
      const jsCode = generator.generate();
      
      if (!jsCode) {
        throw new Error('Failed to generate JavaScript code');
      }
      
      console.log('🔧 RenScript: Compilation completed successfully');
      console.log(`🔧 RenScript: Generated ${jsCode.length} characters (efficient version)`);
      
      return jsCode;
      
    } catch (error) {
      console.error('🔧 RenScript: Compilation failed:', error.message);
      throw new Error(`RenScript compilation failed: ${error.message}`);
    }
  }
}

export { RenScriptCompiler };
