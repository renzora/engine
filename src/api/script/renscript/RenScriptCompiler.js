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
    
    while (!this.check('RBRACE') && !this.isAtEnd()) {
      if (this.check('PROPS')) {
        properties.push(...this.propsDeclaration());
      } else if (this.check('IDENTIFIER') && this.peekNext()?.type === 'ASSIGN') {
        variables.push(this.variableDeclaration());
      } else if (this.check('START') || this.check('UPDATE') || this.check('DESTROY') || this.check('ONCE')) {
        methods.push(this.methodDeclaration());
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
  
  statement() {
    if (this.check('IF')) {
      return this.ifStatement();
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
    this.consume('LBRACE', "Expected '{' after if condition");
    
    const thenStatements = [];
    while (!this.check('RBRACE') && !this.isAtEnd()) {
      thenStatements.push(this.statement());
    }
    this.consume('RBRACE', "Expected '}' after if body");
    
    let elseStatements = null;
    if (this.match('ELSE')) {
      this.consume('LBRACE', "Expected '{' after 'else'");
      elseStatements = [];
      while (!this.check('RBRACE') && !this.isAtEnd()) {
        elseStatements.push(this.statement());
      }
      this.consume('RBRACE', "Expected '}' after else body");
    }
    
    return {
      type: 'IfStatement',
      condition,
      thenStatements,
      elseStatements
    };
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
    this.usedFunctions = new Set();
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
    // API method mappings (camelCase -> snake_case)
    const apiMethods = {
      log: 'log',
      get_position: 'getPosition',
      set_position: 'setPosition',
      get_world_position: 'getWorldPosition',
      get_rotation: 'getRotation',
      set_rotation: 'setRotation',
      get_world_rotation: 'getWorldRotationQuaternion',
      rotate_by: 'rotateBy',
      move_by: 'moveBy',
      move_to: 'setPosition',
      get_scale: 'getScale',
      set_scale: 'setScale',
      is_visible: 'isVisible',
      set_visible: 'setVisible',
      set_color: 'setColor',
      get_color: 'getColor',
      set_emissive_color: 'setEmissiveColor',
      get_emissive_color: 'getEmissiveColor',
      set_material_property: 'setMaterialProperty',
      get_material_property: 'getMaterialProperty',
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
      set_alpha: 'setAlpha',
      set_specular_power: 'setSpecularPower',
      set_diffuse_color: 'setDiffuseColor',
      set_specular_color: 'setSpecularColor',
      set_ambient_color: 'setAmbientColor',
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
      add_tag: 'addTag',
      remove_tag: 'removeTag',
      has_tag: 'hasTag'
    };

    const usedMethods = [];
    
    for (const [renscriptName, apiName] of Object.entries(apiMethods)) {
      if (this.usedFunctions.has(renscriptName)) {
        usedMethods.push(`  const ${renscriptName} = api.${apiName}.bind(api);`);
      }
    }

    return usedMethods.length > 0 ? 
      `  // SMART: Only binding methods actually used in script\n${usedMethods.join('\n')}\n` : 
      '  // No API methods used in this script\n';
  }

  generateUsedMathMethods() {
    const mathMethods = ['sin', 'cos', 'tan', 'abs', 'sqrt', 'pow', 'floor', 'ceil', 'round'];
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

    return usedMath.length > 0 ? 
      `\n  // Math functions\n${usedMath.join('\n')}\n` : 
      '';
  }
  
  generateScript(script) {
    const variables = script.variables.map(v => this.generateVariableDeclaration(v)).join('\n    ');
    const methods = script.methods.map(m => this.generateMethod(m)).join(',\n\n  ');
    const properties = script.properties || [];
    
    // SMART: Only bind functions that are actually used
    const usedApiMethods = this.generateUsedApiBindings();
    const usedMathMethods = this.generateUsedMathMethods();
    const efficientAPI = `${usedApiMethods}${usedMathMethods}`;
    
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
                      method.methodType === 'once' ? 'onOnce' :
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
      case 'IfStatement':
        const condition = this.generateExpression(statement.condition, context);
        const thenBody = statement.thenStatements.map(s => this.generateStatement(s, context)).join('\n      ');
        const elseBody = statement.elseStatements ? 
          statement.elseStatements.map(s => this.generateStatement(s, context)).join('\n      ') : null;
        
        return elseBody ? 
          `if (${condition}) {\n      ${thenBody}\n    } else {\n      ${elseBody}\n    }` :
          `if (${condition}) {\n      ${thenBody}\n    }`;
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
