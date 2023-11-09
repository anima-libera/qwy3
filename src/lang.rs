use std::{collections::HashMap, ops::Deref};

/// A type in the language.
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
enum TypeConstraints {
	/// Only one type satisfy the constraints.
	Only(Type),
	/// Any type can do.
	Any,
}

#[derive(Clone, Debug)]
pub struct FunctionTypeSignature {
	arg_types: Vec<TypeConstraints>,
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
	fn get_type(&self, context: &Context) -> Result<Type, ExpressionTypingError> {
		match self {
			Expression::Const(value) => Ok(value.get_type()),
			Expression::Variable(name) => {
				if let Some(variable_value) = context.variables.get(name) {
					Ok(variable_value.get_type())
				} else {
					Err(ExpressionTypingError::UnknownVariable)
				}
			},
			Expression::FunctionCall { func, .. } => {
				let func_type = func.get_type(context);
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
}

enum Token {
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
}

fn parse_expression(
	tokens: &[Token],
	context: &Context,
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
		_ => todo!(),
	};
	if matches!(tokens.get(i), Some(Token::OpenParenthesis)) {
		i += 1;
		let mut args = vec![];
		loop {
			match parse_expression(&tokens[i..], context) {
				Err(_) => todo!(),
				Ok((sub_expression, number_of_tokens_parsed)) => {
					i += number_of_tokens_parsed;
					args.push(sub_expression);
					if matches!(tokens.get(i), Some(Token::CloseParenthesis)) {
						i += 1;
						expression = Expression::FunctionCall { func: Box::new(expression), args };
						break;
					} else if matches!(tokens.get(i), Some(Token::Comma)) {
						i += 1;
					} else {
						todo!("handle unexpected token error");
					}
				},
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

fn parse(code: &str, context: &Context) -> Result<Expression, ExpressionParsingError> {
	let tokens = tokenize(code);
	parse_expression(&tokens, context).map(|(expression, _number_of_tokens_parsed)| expression)
}

pub fn run(code: &str, context: &Context) -> Result<(), ExpressionParsingError> {
	let expression = parse(code, context)?;
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
