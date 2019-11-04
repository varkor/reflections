use std::collections::HashMap;
use std::f64;
use std::fmt;
use std::mem;
use std::str::FromStr;
use std::vec::IntoIter;

/// String matching varieties: prefix or exact match.
#[derive(PartialEq)]
enum MatchKind {
    Prefix,
    All,
}

/// A lexical unit.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    End,
    Number(f64),
    Name(String),
    OpenParen,
    CloseParen,
    Add,
    Sub,
    Mul,
    Div,
    Exp,
}

impl Token {
    /// A collection of all the non-special tokens, to facilitate longest-prefix matching.
    fn all() -> Vec<Token> {
        use self::Token::*;

        // Tokens with values are given default dummy values, as the tokens are only used for
        // matching on the type.
        vec![
            // `End` is deliberately not included, as a special token that is created implicitly.
            Number(Default::default()),
            Name(Default::default()),
            OpenParen,
            CloseParen,
            Add,
            Sub,
            Mul,
            Div,
            Exp,
        ]
    }

    /// Tests whether a string is valid for a specific token.
    fn matches(&self, s: &str, kind: MatchKind) -> bool {
        use self::Token::*;

        match (self, s) {
            // Empty strings are trivially prefixes of every token.
            (_, "") => kind == MatchKind::Prefix,

            // Literal tokens.
            (OpenParen, "(") |
            (CloseParen, ")") |
            (Add, "+") |
            (Sub, "-") |
            (Mul, "*") |
            (Div, "/") |
            (Exp, "^") => true,

            // Numeric tokens.
            (Number(_), s) => {
                #[derive(PartialEq)]
                enum State { Integer, Dot, Fractional }

                let mut state = State::Integer;
                s.chars().all(|c| {
                    match state {
                        State::Integer => {
                            if c == '.' {
                                state = State::Dot;
                                true
                            } else {
                                c.is_digit(10)
                            }
                        }
                        State::Dot => {
                            state = State::Fractional;
                            c.is_digit(10)
                        }
                        State::Fractional => c.is_digit(10),
                    }
                }) && (kind == MatchKind::Prefix || state != State::Dot)
            }

            // Textual tokens (e.g. variables and functions).
            (Name(_), s) => {
                s.chars().all(|c| {
                    c.is_ascii_alphabetic() && c.is_ascii_lowercase() || c == 'π' || c == 'τ'
                })
            }

            _ => false,
        }
    }
}

/// A token together with the string to which it corresponds.
#[derive(Debug)]
pub struct Lexeme {
    kind: Token,
    string: String,
}

/// Facilitates converting textual input into tokens.
pub struct Lexer;

impl Lexer {
    /// Convert a stream of characters into a stream of lexemes.
    pub fn scan(chars: impl Iterator<Item = char>) -> Result<Vec<Lexeme>, String> {
        let mut lexemes = vec![];
        let mut chars = chars.peekable();
        let mut end = false;

        while !end {
            let mut s = String::new();
            let mut states = Token::all();

            end = loop {
                if let Some(&c) = chars.peek() {
                    if c.is_ascii_whitespace() {
                        chars.next();
                        break false;
                    }

                    let mut s_next = s.clone();
                    s_next.push(c);
                    let states_next: Vec<_> = states.clone()
                        .into_iter()
                        .filter(|t| t.matches(&s_next, MatchKind::Prefix))
                        .collect();
                    if !states_next.is_empty() || s.is_empty() {
                        chars.next();
                        s = s_next;
                        states = states_next;
                    } else {
                        break false;
                    }
                } else {
                    break true;
                }
            };

            // Empty strings correspond to whitespace, so we can skip them.
            if !s.is_empty()  {
                let states: Vec<_> = states
                    .into_iter()
                    .filter(|t| t.matches(&s, MatchKind::All))
                    .collect();
                let mut states = states.into_iter();
                let first = states.next();
                match (first, states.next()) {
                    (None, _) => return Err(format!("unrecognised symbol {}", s)),
                    (Some(state), None) => {
                        lexemes.push(Lexeme {
                            kind: state,
                            string: s,
                        });
                    }
                    _ if end => return Err("unexpected end of input".to_string()),
                    _ => panic!("ambiguous token".to_string()),
                }
            }
        }

        lexemes.push(Lexeme {
            kind: Token::End,
            string: String::new(),
        });

        Ok(lexemes)
    }

    pub fn evaluate(lexemes: impl Iterator<Item = Lexeme>) -> impl Iterator<Item = Token> {
        lexemes.map(|l| {
            match l.kind {
                Token::Number(_) => Token::Number(l.string.parse().unwrap()),
                Token::Name(_) => Token::Name(l.string),
                _ => l.kind,
            }
        })
    }
}

type ParseResult<T> = Result<T, ()>;

/// A parser for expressions.
#[derive(Clone, Debug)]
pub struct Parser<I: Iterator<Item = Token> + Clone> {
    tokens: I,
    pos: usize,
    token: Token,
}

impl Parser<IntoIter<Token>> {
    pub fn new(tokens: Vec<Token>) -> Parser<IntoIter<Token>> {
        let mut tokens = tokens.into_iter();
        if let Some(token) = tokens.next() {
            Self {
                tokens,
                pos: 1,
                token,
            }
        } else {
            panic!("parser given no tokens");
        }
    }
}

/// The various precedences for operations.
#[derive(PartialEq, PartialOrd, Clone, Copy, Debug)]
enum Precedence {
    Additive,
    Multiplicative,
    Exponential,
}

impl Precedence {
    /// The lowest precedence level (i.e. the one that binds least tightly).
    fn lowest() -> Precedence {
        Precedence::Additive
    }

    /// The next highest precedence, or `None` if there are no higher precedence levels.
    fn next(&self) -> Option<Precedence> {
        Some(match self {
            Precedence::Additive => Precedence::Multiplicative,
            Precedence::Multiplicative => Precedence::Exponential,
            Precedence::Exponential => return None,
        })
    }

    /// Whether operators of this precedence are left-associative.
    fn left_associative(&self) -> bool {
        match self {
            Precedence::Additive |
            Precedence::Multiplicative => true,

            Precedence::Exponential => false,
        }
    }
}

/// A mathematical function.
pub enum Function {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Asinh,
    Acosh,
    Atanh,
}

impl FromStr for Function {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "sin" => Function::Sin,
            "cos" => Function::Cos,
            "tan" => Function::Tan,
            "asin" => Function::Asin,
            "acos" => Function::Acos,
            "atan" => Function::Atan,
            "sinh" => Function::Sinh,
            "cosh" => Function::Cosh,
            "tanh" => Function::Tanh,
            "asinh" => Function::Asinh,
            "acosh" => Function::Acosh,
            "atanh" => Function::Atanh,
            _ => return Err(()),
        })
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            Function::Sin => "sin",
            Function::Cos => "cos",
            Function::Tan => "tan",
            Function::Asin => "asin",
            Function::Acos => "acos",
            Function::Atan => "atan",
            Function::Sinh => "sinh",
            Function::Cosh => "cosh",
            Function::Tanh => "tanh",
            Function::Asinh => "asinh",
            Function::Acosh => "acosh",
            Function::Atanh => "atanh",
        })
    }
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// A handy macro while `try` is unavailable: returns the first `Err` or the trailing expression if
/// `Ok`.
macro_rules! try_block {
    ($($block:tt)*) => (
        (|| { ::std::ops::Try::from_ok({ $($block)* }) })()
    )
}

impl<I: Iterator<Item = Token> + Clone> Parser<I> {
    fn err<T>() -> ParseResult<T> {
        Err(())
    }

    /// Advance a single token.
    fn bump(&mut self) {
        if let Token::End = self.token {
            // This signals a flaw in the parser rather than an issue with the input, so crashing
            // is appropriate.
            panic!("tried to bump past end of input");
        }

        self.pos += 1;
        self.token = self.tokens.next().unwrap_or(Token::End);
    }

    /// Check that the current token precisely matches the one given.
    fn check(&self, t: Token) -> ParseResult<()> {
        if self.token == t {
            Ok(())
        } else {
            Self::err()
        }
    }

    /// Check that the current token matches the one given and advance to the next token.
    fn eat(&mut self, t: Token) -> ParseResult<()> {
        self.check(t)?;
        self.bump();
        Ok(())
    }

    /// Check that we've reached the end of input.
    fn check_end(&self) -> ParseResult<()> {
        if let Token::End = self.token {
            Ok(())
        } else {
            Self::err()
        }
    }

    /// Return the current state of the parser for backtracking.
    fn save(&self) -> Self {
        (*self).clone()
    }

    /// Load a previously-saved parser state for backtracking.
    fn restore(&mut self, save: Self) {
        mem::replace(self, save);
    }

    /// The top-level parsing method.
    pub fn parse(&mut self) -> ParseResult<Expr> {
        let expr = self.parse_expr()?;
        self.check_end()?;
        Ok(expr)
    }

    /// E_0 ::= E_1 E_0'
    fn parse_expr(&mut self) -> ParseResult<Expr> {
        self.parse_expr_with_precedence(Some(Precedence::lowest()))
    }

    // E_i ::= E_{i + 1} E_i'
    fn parse_expr_with_precedence(&mut self, precedence: Option<Precedence>) -> ParseResult<Expr> {
        if let Some(precedence) = precedence {
            let mut subexpr = self.parse_op_expr(precedence)?;
            let mut expr_suffix = self.parse_expr_suffix(precedence)?;

            if precedence.left_associative() {
                while let ExprSuffix::Chain { op, expr, suffix } = expr_suffix {
                    subexpr = Expr::BinOp(op, box subexpr, box expr);
                    expr_suffix = *suffix;
                }
            } else {
                let mut chain = vec![];
                while let ExprSuffix::Chain { op, expr, suffix } = expr_suffix {
                    chain.push((op, subexpr));
                    subexpr = expr;
                    expr_suffix = *suffix;
                }
                while let Some((op, expr)) = chain.pop() {
                    subexpr = Expr::BinOp(op, box expr, box subexpr);
                }
            }

            Ok(subexpr)
        } else {
            self.parse_term()
        }
    }

    // E_i' ::= O E_i E_i' | empty
    fn parse_expr_suffix(&mut self, precedence: Precedence) -> ParseResult<ExprSuffix> {
        let save = self.save();

        let op_expr: ParseResult<_> = try_block! {
            ExprSuffix::Chain {
                op: self.parse_bin_op(precedence)?,
                expr: self.parse_expr_with_precedence(precedence.next())?,
                suffix: box self.parse_expr_suffix(precedence)?,
            }
        };

        op_expr.or_else(|_| {
            self.restore(save);
            Ok(ExprSuffix::Empty)
        })
    }

    // P_i ::= U E_i | E_i
    fn parse_op_expr(&mut self, precedence: Precedence) -> ParseResult<Expr> {
        let prefix_op = self.parse_prefix_un_op(precedence);
        let subexpr = self.parse_expr_with_precedence(precedence.next())?;
        if let Ok(op) = prefix_op {
            Ok(Expr::UnOp(op, box subexpr))
        } else {
            Ok(subexpr)
        }
    }

    fn parse_op<T>(&mut self, ops: Vec<(Token, T)>) -> ParseResult<T> {
        for (t, op) in ops.into_iter() {
            let eat = self.eat(t);
            if eat.is_ok() {
                return Ok(op);
            }
        }
        Self::err()
    }

    // O ::= + | - | * | / | ^
    fn parse_bin_op(&mut self, precedence: Precedence) -> ParseResult<BinOp> {
        self.parse_op(match precedence {
            Precedence::Additive => vec![(Token::Add, BinOp::Add), (Token::Sub, BinOp::Sub)],
            Precedence::Multiplicative => vec![(Token::Mul, BinOp::Mul), (Token::Div, BinOp::Div)],
            Precedence::Exponential => vec![(Token::Exp, BinOp::Exp)],
        })
    }

    // U ::= -
    fn parse_prefix_un_op(&mut self, precedence: Precedence) -> ParseResult<UnOp> {
        self.parse_op(match precedence {
            Precedence::Additive => vec![(Token::Sub, UnOp::Minus)],
            Precedence::Multiplicative => vec![],
            Precedence::Exponential => vec![],
        })
    }

    // T ::= ( E ) | V | X
    fn parse_term(&mut self) -> ParseResult<Expr> {
        let save1 = self.save();
        let save2 = self.save();

        let parenthesised_expr: ParseResult<_> = try_block! {
            self.eat(Token::OpenParen)?;
            let expr = self.parse_expr()?;
            self.eat(Token::CloseParen)?;
            expr
        };

        parenthesised_expr.or_else(|_| {
            self.restore(save1);
            self.parse_function()
        }).or_else(|_| {
            self.restore(save2);
            self.parse_var()
        }).or_else(|_| {
            self.parse_value()
        }).or_else(|_| {
            Self::err()
        })
    }

    // F ::= ('a' ..= 'z')+ ( E_0 )
    fn parse_function(&mut self) -> ParseResult<Expr> {
        let f = match self.token {
            Token::Name(ref n) if n.len() > 1 => {
                Function::from_str(&n)?
            }
            _ => return Self::err(),
        };
        self.bump();
        self.eat(Token::OpenParen)?;
        let expr = self.parse_expr()?;
        self.eat(Token::CloseParen)?;
        Ok(Expr::Function(f, box expr))
    }

    /// Parse a variable: a single alphabetic character.
    fn parse_var(&mut self) -> ParseResult<Expr> {
        let n = match self.token {
            Token::Name(ref n) if n.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) => {
                n.clone()
            }
            _ => return Self::err(),
        };
        self.bump();
        Ok(Expr::Var(n))
    }

    /// Parse a numeric value (integral or floating-point).
    fn parse_value(&mut self) -> ParseResult<Expr> {
        let v = match self.token {
            Token::Number(v) => v,
            Token::Name(ref n) => {
                match n.as_str() {
                    "π" => f64::consts::PI,
                    "τ" => f64::consts::PI * 2.0,
                    _ => return Self::err(),
                }
            }
            _ => return Self::err(),
        };
        self.bump();
        Ok(Expr::Number(v))
    }
}

/// The unary operators.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnOp {
    Minus, // `-`
}

/// The binary operators.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BinOp {
    Add, // `+`
    Sub, // `-`
    Mul, // `*`
    Div, // `/`
    Exp, // `^`
}

/// A mathematical expression.
#[derive(Debug)]
pub enum Expr {
    Number(f64),
    Var(String),
    UnOp(UnOp, Box<Expr>),
    BinOp(BinOp, Box<Expr>, Box<Expr>),
    Function(Function, Box<Expr>),
}

/// An expression suffix represents a chain of operators and subexpressions, allowing us to parse
/// chains of left-associative operators and operands. This is necessary to derive left-associative
/// expressions while avoiding left recursion.
#[derive(Debug)]
enum ExprSuffix {
    Chain {
        op: BinOp,
        expr: Expr,
        suffix: Box<ExprSuffix>,
    },
    Empty,
}

impl Expr {
    /// Evaluate a numeric expression, given a set of variable bindings.
    /// The two `bindings` correspond to those bindings that are constant, versus those that
    /// change frequently. From the perspective of `evaluate`, there's not a difference, but
    /// it avoids unnecessary `clone`s or implementing a delta `HashMap`.
    pub fn evaluate(&self, bindings: (&HashMap<char, f64>, &HashMap<char, f64>)) -> f64 {
        match self {
            &Expr::Number(x) => x,
            Expr::Var(v) => {
                assert_eq!(v.len(), 1);
                let name = v.chars().next().unwrap();
                if let Some(&x) = bindings.0.get(&name).or(bindings.1.get(&name)) {
                    x
                } else {
                    panic!("no binding for {}", v);
                }
            }
            Expr::UnOp(op, x) => {
                let x = x.evaluate(bindings);
                match op {
                    UnOp::Minus => -x,
                }
            }
            Expr::BinOp(op, lhs, rhs) => {
                let lhs = lhs.evaluate(bindings);
                let rhs = rhs.evaluate(bindings);
                match op {
                    BinOp::Add => lhs + rhs,
                    BinOp::Sub => lhs - rhs,
                    BinOp::Mul => lhs * rhs,
                    BinOp::Div => lhs / rhs,
                    BinOp::Exp => lhs.powf(rhs),
                }
            }
            Expr::Function(f, x) => {
                let x = x.evaluate(bindings);
                match f {
                    Function::Sin => x.sin(),
                    Function::Cos => x.cos(),
                    Function::Tan => x.tan(),
                    Function::Asin => x.asin(),
                    Function::Acos => x.acos(),
                    Function::Atan => x.atan(),
                    Function::Sinh => x.sinh(),
                    Function::Cosh => x.cosh(),
                    Function::Tanh => x.tanh(),
                    Function::Asinh => x.asinh(),
                    Function::Acosh => x.acosh(),
                    Function::Atanh => x.atanh(),
                }
            }
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Number(x) => write!(f, "{}", x),
            Expr::Var(v) => write!(f, "{}", v),
            Expr::UnOp(op, x) => {
                let op = match op {
                    UnOp::Minus => "-",
                };
                write!(f, "({}{})", op, x)
            }
            Expr::BinOp(op, lhs, rhs) => {
                let op = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Exp => "^",
                };
                write!(f, "({} {} {})", lhs, op, rhs)
            }
            Expr::Function(fun, x) => write!(f, "{}({})", fun, x),
        }
    }
}
