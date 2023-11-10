use std::{collections::HashMap, ops::Deref};

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

#[derive(Clone)]
enum BuiltInFunctionBody {
	PrintInteger,
	PrintThreeIntegers,
	ToType,
	PrintType,
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
		variables.insert(
			"print_integer".to_string(),
			Value::Function(Function {
				signature: FunctionTypeSignature {
					arg_types: vec![TypeConstraints::Only(Type::Integer)],
					return_type: Box::new(Type::Nothing),
				},
				body: FunctionBody::BuiltIn(BuiltInFunctionBody::PrintInteger),
			}),
		);
		variables.insert(
			"print_three_integers".to_string(),
			Value::Function(Function {
				signature: FunctionTypeSignature {
					arg_types: vec![
						TypeConstraints::Only(Type::Integer),
						TypeConstraints::Only(Type::Integer),
						TypeConstraints::Only(Type::Integer),
					],
					return_type: Box::new(Type::Nothing),
				},
				body: FunctionBody::BuiltIn(BuiltInFunctionBody::PrintThreeIntegers),
			}),
		);
		variables.insert(
			"type_of".to_string(),
			Value::Function(Function {
				signature: FunctionTypeSignature {
					arg_types: vec![TypeConstraints::Any],
					return_type: Box::new(Type::Type),
				},
				body: FunctionBody::BuiltIn(BuiltInFunctionBody::ToType),
			}),
		);
		variables.insert(
			"print_type".to_string(),
			Value::Function(Function {
				signature: FunctionTypeSignature {
					arg_types: vec![TypeConstraints::Only(Type::Type)],
					return_type: Box::new(Type::Nothing),
				},
				body: FunctionBody::BuiltIn(BuiltInFunctionBody::PrintType),
			}),
		);
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
	FunctionCallWithWrongNumberOfArguments,
	FunctionCallWithAnArgumentOfTheWrongType,
}

/// Parsing of some amount of tokens into an expression.
/// The amount of token parsed is returned alongside the parsed expression.
fn parse_expression(
	tokens: &[Token],
	type_context: &TypeContext,
) -> Result<(Expression, usize), ExpressionParsingError> {
	let mut i = 0;
	let mut expression = match tokens.get(i) {
		None => return Err(ExpressionParsingError::NoTokens),
		Some(Token::Integer(value)) => {
			i += 1;
			Expression::Const(Value::Integer(*value))
		},
		Some(Token::Word(word)) => {
			i += 1;
			Expression::Variable(word.clone())
		},
		Some(unexpected_token) => {
			return Err(ExpressionParsingError::UnexpectedToken(
				(*unexpected_token).clone(),
			))
		},
	};

	if matches!(tokens.get(i), Some(Token::OpenParenthesis)) {
		i += 1;
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
			let (arg_expression, number_of_tokens_parsed) =
				parse_expression(&tokens[i..], type_context)?;
			i += number_of_tokens_parsed;
			args.push(arg_expression);

			if matches!(tokens.get(i), Some(Token::CloseParenthesis)) {
				i += 1;
				// Closing parenthesis, this is the end of the arguments.
				// We can now check the types of the arguments againts
				// the type constraints of the function.

				let expected_arg_count = function_type_signature.arg_types.len();
				let actual_arg_count = args.len();
				if expected_arg_count != actual_arg_count {
					return Err(ExpressionParsingError::FunctionCallWithWrongNumberOfArguments);
				}
				for (arg_i, arg) in args.iter().enumerate() {
					let type_constraints = &function_type_signature.arg_types[arg_i];
					let actual_type = match arg.get_type(type_context) {
						Ok(actual_type) => actual_type,
						Err(_) => return Err(ExpressionParsingError::ErroneousType),
					};
					if !type_constraints.is_satisfied_by_type(&actual_type) {
						return Err(ExpressionParsingError::FunctionCallWithAnArgumentOfTheWrongType);
					}
				}

				expression = Expression::FunctionCall { func: Box::new(expression), args };
				break;
			} else if matches!(tokens.get(i), Some(Token::Comma)) {
				i += 1;
			} else {
				todo!("handle unexpected token error");
			}
		}
	}

	Ok((expression, i))
}

fn evaluate_expression(expression: &Expression, context: &Context) -> Value {
	match expression {
		Expression::Const(value) => value.clone(),
		Expression::Variable(name) => context.variables.get(name).unwrap().clone(),
		Expression::FunctionCall { func, args } => {
			let func_as_value = evaluate_expression(func, context);
			match func_as_value {
				Value::Function(Function { body, .. }) => {
					let args_as_value: Vec<_> = args
						.iter()
						.map(|arg| evaluate_expression(arg, context))
						.collect();
					match body {
						FunctionBody::Expression(body_expression) => {
							evaluate_expression(&body_expression, context)
						},
						FunctionBody::BuiltIn(BuiltInFunctionBody::PrintInteger) => {
							let values: Vec<_> = args_as_value
								.iter()
								.map(|arg| match arg {
									Value::Integer(value) => value,
									_ => todo!(),
								})
								.collect();
							let value = values[0];
							println!("printing integer {value}",);
							Value::Nothing
						},
						FunctionBody::BuiltIn(BuiltInFunctionBody::PrintThreeIntegers) => {
							let values: Vec<_> = args_as_value
								.iter()
								.map(|arg| match arg {
									Value::Integer(value) => value,
									_ => todo!(),
								})
								.collect();
							println!("printing three integers {values:?}",);
							Value::Nothing
						},
						FunctionBody::BuiltIn(BuiltInFunctionBody::ToType) => {
							let value_type = args_as_value[0].get_type();
							Value::Type(value_type)
						},
						FunctionBody::BuiltIn(BuiltInFunctionBody::PrintType) => {
							let type_values: Vec<_> = args_as_value
								.iter()
								.map(|arg| match arg {
									Value::Type(type_value) => type_value,
									_ => todo!(),
								})
								.collect();
							let type_value = type_values[0];
							println!("printing integer {type_value:?}",);
							Value::Nothing
						},
					}
				},
				_ => todo!(),
			}
		},
	}
}

fn parse(code: &str, type_context: &TypeContext) -> Result<Expression, ExpressionParsingError> {
	let tokens = tokenize(code);
	parse_expression(&tokens, type_context).map(|(expression, _number_of_tokens_parsed)| expression)
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
