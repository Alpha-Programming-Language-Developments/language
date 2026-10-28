; Keywords
"structure" @keyword
"attach" @keyword
"static" @keyword
"nonstatic" @keyword
"make" @keyword
"check" @keyword.control
"function" @keyword.function
"var" @keyword
"const" @keyword
"import" @keyword.control.import
"return" @keyword.control.return
"if" @keyword.control
"else" @keyword.control
"for" @keyword.control
"each" @keyword.control
"in" @keyword.control
"try" @keyword.control
"catch" @keyword.control
"system" @keyword.control
"public" @keyword
"here" @keyword
(this) @variable.language

; Logical Operators as Keywords
"and" @keyword.operator
"or" @keyword.operator
"not" @keyword.operator

; Comments
(line_comment) @comment
(block_comment) @comment

; Types
(primitive_type) @type.builtin
(custom_type) @type
(generic_type name: (_) @type)
(structure_definition name: (identifier) @type)

; Literals
(string) @string
(multiline_string) @string
(interpolated_string) @string
(interpolated_multiline_string) @string
(char) @character
(number) @number
(boolean) @boolean
(nothing) @constant.builtin
(import_path) @string.special.path

; Functions & Methods
(function_definition
  name: (identifier) @function)

(attached_method
  name: (identifier) @function.method)

(call_expression
  function: (identifier) @function.call)

(call_expression
  function: (member_expression
    property: (identifier) @function.method))

; Parameters & Variables
(parameter
  name: (identifier) @variable.parameter)

(variable_declaration
  name: (identifier) @variable)

(structure_field
  name: (identifier) @property)

(try_catch_statement
  error_variable: (identifier) @variable)

(member_expression
  property: (identifier) @property)

; Operators
[
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "&&"
  "||"
  "!"
  "!?"
  "->"
  "=>"
  "..."
  ".."
  "+"
  "-"
  "*"
  "/"
  "%"
  "++"
  "--"
] @operator

; Delimiters & Punctuation
[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ","
  ":"
  ";"
  "."
] @punctuation.delimiter
