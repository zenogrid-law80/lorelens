(identifier) @variable

(class_declaration name: (identifier) @type)
(interface_declaration name: (identifier) @type)
(struct_declaration (identifier) @type)
(enum_declaration name: (identifier) @type)
(namespace_declaration name: (identifier) @module)
(method_declaration name: (identifier) @function)
(local_function_statement name: (identifier) @function)
(constructor_declaration name: (identifier) @constructor)
(predefined_type) @type.builtin

[(integer_literal) (real_literal)] @number
[(character_literal) (string_literal) (verbatim_string_literal) (raw_string_literal) (interpolated_string_expression)] @string
[(boolean_literal) (null_literal)] @constant.builtin
(comment) @comment

[(modifier) (implicit_type) "this"] @keyword
