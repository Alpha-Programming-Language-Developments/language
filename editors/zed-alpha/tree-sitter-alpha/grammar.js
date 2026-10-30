/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const PREC = {
  COMMENT: 0,
  ASSIGN: 1,
  OR: 2,
  AND: 3,
  NOT: 4,
  RELATIONAL: 5,
  ADD: 6,
  MULTIPLY: 7,
  UNARY: 8,
  CALL: 9,
  MEMBER: 10,
};

module.exports = grammar({
  name: 'alpha',

  extras: $ => [
    /\s/,
    $.line_comment,
    $.block_comment,
  ],

  word: $ => $.identifier,

  conflicts: $ => [
    [$.tuple_pattern, $._expression],
  ],

  rules: {
    source_file: $ => repeat($._statement),

    _statement: $ => choice(
      $.import_statement,
      $.module_definition,
      $.enum_definition,
      $.structure_definition,
      $.function_definition,
      $.variable_declaration,
      $.assignment_statement,
      $.inc_dec_statement,
      $.if_statement,
      $.for_statement,
      $.schedule_statement,
      $.verify_statement,
      $.return_statement,
      $.try_catch_statement,
      $.system_statement,
      $.expression_statement,
      $.block,
      $.strict_directive,
      $.strict_directive,
    ),

    strict_directive: $ => '@strict',

    strict_directive: $ => '@strict',

    // Import statement: e.g. import Core/Basic.alpha or import "something"
    import_statement: $ => seq(
      'import',
      field('path', choice($.import_path, $.string)),
      optional(';'),
    ),

    import_path: $ => /[a-zA-Z0-9_\/.\-]+/,

    // Module / Namespace definition: module MathUtils { ... }
    module_definition: $ => seq(
      'module',
      field('name', $.identifier),
      '{',
      repeat(choice(
        $.function_definition,
        $.variable_declaration,
        $.structure_definition,
        $.enum_definition,
        $.module_definition,
      )),
      '}',
      optional(';'),
    ),

    // Enum definition: enum Status { Pending, Active, Done }
    enum_definition: $ => seq(
      'enum',
      field('name', $.identifier),
      '{',
      commaSep(field('variant', $.identifier)),
      optional(','),
      '}',
      optional(';'),
    ),

    // Structure definition
    structure_definition: $ => seq(
      'structure',
      field('name', $.identifier),
      '{',
      repeat(choice(
        $.structure_field,
        $.attached_method,
        $.attach_block,
      )),
      '}',
      optional(';'),
    ),

    structure_field: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $._type),
      optional(choice(',', ';')),
    ),

    attach_block: $ => seq(
      'attach',
      '{',
      repeat($.attached_method),
      '}',
      optional(';'),
    ),

    attached_method: $ => seq(
      'function',
      field('name', $.identifier),
      field('parameters', $.parameter_list),
      field('metadata', $.attached_method_metadata),
      field('body', $.block),
    ),

    attached_method_metadata: $ => seq(
      '[',
      commaSep1(choice(
        field('visibility', choice('public', 'here')),
        field('static', choice('static', 'nonstatic')),
        field('return_type', $._type),
      )),
      ']',
    ),

    // Function definition: function add(a: int) [public, int] { ... }
    function_definition: $ => seq(
      'function',
      field('name', $.identifier),
      field('parameters', $.parameter_list),
      field('metadata', $.function_metadata),
      field('body', $.block),
    ),

    parameter_list: $ => seq(
      '(',
      commaSep($.parameter),
      ')',
    ),

    parameter: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $._type),
    ),

    function_metadata: $ => seq(
      '[',
      commaSep1(choice(
        field('visibility', choice('public', 'here')),
        field('return_type', $._type),
      )),
      ']',
    ),

    // Variable declaration: var x: int = 10, const var MAX: int = 100, var (a, b) = pair
    variable_declaration: $ => seq(
      optional(choice('const', 'var', seq('const', 'var'))),
      choice('var', seq('const', 'var')),
      field('name', choice(
        $.identifier,
        $.tuple_pattern,
      )),
      optional(seq(':', field('type', $._type))),
      '=',
      field('value', $._expression),
      optional(';'),
    ),

    tuple_pattern: $ => seq(
      '(',
      commaSep1(choice(
        $.identifier,
        seq(field('name', $.identifier), ':', field('type', $._type)),
      )),
      ')',
    ),

    assignment_statement: $ => prec.right(PREC.ASSIGN, seq(
      field('left', choice(
        $.identifier,
        $.this,
        $.member_expression,
        $.index_expression,
        $.tuple_pattern,
      )),
      field('operator', choice('=', '+=', '-=', '*=', '/=', '%=')),
      field('right', $._expression),
      optional(';'),
    )),

    inc_dec_statement: $ => seq(
      field('target', choice($.identifier, $.this, $.member_expression, $.index_expression)),
      field('operator', choice('++', '--')),
      optional(';'),
    ),

    if_statement: $ => seq(
      'if',
      field('condition', $._expression),
      field('consequence', $.block),
      optional(seq(
        'else',
        field('alternative', choice($.block, $.if_statement)),
      )),
    ),

    // Defer: make schedule (func) / make schedule { block }
    schedule_statement: $ => seq(
      'make',
      'schedule',
      choice(
        field('body', $.block),
        seq('(', field('expression', $._expression), ')'),
      ),
      optional(';'),
    ),

    // Assertion: verify condition, "msg"
    verify_statement: $ => seq(
      'verify',
      field('condition', $._expression),
      optional(seq(',', field('message', $._expression))),
      optional(';'),
    ),

    check_arm: $ => seq(
      field('pattern', choice(
        'something',
        seq('(', 'something', ')'),
        'else',
        '_',
        $._expression,
      )),
      '=>',
      choice($.block, $._expression),
      optional(choice(',', ';')),
    ),

    for_statement: $ => choice(
      // Infinite loop: for { ... }
      seq('for', field('body', $.block)),

      // Repeat loop without variable: for 1...3 { ... } or for 1..3 { ... }
      seq(
        'for',
        field('start', $._expression),
        field('operator', choice('...', '..')),
        field('end', $._expression),
        field('body', $.block),
      ),

      // Range loop: for n in 1...3 { ... } or for n in 1..3 { ... }
      seq(
        'for',
        field('variable', $.identifier),
        'in',
        field('start', $._expression),
        field('operator', choice('...', '..')),
        field('end', $._expression),
        field('body', $.block),
      ),

      // Counter loop: for i = 0, i < 5, i++ { ... }
      seq(
        'for',
        field('initializer', choice(
          $.variable_declaration,
          seq(field('name', $.identifier), '=', field('value', $._expression)),
        )),
        ',',
        field('condition', $._expression),
        ',',
        field('update', choice(
          $.inc_dec_statement,
          seq(field('name', $.identifier), '=', field('value', $._expression)),
        )),
        field('body', $.block),
      ),

      // Iteration loop: for each item in list { ... }, for each k, v in map { ... }
      seq(
        'for',
        'each',
        field('variable', $.identifier),
        optional(seq(',', field('value_variable', $.identifier))),
        'in',
        field('iterable', $._expression),
        field('body', $.block),
      ),
    ),

    return_statement: $ => prec.right(seq(
      'return',
      optional(field('value', $._expression)),
      optional(';'),
    )),

    try_catch_statement: $ => seq(
      'try',
      field('try_block', $.block),
      'catch',
      field('error_variable', $.identifier),
      field('catch_block', $.block),
    ),

    system_statement: $ => seq(
      '!',
      'system',
      field('target', choice($.identifier, seq('(', $.identifier, ')'))),
      field('body', $.block),
    ),

    expression_statement: $ => seq(
      field('expression', $._expression),
      optional(';'),
    ),

    block: $ => seq(
      '{',
      repeat($._statement),
      '}',
    ),

    // Types
    _type: $ => choice(
      $.primitive_type,
      $.generic_type,
      $.union_type,
      $.tuple_type,
      $.custom_type,
    ),

    primitive_type: $ => choice(
      'int',
      'unt',
      'unt',
      'float',
      'string',
      'char',
      'boolean',
      'something',
      'nothing',
      'hexcolor',
    ),

    generic_type: $ => seq(
      field('name', choice('vect', 'map', $.identifier)),
      '<',
      commaSep1($._type),
      '>',
    ),

    union_type: $ => seq(
      '<',
      commaSep1($._type),
      '>',
    ),

    tuple_type: $ => seq(
      '(',
      commaSep1($._type),
      ')',
    ),

    custom_type: $ => prec.left(seq(
      $.identifier,
      repeat(seq('.', $.identifier)),
    )),

    // Expressions
    _expression: $ => choice(
      $.identifier,
      $.this,
      $.number,
      $.hexcolor_literal,
      $.string,
      $.multiline_string,
      $.interpolated_string,
      $.interpolated_multiline_string,
      $.char,
      $.boolean,
      $.nothing,
      $.make_expression,
      $.ternary_expression,
      $.cast_expression,
      $.force_unwrap_expression,
      $.check_expression,
      $.binary_expression,
      $.unary_expression,
      $.call_expression,
      $.member_expression,
      $.index_expression,
      $.tuple_expression,
      $.parenthesized_expression,
    ),

    this: $ => 'this',

    hexcolor_literal: $ => /#\([0-9a-fA-F]{3,8}\)/,

    make_expression: $ => seq(
      'make',
      choice('this', $.identifier),
      '{',
      commaSep(seq(field('name', $.identifier), ':', field('value', $._expression))),
      '}',
    ),

    ternary_expression: $ => prec.right(PREC.ASSIGN, choice(
      seq(
        'if',
        field('condition', $._expression),
        ':',
        field('consequence', $._expression),
        'else',
        field('alternative', $._expression),
      ),
      seq(
        field('condition', $._expression),
        '?',
        field('consequence', $._expression),
        ':',
        field('alternative', $._expression),
      ),
    )),

    cast_expression: $ => seq(
      '(',
      field('value', $._expression),
      '->',
      field('type', $._type),
      ')',
    ),

    force_unwrap_expression: $ => prec.left(PREC.CALL, seq(
      field('argument', $._expression),
      choice('!', '!?'),
    )),

    check_expression: $ => seq(
      'check',
      field('target', $._expression),
      '{',
      repeat($.check_arm),
      '}',
    ),

    tuple_expression: $ => seq(
      '(',
      $._expression,
      ',',
      commaSep($._expression),
      ')',
    ),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    index_expression: $ => prec(PREC.MEMBER, seq(
      field('array', choice($.identifier, $.this, $.member_expression, $.call_expression, $.index_expression)),
      '[',
      field('index', choice(
        $._expression,
        $.slice_expression,
      )),
      ']',
    )),

    slice_expression: $ => choice(
      seq(
        field('start', $._expression),
        field('operator', choice('...', '..')),
        optional(field('end', $._expression)),
      ),
      seq(
        field('operator', choice('...', '..')),
        field('end', $._expression),
      ),
    ),

    binary_expression: $ => {
      const table = [
        ['or', PREC.OR],
        ['||', PREC.OR],
        ['and', PREC.AND],
        ['&&', PREC.AND],
        ['==', PREC.RELATIONAL],
        ['!=', PREC.RELATIONAL],
        ['<', PREC.RELATIONAL],
        ['<=', PREC.RELATIONAL],
        ['>', PREC.RELATIONAL],
        ['>=', PREC.RELATIONAL],
        ['?:', PREC.ASSIGN],
        ['+', PREC.ADD],
        ['-', PREC.ADD],
        ['*', PREC.MULTIPLY],
        ['/', PREC.MULTIPLY],
        ['%', PREC.MULTIPLY],
      ];

      return choice(...table.map(([op, precedence]) =>
        prec.left(precedence, seq(
          field('left', $._expression),
          field('operator', op),
          field('right', $._expression),
        ))
      ));
    },

    unary_expression: $ => {
      const table = [
        ['not', PREC.NOT],
        ['!', PREC.NOT],
        ['-', PREC.UNARY],
        ['+', PREC.UNARY],
      ];

      return choice(...table.map(([op, precedence]) =>
        prec.right(precedence, seq(
          field('operator', op),
          field('argument', $._expression),
        ))
      ));
    },

    call_expression: $ => prec(PREC.CALL, seq(
      field('function', choice($.identifier, $.this, $.member_expression)),
      field('arguments', $.argument_list),
    )),

    argument_list: $ => seq(
      '(',
      commaSep(choice($._expression, $.block)),
      ')',
    ),

    member_expression: $ => prec(PREC.MEMBER, seq(
      field('object', choice($.identifier, $.this, $.member_expression, $.call_expression)),
      '.',
      field('property', $.identifier),
    )),

    // Literals and Tokens
    identifier: $ => /[a-zA-Z_?][a-zA-Z0-9_?]*/,

    number: $ => /-?[0-9]+(\.[0-9]+)?/,

    string: $ => seq(
      '"',
      repeat(choice(
        /[^"\\\n]+/,
        /\\./,
      )),
      '"',
    ),

    multiline_string: $ => seq(
      '"""',
      repeat(choice(
        /[^"\\]+/,
        /\\./,
        /"[^"]/,
        /""[^"]/,
      )),
      '"""',
    ),

    interpolated_string: $ => seq(
      '$"',
      repeat(choice(
        /[^"\\\n]+/,
        /\\./,
      )),
      '"',
    ),

    interpolated_multiline_string: $ => seq(
      '$"""',
      repeat(choice(
        /[^"\\]+/,
        /\\./,
        /"[^"]/,
        /""[^"]/,
      )),
      '"""',
    ),

    char: $ => seq(
      "'",
      choice(
        /[^'\\\n]/,
        /\\./,
      ),
      "'",
    ),

    boolean: $ => choice('true', 'false'),

    nothing: $ => 'nothing',

    line_comment: $ => token(seq('//', /.*/)),

    block_comment: $ => token(seq(
      '/*',
      /[^*]*\*+([^/*][^*]*\*+)*/,
      '/',
    )),
  },
});

function commaSep(rule) {
  return optional(commaSep1(rule));
}

function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}
