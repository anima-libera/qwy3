use std::{
	collections::{HashMap, VecDeque},
	ops::Deref,
};

use enum_iterator::Sequence;

/// A type in the language.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Type {
	Nothing,
	Integer,
	Function(FunctionTypeSignature),
	Type,
}

/// A value in the language.
#[derive(Clone)]
pub enum Value {
	Nothing,
	Integer(i32),
	Function(Function),
	Type(Type),
}

impl Value {
	fn get_type(&self) -> Type {
		match self {
			Value::Nothing => Type::Nothing,
			Value::Integer(_) => Type::Integer,
			Value::Function(Function { signature, .. }) => Type::Function(signature.clone()),
			Value::Type(_) => Type::Type,
		}
	}
}

/// Constraints on a type.
/// Function signatures present such constraints for the argument types instead of directly types,
/// so that functions such as `type_of` can take a value of any type as its argument.
#[derive(Clone, PartialEq, Eq, Debug)]
enum TypeConstraints {
	/// Only one type satisfy the constraints.
	Only(Type),
	/// Any type can do.
	Any,
}

impl TypeConstraints {
	fn is_satisfied_by_type(&self, some_type: &Type) -> bool {
		match self {
			TypeConstraints::Only(expected_type) => expected_type == some_type,
			TypeConstraints::Any => true,
		}
	}
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FunctionTypeSignature {
	arg_types: Vec<TypeConstraints>,
	/// The returned type is really a type and not a type constraint to make sure that
	/// expressions can all be typed.
	return_type: Box<Type>,
}

#[derive(Clone, Copy, Sequence)]
enum BuiltInFunctionBody {
	PrintInteger,
	PrintThreeIntegers,
	ToType,
	PrintType,
}

impl BuiltInFunctionBody {
	fn evaluate(self, arg_values: Vec<Value>) -> Value {
		match self {
			BuiltInFunctionBody::PrintInteger => {
				let integer_value = match arg_values[0] {
					Value::Integer(integer_value) => integer_value,
					_ => todo!(),
				};
				println!("printing integer {integer_value}",);
				Value::Nothing
			},
			BuiltInFunctionBody::PrintThreeIntegers => {
				let integer_values: Vec<_> = arg_values
					.iter()
					.map(|arg| match arg {
						Value::Integer(integer_value) => integer_value,
						_ => todo!(),
					})
					.collect();
				println!("printing three integers {integer_values:?}",);
				Value::Nothing
			},
			BuiltInFunctionBody::ToType => {
				let type_value = arg_values[0].get_type();
				Value::Type(type_value)
			},
			BuiltInFunctionBody::PrintType => {
				let type_value = match arg_values.into_iter().next().unwrap() {
					Value::Type(type_value) => type_value,
					_ => todo!(),
				};
				println!("printing type {type_value:?}",);
				Value::Nothing
			},
		}
	}

	fn default_name(self) -> &'static str {
		match self {
			BuiltInFunctionBody::PrintInteger => "print_integer",
			BuiltInFunctionBody::PrintThreeIntegers => "print_three_integers",
			BuiltInFunctionBody::ToType => "type_of",
			BuiltInFunctionBody::PrintType => "print_type",
		}
	}

	fn function_type_signature(self) -> FunctionTypeSignature {
		match self {
			BuiltInFunctionBody::PrintInteger => FunctionTypeSignature {
				arg_types: vec![TypeConstraints::Only(Type::Integer)],
				return_type: Box::new(Type::Nothing),
			},
			BuiltInFunctionBody::PrintThreeIntegers => FunctionTypeSignature {
				arg_types: vec![
					TypeConstraints::Only(Type::Integer),
					TypeConstraints::Only(Type::Integer),
					TypeConstraints::Only(Type::Integer),
				],
				return_type: Box::new(Type::Nothing),
			},
			BuiltInFunctionBody::ToType => FunctionTypeSignature {
				arg_types: vec![TypeConstraints::Any],
				return_type: Box::new(Type::Type),
			},
			BuiltInFunctionBody::PrintType => FunctionTypeSignature {
				arg_types: vec![TypeConstraints::Only(Type::Type)],
				return_type: Box::new(Type::Nothing),
			},
		}
	}

	fn function(self) -> Function {
		Function {
			signature: self.function_type_signature(),
			body: FunctionBody::BuiltIn(self),
		}
	}
}

#[derive(Clone)]
enum FunctionBody {
	BuiltIn(BuiltInFunctionBody),
	Expression(Box<Expression>),
}

#[derive(Clone)]
pub struct Function {
	signature: FunctionTypeSignature,
	body: FunctionBody,
}

#[derive(Clone)]
enum Expression {
	Const(Value),
	Variable(String),
	FunctionCall { func: Box<Expression>, args: Vec<Expression> },
}

enum ExpressionTypingError {
	FunctionCallOnNotAFunction,
	FunctionCallOnErroneousType,
	UnknownVariable,
}

impl Expression {
	fn get_type(&self, type_context: &TypeContext) -> Result<Type, ExpressionTypingError> {
		match self {
			Expression::Const(value) => Ok(value.get_type()),
			Expression::Variable(name) => {
				if let Some(variable_type) = type_context.variables.get(name) {
					Ok(variable_type.clone())
				} else {
					Err(ExpressionTypingError::UnknownVariable)
				}
			},
			Expression::FunctionCall { func, .. } => {
				let func_type = func.get_type(type_context);
				match func_type {
					Ok(Type::Function(signature)) => Ok(signature.return_type.deref().clone()),
					Err(_) => Err(ExpressionTypingError::FunctionCallOnErroneousType),
					Ok(_) => Err(ExpressionTypingError::FunctionCallOnNotAFunction),
				}
			},
		}
	}
}

pub struct Context {
	variables: HashMap<String, Value>,
}
pub struct TypeContext {
	variables: HashMap<String, Type>,
}

impl Context {
	pub fn with_builtins() -> Context {
		let mut variables = HashMap::new();
		for built_in_function_body in enum_iterator::all::<BuiltInFunctionBody>() {
			variables.insert(
				built_in_function_body.default_name().to_string(),
				Value::Function(built_in_function_body.function()),
			);
		}
		Context { variables }
	}

	fn get_type_context(&self) -> TypeContext {
		let mut variables = HashMap::new();
		for (name, value) in self.variables.iter() {
			variables.insert(name.to_owned(), value.get_type());
		}
		TypeContext { variables }
	}
}

#[derive(Clone, Debug)]
pub enum Token {
	Word(String),
	Integer(i32),
	OpenParenthesis,
	CloseParenthesis,
	Comma,
	Semicolon,
}

fn tokenize(code: &str) -> Vec<Token> {
	let mut tokens = vec![];
	let mut chars = code.chars().peekable();

	while chars.peek().is_some() {
		match chars.peek().copied() {
			None => break,
			Some(c) if c.is_whitespace() => {
				chars.next();
			},
			Some(c) if c.is_ascii_alphabetic() || c == '_' => {
				let mut word = String::new();
				while chars
					.peek()
					.copied()
					.is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
				{
					word.push(chars.next().unwrap());
				}
				tokens.push(Token::Word(word));
			},
			Some(c) if c.is_ascii_digit() => {
				let mut value = 0;
				while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
					value = value * 10 + chars.next().unwrap() as i32 - '0' as i32;
				}
				tokens.push(Token::Integer(value));
			},
			Some('(') => {
				chars.next();
				tokens.push(Token::OpenParenthesis);
			},
			Some(')') => {
				chars.next();
				tokens.push(Token::CloseParenthesis);
			},
			Some(',') => {
				chars.next();
				tokens.push(Token::Comma);
			},
			Some(';') => {
				chars.next();
				tokens.push(Token::Semicolon);
			},
			_ => todo!(),
		}
	}
	tokens
}

#[derive(Debug)]
pub enum ExpressionParsingError {
	NoTokens,
	UnexpectedToken(Token),
	ErroneousType,
	FunctionCallOnNotAFunction,
	FunctionCallTypeCheckError(FunctionCallTypeCheckError),
}

#[derive(Debug)]
pub enum FunctionCallTypeCheckError {
	WrongNumberOfArguments,
	ArgumentOfErroneousType,
	ArgumentOfTheWrongType,
}

fn check_function_call_argument_types(
	function_type_signature: FunctionTypeSignature,
	args: &[Expression],
	type_context: &TypeContext,
) -> Result<(), FunctionCallTypeCheckError> {
	let expected_arg_count = function_type_signature.arg_types.len();
	let actual_arg_count = args.len();
	if expected_arg_count != actual_arg_count {
		return Err(FunctionCallTypeCheckError::WrongNumberOfArguments);
	}
	for (arg_i, arg) in args.iter().enumerate() {
		let type_constraints = &function_type_signature.arg_types[arg_i];
		let actual_type = match arg.get_type(type_context) {
			Ok(actual_type) => actual_type,
			Err(_) => return Err(FunctionCallTypeCheckError::ArgumentOfErroneousType),
		};
		if !type_constraints.is_satisfied_by_type(&actual_type) {
			return Err(FunctionCallTypeCheckError::ArgumentOfTheWrongType);
		}
	}
	Ok(())
}

/// Parsing of some amount of tokens into an expression.
fn parse_expression(
	tokens: &mut VecDeque<Token>,
	type_context: &TypeContext,
) -> Result<Expression, ExpressionParsingError> {
	// Parsing a leaf expression, ie an expression that doesn't contain more arbitrary expressions.
	let mut expression = match tokens.front().cloned() {
		Some(Token::Integer(value)) => {
			tokens.pop_front();
			Expression::Const(Value::Integer(value))
		},
		Some(Token::Word(word)) => {
			tokens.pop_front();
			Expression::Variable(word)
		},
		Some(unexpected_token) => {
			return Err(ExpressionParsingError::UnexpectedToken(unexpected_token))
		},
		None => return Err(ExpressionParsingError::NoTokens),
	};

	// If an open parenthesis follow then it would mean that we are parsing a function call.
	if matches!(tokens.front(), Some(Token::OpenParenthesis)) {
		tokens.pop_front(); // The open parenthesis.

		// Function call.
		// We are now parsing the potential arguments up until the closing parenthesis.
		// We still check that `expression` (that is called by this call) is a function.

		let function_type_signature = match expression.get_type(type_context) {
			Ok(Type::Function(type_signature)) => type_signature,
			Ok(_not_a_function_type) => {
				return Err(ExpressionParsingError::FunctionCallOnNotAFunction)
			},
			Err(_) => return Err(ExpressionParsingError::ErroneousType),
		};

		let mut args = vec![];
		loop {
			args.push(parse_expression(tokens, type_context)?);

			if matches!(tokens.front(), Some(Token::CloseParenthesis)) {
				tokens.pop_front(); // The close parenthesis.

				// Closing parenthesis, this is the end of the arguments.
				// We can now check the types of the arguments againts
				// the type constraints of the function.

				check_function_call_argument_types(function_type_signature, &args, type_context)
					.map_err(ExpressionParsingError::FunctionCallTypeCheckError)?;

				expression = Expression::FunctionCall { func: Box::new(expression), args };
				break;
			} else if matches!(tokens.front(), Some(Token::Comma)) {
				tokens.pop_front(); // The comma.
			} else {
				todo!("handle unexpected token error");
			}
		}
	}

	Ok(expression)
}

fn evaluate_expression(expression: &Expression, context: &Context) -> Value {
	match expression {
		Expression::Const(value) => value.clone(),
		Expression::Variable(name) => context.variables.get(name).unwrap().clone(),
		Expression::FunctionCall { func, args } => {
			let func_as_value = evaluate_expression(func, context);
			match func_as_value {
				Value::Function(Function { body, .. }) => {
					let arg_values: Vec<_> = args
						.iter()
						.map(|arg| evaluate_expression(arg, context))
						.collect();
					match body {
						FunctionBody::Expression(body_expression) => {
							evaluate_expression(&body_expression, context)
						},
						FunctionBody::BuiltIn(built_in_function_body) => {
							built_in_function_body.evaluate(arg_values)
						},
					}
				},
				_ => todo!(),
			}
		},
	}
}

fn parse(code: &str, type_context: &TypeContext) -> Result<Expression, ExpressionParsingError> {
	let mut tokens = VecDeque::from(tokenize(code));
	parse_expression(&mut tokens, type_context)
}

pub fn run(code: &str, context: &Context) -> Result<(), ExpressionParsingError> {
	let expression = parse(code, &context.get_type_context())?;
	evaluate_expression(&expression, context);
	Ok(())
}

pub fn test_lang(test_id: u32) {
	match test_id {
		1 => {
			run("print_integer(69)", &Context::with_builtins()).unwrap();
		},
		2 => {
			run("print_three_integers(42, 2, 8)", &Context::with_builtins()).unwrap();
		},
		3 => {
			run(
				"print_type(type_of(print_integer))",
				&Context::with_builtins(),
			)
			.unwrap();
		},
		unknown_id => panic!("test lang id {unknown_id} doesn't identify a known test"),
	}
}
