//! Qwy3 internal command programming language, named Qwy Script.
//!
//! This module handles the parsing and interpretation of programs.
//! Programs are statically typed (because it feels so much better when
//! working with values which we know the type of at program writing time,
//! this is a subjective opinion but this is also my project >w<).

use std::{
	collections::{HashMap, VecDeque},
	ops::Deref,
};

use enum_iterator::Sequence;

/// A type in Qwy Script.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Type {
	Nothing,
	Integer,
	Function(FunctionTypeSignature),
	Type,
	Name,
}

/// A value in Qwy Script.
#[derive(Clone, Debug)]
pub enum Value {
	Nothing,
	Integer(i32),
	Function(Function),
	Type(Type),
	Name(String),
}

impl Value {
	fn get_type(&self) -> Type {
		match self {
			Value::Nothing => Type::Nothing,
			Value::Integer(_) => Type::Integer,
			Value::Function(Function { signature, .. }) => Type::Function(signature.clone()),
			Value::Type(_) => Type::Type,
			Value::Name(_) => Type::Name,
		}
	}
}

/// Constraints on a type.
/// Function signatures present such constraints for the argument types instead of directly types,
/// so that functions such as `type_of` can take a value of any type as its argument.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TypeConstraints {
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

#[derive(Clone, Copy, Sequence, Debug)]
enum BuiltInFunctionBody {
	PrintInteger,
	PrintThreeIntegers,
	ToType,
	PrintType,
	/// TODO: Maybe move this feature somewhere else than a function >w<.
	DeclareAndSetGlobalVariable,
}

impl BuiltInFunctionBody {
	fn evaluate(self, arg_values: Vec<Value>, context: &mut Context) -> Value {
		match self {
			BuiltInFunctionBody::PrintInteger => {
				let integer_value = match arg_values[0] {
					Value::Integer(integer_value) => integer_value,
					_ => todo!(),
				};
				println!("printing integer {integer_value}");
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
				println!("printing three integers {integer_values:?}");
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
				println!("printing type {type_value:?}");
				Value::Nothing
			},
			BuiltInFunctionBody::DeclareAndSetGlobalVariable => {
				let mut arg_values = arg_values.into_iter();
				let name_as_string = match arg_values.next().unwrap() {
					Value::Name(name_as_string) => name_as_string,
					_ => todo!(),
				};
				let value = arg_values.next().unwrap();
				println!("declaring {name_as_string} and setting it to {value:?}");
				let previous_value = context.variables.insert(name_as_string, value);
				if previous_value.is_some() {
					panic!("declaring global variable that was already declared");
				}
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
			BuiltInFunctionBody::DeclareAndSetGlobalVariable => "declare_and_set_global_variable",
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
			BuiltInFunctionBody::DeclareAndSetGlobalVariable => FunctionTypeSignature {
				arg_types: vec![TypeConstraints::Only(Type::Name), TypeConstraints::Any],
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

#[derive(Clone, Debug)]
enum FunctionBody {
	BuiltIn(BuiltInFunctionBody),
	Expression(Box<Expression>),
}

#[derive(Clone, Debug)]
pub struct Function {
	signature: FunctionTypeSignature,
	body: FunctionBody,
}

#[derive(Clone, Debug)]
enum Expression {
	Const(Value),
	Variable(String),
	FunctionCall {
		func: Box<(Expression, Span)>,
		args: Vec<(Expression, Span)>,
	},
	Block(Vec<(Expression, Span)>),
}

#[derive(Debug)]
pub enum ExpressionTypingError {
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
				let func_type = func.0.get_type(type_context);
				match func_type {
					Ok(Type::Function(signature)) => Ok(signature.return_type.deref().clone()),
					Err(_) => Err(ExpressionTypingError::FunctionCallOnErroneousType),
					Ok(_) => Err(ExpressionTypingError::FunctionCallOnNotAFunction),
				}
			},
			Expression::Block(expr_sequence) => expr_sequence.last().unwrap().0.get_type(type_context),
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

/// Indicates a non-empty interval in the qwyllang source code text.
#[derive(Clone, Debug)]
pub struct Span {
	start: usize,
	/// Included.
	end: usize,
}

#[derive(Clone, Debug)]
pub enum Token {
	Word(String),
	Integer(i32),
	OpenParenthesis,
	CloseParenthesis,
	Comma,
	OpenCurly,
	CloseCurly,
	Semicolon,
	Sigil,
}

fn tokenize(qwy_script_code: &str) -> Vec<(Token, Span)> {
	let mut tokens = vec![];
	let mut chars = qwy_script_code.chars().enumerate().peekable();

	while chars.peek().is_some() {
		match chars.peek().copied() {
			None => break,
			Some((_i, c)) if c.is_whitespace() => {
				chars.next();
			},
			Some((i, c)) if c.is_ascii_alphabetic() || c == '_' => {
				let mut word = String::new();
				let start = i;
				let mut end = i;
				while chars
					.peek()
					.copied()
					.is_some_and(|(_i, c)| c.is_ascii_alphanumeric() || c == '_')
				{
					let (i, c) = chars.next().unwrap();
					word.push(c);
					end = i;
				}
				tokens.push((Token::Word(word), Span { start, end }));
			},
			Some((i, c)) if c.is_ascii_digit() => {
				let mut value = 0;
				let start = i;
				let mut end = i;
				while chars.peek().is_some_and(|(_i, c)| c.is_ascii_digit()) {
					let (i, c) = chars.next().unwrap();
					value = value * 10 + c as i32 - '0' as i32;
					end = i;
				}
				tokens.push((Token::Integer(value), Span { start, end }));
			},
			Some((i, '(')) => {
				chars.next();
				tokens.push((Token::OpenParenthesis, Span { start: i, end: i }));
			},
			Some((i, ')')) => {
				chars.next();
				tokens.push((Token::CloseParenthesis, Span { start: i, end: i }));
			},
			Some((i, ',')) => {
				chars.next();
				tokens.push((Token::Comma, Span { start: i, end: i }));
			},
			Some((i, '{')) => {
				chars.next();
				tokens.push((Token::OpenCurly, Span { start: i, end: i }));
			},
			Some((i, '}')) => {
				chars.next();
				tokens.push((Token::CloseCurly, Span { start: i, end: i }));
			},
			Some((i, ';')) => {
				chars.next();
				tokens.push((Token::Semicolon, Span { start: i, end: i }));
			},
			Some((i, '$')) => {
				chars.next();
				tokens.push((Token::Sigil, Span { start: i, end: i }));
			},
			_ => todo!(),
		}
	}
	tokens
}

#[derive(Debug)]
pub enum ExpressionParsingError {
	ExpectedStartOfExpressionButGotNoMoreTokens,
	ExpectedStartOfExpressionButGotUnexpectedToken(Token, Span),
	ExpectedCommaToSeparateArgumentsButGotUnexpectedToken(Token, Span),
	ExpectedCommaOrClosedParenthesisButGotNoMoreTokens,
	ExpressionTypingError(ExpressionTypingError, Span),
	/// The actual type of the not-a-function, and its span.
	FunctionCallOnNotAFunction(Type, Span),
	/// The span is the span of the whole function call in which there is a typeing error.
	FunctionCallTypingError(FunctionCallTypingError, Span),
	ExpectedWordAfterSigilButGotUnexpectedToken(Token, Span),
	ExpectedWordAfterSigilButGotNoMoreTokens,
	ExpectedSemicolonToSeparateExpressionsInBlockButGotUnexpectedToken(Token, Span),
	ExpectedSemicolonOrClosedCurlyButGotNoMoreTokens,
}

/// Parsing of some amount of tokens into an expression.
/// This focuses on leaf expressions, ie expressions that do not contain sub expressions,
/// leafs in the AST.
fn parse_leaf_expression(
	tokens: &mut VecDeque<(Token, Span)>,
	type_context: &TypeContext,
) -> Result<(Expression, Span), ExpressionParsingError> {
	// Parsing a leaf expression, ie an expression that doesn't contain more arbitrary expressions.
	let (expression, expression_span) = match tokens.front().cloned() {
		Some((Token::Integer(value), span)) => {
			tokens.pop_front();
			(Expression::Const(Value::Integer(value)), span)
		},
		Some((Token::Word(word), span)) => {
			tokens.pop_front();
			(Expression::Variable(word), span)
		},
		Some((Token::Sigil, sigil_span)) => {
			tokens.pop_front();
			match tokens.pop_front() {
				Some((Token::Word(name), name_span)) => (
					Expression::Const(Value::Name(name)),
					Span { start: sigil_span.start, end: name_span.end },
				),
				Some((unexpected_token, span)) => {
					return Err(
						ExpressionParsingError::ExpectedWordAfterSigilButGotUnexpectedToken(
							unexpected_token,
							span,
						),
					)
				},
				None => return Err(ExpressionParsingError::ExpectedWordAfterSigilButGotNoMoreTokens),
			}
		},
		Some((unexpected_token, span)) => {
			return Err(
				ExpressionParsingError::ExpectedStartOfExpressionButGotUnexpectedToken(
					unexpected_token,
					span,
				),
			)
		},
		None => return Err(ExpressionParsingError::ExpectedStartOfExpressionButGotNoMoreTokens),
	};

	if let Err(expression_typing_error) = expression.get_type(type_context) {
		Err(ExpressionParsingError::ExpressionTypingError(
			expression_typing_error,
			expression_span,
		))
	} else {
		Ok((expression, expression_span))
	}
}

/// Parsing of some amount of tokens into an expression.
/// /// This focuses on leaf expressions or call expressions.
fn parse_leaf_or_call_expression(
	tokens: &mut VecDeque<(Token, Span)>,
	type_context: &TypeContext,
) -> Result<(Expression, Span), ExpressionParsingError> {
	// Parsing a leaf expression, ie an expression that doesn't contain more arbitrary expressions.
	let (mut expression, mut expression_span) = parse_leaf_expression(tokens, type_context)?;

	// If an open parenthesis follow then it would mean that we are parsing a function call.
	if let Some((Token::OpenParenthesis, open_parenthesis_span)) = tokens.front().cloned() {
		tokens.pop_front(); // The open parenthesis.

		// Function call.
		// We are now parsing the potential arguments up until the closing parenthesis.
		// We still check that `expression` (that is called by this call) is a function.

		let function_span = expression_span.clone();
		let mut argument_list_in_parenthesis_span = open_parenthesis_span;

		let function_type_signature = match expression.get_type(type_context) {
			Ok(Type::Function(type_signature)) => type_signature,
			Ok(not_a_function_type) => {
				return Err(ExpressionParsingError::FunctionCallOnNotAFunction(
					not_a_function_type,
					expression_span.clone(),
				))
			},
			Err(_) => unreachable!("handled earlier"),
		};

		// Comma-separated sequence of arguments, ended by a closed parenthesis.
		// A comma just before the closed parenthesis is allowed, it makses sense when
		// having the closing parenthesis on an other line than the last argument.
		let mut args_and_spans = vec![];
		let mut comma_needed_before_next_argument = false;
		loop {
			if let Some((Token::CloseParenthesis, close_parenthesis_span)) = tokens.front() {
				expression_span.end = close_parenthesis_span.end;
				argument_list_in_parenthesis_span.end = close_parenthesis_span.end;
				tokens.pop_front(); // The close parenthesis.
				break;
			}

			if comma_needed_before_next_argument
				&& matches!(tokens.front(), Some((Token::Comma, _comma_span)))
			{
				tokens.pop_front(); // The comma.
				comma_needed_before_next_argument = false;
			}

			if !matches!(tokens.front(), Some((Token::CloseParenthesis, _span))) {
				if comma_needed_before_next_argument {
					if let Some((unexpected_token, span)) = tokens.pop_front() {
						return Err(
							ExpressionParsingError::ExpectedCommaToSeparateArgumentsButGotUnexpectedToken(
								unexpected_token,
								span,
							),
						);
					} else {
						return Err(
							ExpressionParsingError::ExpectedCommaOrClosedParenthesisButGotNoMoreTokens,
						);
					}
				} else {
					// An argument.
					args_and_spans.push(parse_expression(tokens, type_context)?);
					comma_needed_before_next_argument = true;
				}
			}
		}

		// We can now check the types of the arguments againts
		// the type constraints of the function.
		check_function_call_argument_types(
			function_type_signature,
			function_span.clone(),
			argument_list_in_parenthesis_span,
			&args_and_spans,
			type_context,
		)
		.map_err(|function_type_error| {
			ExpressionParsingError::FunctionCallTypingError(
				function_type_error,
				expression_span.clone(),
			)
		})?;
		expression = Expression::FunctionCall {
			func: Box::new((expression, function_span)),
			args: args_and_spans,
		};
	}

	Ok((expression, expression_span))
}

/// Parsing of some amount of tokens into an expression.
fn parse_expression(
	tokens: &mut VecDeque<(Token, Span)>,
	type_context: &TypeContext,
) -> Result<(Expression, Span), ExpressionParsingError> {
	// If we find an open curly for starters then it would mean that we are parsing a block.
	let (expression, expression_span) = if let Some((Token::OpenCurly, open_curly_span)) =
		tokens.front().cloned()
	{
		tokens.pop_front(); // The open curly.

		let mut expression_sequence = vec![];

		let block_span = loop {
			expression_sequence.push(parse_expression(tokens, type_context)?);

			match tokens.front().cloned() {
				Some((Token::CloseCurly, close_curly_span)) => {
					tokens.pop_front(); // The close curly.
					break Span { start: open_curly_span.start, end: close_curly_span.end };
				},
				Some((Token::Semicolon, _semicolon_span)) => {
					tokens.pop_front(); // The semicolon.
				},
				Some((unexpected_token, unexpected_token_span)) => {
					return Err(ExpressionParsingError::ExpectedSemicolonToSeparateExpressionsInBlockButGotUnexpectedToken(unexpected_token, unexpected_token_span));
				},
				None => {
					return Err(
						ExpressionParsingError::ExpectedSemicolonOrClosedCurlyButGotNoMoreTokens,
					);
				},
			}
		};

		(Expression::Block(expression_sequence), block_span)
	} else {
		// Parsing an expression without handling blocks.
		parse_leaf_or_call_expression(tokens, type_context)?
	};

	Ok((expression, expression_span))
}

#[derive(Debug)]
pub enum FunctionCallTypingError {
	/// How many arguments are missing.
	MissingArguments {
		how_many_args_missing: u32,
		args_list_in_parenthesis_span: Span,
		function_span: Span,
		function_call_span: Span,
	},
	/// How many arguments are there above the expected number of arguments.
	TooManyArguments {
		how_many_args_in_excess: u32,
		args_in_excess_span: Span,
		function_span: Span,
		function_call_span: Span,
	},
	/// Argument expression typing error and argument span.
	ArgumentExpressionTypingError {
		arg_typing_error: ExpressionTypingError,
		faulty_arg_span: Span,
		faulty_arg_index: usize,
		function_span: Span,
		function_call_span: Span,
	},
	/// Expected type constraints by the called function type signature,
	/// the (wrong) type of the faulty argument, and the span of that argument,
	/// and the span of the called function.
	ArgumentOfTheWrongType {
		parameter_type_constraints: TypeConstraints,
		wrong_arg_type: Type,
		wrong_arg_span: Span,
		wrong_arg_index: usize,
		function_span: Span,
		function_call_span: Span,
	},
}

fn check_function_call_argument_types(
	function_type_signature: FunctionTypeSignature,
	function_span: Span,
	args_list_in_parenthesis_span: Span,
	args: &[(Expression, Span)],
	type_context: &TypeContext,
) -> Result<(), FunctionCallTypingError> {
	let function_call_span = Span {
		start: function_span.start,
		end: args_list_in_parenthesis_span.end,
	};

	// Check for the number of arguments.
	let expected_arg_count = function_type_signature.arg_types.len();
	let actual_arg_count = args.len();
	if expected_arg_count > actual_arg_count {
		return Err(FunctionCallTypingError::MissingArguments {
			how_many_args_missing: (expected_arg_count - actual_arg_count) as u32,
			args_list_in_parenthesis_span,
			function_span,
			function_call_span,
		});
	}
	if expected_arg_count < actual_arg_count {
		let args_in_excess_span = Span {
			start: args[expected_arg_count].1.start,
			end: args[actual_arg_count - 1].1.end,
		};
		return Err(FunctionCallTypingError::TooManyArguments {
			how_many_args_in_excess: (actual_arg_count - expected_arg_count) as u32,
			args_in_excess_span,
			function_span,
			function_call_span,
		});
	}

	// Check for the types of the arguments.
	for (arg_i, (arg, arg_span)) in args.iter().enumerate() {
		let type_constraints = &function_type_signature.arg_types[arg_i];
		let actual_type = match arg.get_type(type_context) {
			Ok(actual_type) => actual_type,
			Err(type_error) => {
				return Err(FunctionCallTypingError::ArgumentExpressionTypingError {
					arg_typing_error: type_error,
					faulty_arg_span: arg_span.clone(),
					faulty_arg_index: arg_i,
					function_span,
					function_call_span,
				})
			},
		};
		if !type_constraints.is_satisfied_by_type(&actual_type) {
			return Err(FunctionCallTypingError::ArgumentOfTheWrongType {
				parameter_type_constraints: type_constraints.clone(),
				wrong_arg_type: actual_type,
				wrong_arg_span: arg_span.clone(),
				wrong_arg_index: arg_i,
				function_span,
				function_call_span,
			});
		}
	}
	Ok(())
}

fn evaluate_expression(expression: &Expression, context: &mut Context) -> Value {
	match expression {
		Expression::Const(value) => value.clone(),
		Expression::Variable(name) => context.variables.get(name).unwrap().clone(),
		Expression::FunctionCall { func, args } => {
			let func_value = evaluate_expression(&func.0, context);
			let func_body = match func_value {
				Value::Function(Function { body, .. }) => body,
				_ => todo!(),
			};
			let arg_values: Vec<_> = args
				.iter()
				.map(|arg| evaluate_expression(&arg.0, context))
				.collect();
			match func_body {
				FunctionBody::Expression(body_expression) => {
					evaluate_expression(&body_expression, context)
				},
				FunctionBody::BuiltIn(built_in_function_body) => {
					built_in_function_body.evaluate(arg_values, context)
				},
			}
		},
		Expression::Block(expr_sequence) => {
			let ((last_expr, _last_span), expr_sequence_before_last) =
				expr_sequence.split_last().unwrap();
			for (expr, _span) in expr_sequence_before_last {
				evaluate_expression(expr, context);
			}
			evaluate_expression(last_expr, context)
		},
	}
}

fn parse(
	qwy_script_code: &str,
	type_context: &TypeContext,
) -> Result<(Expression, Span), ExpressionParsingError> {
	let mut tokens = VecDeque::from(tokenize(qwy_script_code));
	parse_expression(&mut tokens, type_context)
}

pub fn run(qwy_script_code: &str, context: &mut Context) -> Result<(), ExpressionParsingError> {
	let (expression, _span) = parse(qwy_script_code, &context.get_type_context())?;
	evaluate_expression(&expression, context);
	Ok(())
}

pub fn test_lang(test_id: u32) {
	match test_id {
		1 => {
			run("print_integer(69)", &mut Context::with_builtins()).unwrap();
		},
		2 => {
			run(
				"print_three_integers(42, 2, 8)",
				&mut Context::with_builtins(),
			)
			.unwrap();
		},
		3 => {
			run(
				"print_type(type_of(print_integer))",
				&mut Context::with_builtins(),
			)
			.unwrap();
		},
		4 => {
			let mut context = Context::with_builtins();
			context.variables.insert(
				"jaaj".to_string(),
				Value::Function(Function {
					signature: FunctionTypeSignature {
						arg_types: vec![],
						return_type: Box::new(Type::Integer),
					},
					body: FunctionBody::Expression(Box::new(Expression::Const(Value::Integer(420)))),
				}),
			);
			run("print_integer(jaaj())", &mut context).unwrap();
		},
		5 => {
			let context = Context::with_builtins();
			let parsing_error = parse("print_integer()", &context.get_type_context()).unwrap_err();
			dbg!(parsing_error);
		},
		6 => {
			let mut context = Context::with_builtins();
			run("declare_and_set_global_variable($test, 8)", &mut context).unwrap();
			run("print_integer(test)", &mut context).unwrap();
		},
		7 => {
			run(
				"{print_integer(1); print_integer(2); print_integer(3)}",
				&mut Context::with_builtins(),
			)
			.unwrap();
		},
		8 => {
			let mut context = Context::with_builtins();
			run(
				"declare_and_set_global_variable($test, {print_integer(42); 8})",
				&mut context,
			)
			.unwrap();
			run("print_integer(test)", &mut context).unwrap();
		},
		unknown_id => panic!("test lang id {unknown_id} doesn't identify a known test"),
	}
}
