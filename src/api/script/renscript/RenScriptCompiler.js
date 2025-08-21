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
      
      // Comments
      if (char === '#') {
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
        case '+': this.addToken('PLUS'); break;
        case '-': this.addToken('MINUS'); break;
        case '*': this.addToken('MULTIPLY'); break;
        case '/': this.addToken('DIVIDE'); break;
        case '=': this.addToken('ASSIGN'); break;
        case ';': this.addToken('SEMICOLON'); break;
        case ':': this.addToken('COLON'); break;
        case '?': this.addToken('QUESTION'); break;
        default:
          throw new Error(`Unexpected character '${char}' at line ${this.line}, column ${this.column}`);
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
      'props': 'PROPS',
      'start': 'START',
      'update': 'UPDATE',
      'destroy': 'DESTROY',
      'if': 'IF',
      'else': 'ELSE',
      'while': 'WHILE',
      'for': 'FOR',
      'true': 'TRUE',
      'false': 'FALSE',
      'null': 'NULL'
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
    this.consume('SCRIPT', "Expected 'script'");
    const name = this.consume('IDENTIFIER', "Expected script name").value;
    this.consume('LBRACE', "Expected '{'");
    
    const variables = [];
    const methods = [];
    const properties = [];
    
    while (!this.check('RBRACE') && !this.isAtEnd()) {
      if (this.check('PROPS')) {
        properties.push(...this.propsDeclaration());
      } else if (this.check('IDENTIFIER') && this.peekNext()?.type === 'ASSIGN') {
        variables.push(this.variableDeclaration());
      } else if (this.check('START') || this.check('UPDATE') || this.check('DESTROY')) {
        methods.push(this.methodDeclaration());
      } else {
        throw new Error(`Unexpected token ${this.peek().type} at line ${this.peek().line}`);
      }
    }
    
    this.consume('RBRACE', "Expected '}'");
    
    return {
      type: 'Script',
      name,
      variables,
      methods,
      properties
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
    
    let defaultValue = null;
    let min = null;
    let max = null;
    let options = null;
    let description = null;
    
    // Parse property options
    if (this.match('LBRACE')) {
      while (!this.check('RBRACE') && !this.isAtEnd()) {
        const optionName = this.consume('IDENTIFIER', "Expected option name").value;
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
      description
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
    const methodType = this.advance().type; // START, UPDATE, or DESTROY
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
  
  statement() {
    if (this.check('IDENTIFIER') && this.peekNext()?.type === 'ASSIGN') {
      return this.assignment();
    }
    
    return this.expressionStatement();
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
    return this.addition();
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
    if (this.match('MINUS')) {
      const operator = this.previous().type;
      const right = this.unary();
      return {
        type: 'UnaryExpression',
        operator: '-',
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
      return this.arrayLiteral();
    }
    
    throw new Error(`Unexpected token ${this.peek().type} at line ${this.peek().line}`);
  }
  
  arrayLiteral() {
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
  }
  
  generate() {
    return this.generateScript(this.ast);
  }
  
  generateScript(script) {
    const variables = script.variables.map(v => this.generateVariableDeclaration(v)).join('\n  ');
    const methods = script.methods.map(m => this.generateMethod(m)).join(',\n\n  ');
    const properties = script.properties || [];
    
    // Create the API function mapping
    const apiFunctions = `
  // === Core Transform API ===
  const log = (...args) => api.log(...args);
  const get_position = () => api.getPosition();
  const set_position = (x, y, z) => api.setPosition(x, y, z);
  const get_world_position = () => api.getWorldPosition();
  const get_rotation = () => api.getRotation();
  const set_rotation = (x, y, z) => api.setRotation(x, y, z);
  const get_world_rotation = () => api.getWorldRotationQuaternion();
  const rotate_by = (x, y, z) => api.rotateBy(x, y, z);
  const move_by = (x, y, z) => api.moveBy(x, y, z);
  const move_to = (x, y, z) => api.setPosition(x, y, z);
  const look_at = (target, up) => api.lookAt(target, up);
  const get_scale = () => api.getScale();
  const set_scale = (x, y, z) => api.setScale(x, y, z);
  
  // === Visibility & Material API ===
  const is_visible = () => api.isVisible();
  const set_visible = (visible) => api.setVisible(visible);
  const set_color = (r, g, b) => api.setColor(r, g, b);
  const get_color = () => api.getColor();
  const set_emissive_color = (r, g, b) => api.setEmissiveColor(r, g, b);
  const get_emissive_color = () => api.getEmissiveColor();
  const set_material_property = (property, value) => api.setMaterialProperty(property, value);
  const get_material_property = (property) => api.getMaterialProperty(property);
  
  // === Animation API ===
  const animate = (property, targetValue, duration, easing) => api.animate(property, targetValue, duration, easing);
  const stop_animation = () => api.stopAnimation();
  const pause_animation = () => api.pauseAnimation();
  const resume_animation = () => api.resumeAnimation();
  
  // === Physics API ===
  const set_physics_impostor = (type, options) => api.setPhysicsImpostor(type, options);
  const apply_impulse = (force, contactPoint) => api.applyImpulse(force, contactPoint);
  const set_linear_velocity = (velocity) => api.setLinearVelocity(velocity);
  const set_angular_velocity = (velocity) => api.setAngularVelocity(velocity);
  
  // === Scene Query API ===
  const find_object = (name) => api.findObjectByName(name);
  const find_objects_by_tag = (tag) => api.findObjectsByTag(tag);
  const raycast = (direction, maxDistance, excludeObjects) => api.raycast(direction, maxDistance, excludeObjects);
  const get_objects_in_radius = (radius, objectTypes) => api.getObjectsInRadius(radius, objectTypes);
  
  // === Audio API ===
  const play_sound = (soundPath, options) => api.playSound(soundPath, options);
  const stop_sound = (sound) => api.stopSound(sound);
  const set_sound_volume = (sound, volume) => api.setSoundVolume(sound, volume);
  
  // === Input API ===
  const is_key_pressed = (key) => api.isKeyPressed(key);
  const is_mouse_button_pressed = (button) => api.isMouseButtonPressed(button);
  const get_mouse_position = () => api.getMousePosition();
  
  // === Time API ===
  const get_time = () => api.getTime();
  const get_delta_time = () => api.getDeltaTime();
  
  // === Object Management API ===
  const clone_object = (name, position) => api.clone(name, position);
  const dispose_object = () => api.dispose();
  const set_metadata = (key, value) => api.setMetadata(key, value);
  const get_metadata = (key) => api.getMetadata(key);
  const add_tag = (tag) => api.addTag(tag);
  const remove_tag = (tag) => api.removeTag(tag);
  const has_tag = (tag) => api.hasTag(tag);
  
  // === Math Functions ===
  const sin = Math.sin;
  const cos = Math.cos;
  const tan = Math.tan;
  const abs = Math.abs;
  const sqrt = Math.sqrt;
  const pow = Math.pow;
  const min = (...args) => Math.min(...args);
  const max = (...args) => Math.max(...args);
  const floor = Math.floor;
  const ceil = Math.ceil;
  const round = Math.round;
  const random = (min, max) => api.random(min, max);
  const clamp = (value, min, max) => api.clamp(value, min, max);
  const lerp = (start, end, t) => api.lerp(start, end, t);
  const to_radians = (degrees) => api.toRadians(degrees);
  const to_degrees = (radians) => api.toDegrees(radians);
  const distance = (pos1, pos2) => api.distance(pos1, pos2);
  const normalize = (vector) => api.normalize(vector);
  const dot = (vec1, vec2) => api.dot(vec1, vec2);
  const cross = (vec1, vec2) => api.cross(vec1, vec2);`;
    
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
      description: ${p.description ? this.generateExpression(p.description) : null}
    }`).join(',\n')}
  ],` : '';

    return `
function createRenScript(scene, api) {
${apiFunctions}
  
  return {
    // Script variables
    ${variables}
    ${propertiesMetadata}
    
    ${methods}
  };
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
                      method.methodType;
    
    return `${methodName}(${params}) {
    ${statements}
  }`;
  }
  
  generateStatement(statement, context = {}) {
    switch (statement.type) {
      case 'Assignment':
        return `this.${statement.name} = ${this.generateExpression(statement.value, context)};`;
      case 'ExpressionStatement':
        return `${this.generateExpression(statement.expression, context)};`;
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
        const callee = expression.callee.type === 'Identifier' ? expression.callee.name : this.generateExpression(expression.callee, context);
        const args = expression.arguments.map(arg => this.generateExpression(arg, context)).join(', ');
        return `${callee}(${args})`;
      case 'ArrayLiteral':
        const elements = expression.elements.map(element => this.generateExpression(element, context)).join(', ');
        return `[${elements}]`;
      default:
        throw new Error(`Unknown expression type: ${expression.type}`);
    }
  }
}

class RenScriptCompiler {
  static compile(source) {
    try {
      console.log('🔧 RenScript: Compiling source');
      
      // Tokenize
      const lexer = new RenScriptLexer(source);
      const tokens = lexer.tokenize();
      console.log('🔧 RenScript: Tokens generated:', tokens.length);
      
      // Parse
      const parser = new RenScriptParser(tokens);
      const ast = parser.parse();
      console.log('🔧 RenScript: AST generated:', ast);
      
      // Generate code
      const generator = new RenScriptCodeGenerator(ast);
      const jsCode = generator.generate();
      console.log('🔧 RenScript: JavaScript generated');
      
      return jsCode;
      
    } catch (error) {
      throw new Error(`RenScript compilation failed: ${error.message}`);
    }
  }
}

export { RenScriptCompiler };