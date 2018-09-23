use std::vec::IntoIter;
use std::mem;
use std::fmt;
use std::collections::HashMap;

#[allow(unused_imports)]
use log;

macro_rules! try_block {
    ($($block:tt)*) => (
        (|| { ::std::ops::Try::from_ok({ $($block)* }) })()
    )
}

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

#[derive(Debug)]
pub struct Lexeme {
    kind: Token,
    string: String,
}

#[derive(PartialEq)]
enum MatchKind {
    Prefix,
    All,
}

impl Token {
    fn all() -> Vec<Token> {
        use self::Token::*;

        // Tokens with values are given dummy values, as they're simply used for matching purposes.
        vec![
            // `End` is deliberately not included as it is created implicitly.
            Number(0.0),
            Name(String::new()),
            OpenParen,
            CloseParen,
            Add,
            Sub,
            Mul,
            Div,
            Exp,
        ]
    }

    fn matches(&self, s: &str, kind: MatchKind) -> bool {
        use self::Token::*;

        match (self, s) {
            // Empty strings are trivially prefixes of every token.
            (_, "") => true,

            // Literal tokens.
            (OpenParen, "(") => true,
            (CloseParen, ")") => true,
            (Add, "+") => true,
            (Sub, "-") => true,
            (Mul, "*") => true,
            (Div, "/") => true,
            (Exp, "^") => true,

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

            (Name(_), s) => {
                s.chars().all(|c| c.is_ascii_alphabetic() && c.is_ascii_lowercase())
            }

            _ => false,
        }
    }
}

pub struct Lexer;

impl Lexer {
    pub fn scan(chars: Vec<char>) -> Result<Vec<Lexeme>, String> {
        let mut s;
        let mut states;
        let mut lexemes = vec![];

        let mut chars = chars.into_iter().peekable();

        let mut end = false;
        while !end {
            s = String::new();
            states = Token::all();

            end = loop {
                if let Some(&c) = chars.peek() {
                    if c.is_ascii_whitespace() {
                        chars.next();
                        break false;
                    }

                    let mut sn = s.clone();
                    sn.push(c);
                    let statesn: Vec<_> = states.clone().into_iter().filter(|t| t.matches(&sn, MatchKind::Prefix)).collect();
                    if !statesn.is_empty() || s.is_empty() {
                        chars.next();
                        s = sn;
                        states = statesn;
                    } else {
                        break false;
                    }
                } else {
                    break true;
                }
            };

            // Empty strings correspond to whitespace, so we can skip them.
            if !s.is_empty()  {
                let states: Vec<_> = states.into_iter().filter(|t| t.matches(&s, MatchKind::All)).collect();
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

    pub fn evaluate(lexemes: Vec<Lexeme>) -> Vec<Token> {
        let mut tokens = vec![];

        use self::Token::*;

        for l in lexemes.into_iter() {
            tokens.push(match l.kind {
                Number(_) => Number(l.string.parse().unwrap()),
                Name(_) => Name(l.string),
                _ => l.kind,
            });
        }

        tokens
    }
}

type ParseResult<T> = Result<T, ()>;

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

#[derive(PartialEq, PartialOrd, Clone, Copy, Debug)]
enum Precedence {
    Additive,
    Multiplicative,
    Exponential,
    Last,
}

impl Precedence {
    fn next(&self) -> Precedence {
        use self::Precedence::*;
        match self {
            Additive => Multiplicative,
            Multiplicative => Exponential,
            Exponential => Last,
            Last => panic!("tried to get a precedence after the last"),
        }
    }

    fn left_associative(&self) -> bool {
        use self::Precedence::*;
        match self {
            Additive => true,
            Multiplicative => true,
            Exponential => false,
            Last => panic!("tried to get the associativity of an unknown precedence"),
        }
    }
}

impl<I: Iterator<Item = Token> + Clone> Parser<I> {
    fn err<T>() -> ParseResult<T> {
        Err(())
    }

    fn bump(&mut self) {
        if let Token::End = self.token {
            panic!("tried to bump past end of input");
        }

        self.pos += 1;
        self.token = self.tokens.next().unwrap_or(Token::End);
    }

    fn check(&self, t: Token) -> ParseResult<()> {
        if self.token == t {
            Ok(())
        } else {
            Self::err()
        }
    }

    fn eat(&mut self, t: Token) -> ParseResult<()> {
        self.check(t)?;
        self.bump();
        Ok(())
    }

    fn check_end(&self) -> ParseResult<()> {
        if let Token::End = self.token {
            Ok(())
        } else {
            Self::err()
        }
    }

    fn save(&self) -> Self {
        (*self).clone()
    }

    fn restore(&mut self, save: Self) {
        mem::replace(self, save);
    }

    pub fn parse_equation(&mut self) -> ParseResult<Expr> {
        let expr = self.parse_expr(Precedence::Additive)?;
        self.check_end()?;
        Ok(expr)
    }

    // E_i ::= E_{i + 1} E_i'
    fn parse_expr(&mut self, precedence: Precedence) -> ParseResult<Expr> {
        if let Precedence::Last = precedence {
            self.parse_term()
        } else {
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
        }
    }

    // E_i' ::= O E_i E_i' | empty
    fn parse_expr_suffix(&mut self, precedence: Precedence) -> ParseResult<ExprSuffix> {
        let save = self.save();

        let op_expr: ParseResult<_> = try_block! {
            ExprSuffix::Chain {
                op: self.parse_bin_op(precedence)?,
                expr: self.parse_expr(precedence.next())?,
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
        let subexpr = self.parse_expr(precedence.next())?;
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
        use self::Token::*;
        use self::Precedence::*;
        let ops = match precedence {
            Additive => vec![(Add, BinOp::Add), (Sub, BinOp::Sub)],
            Multiplicative => vec![(Mul, BinOp::Mul), (Div, BinOp::Div)],
            Exponential => vec![(Exp, BinOp::Exp)],
            Last => panic!("tried to get binary operations with unknown precedence"),
        };
        self.parse_op(ops)
    }

    // U ::= -
    fn parse_prefix_un_op(&mut self, precedence: Precedence) -> ParseResult<UnOp> {
        use self::Token::*;
        use self::Precedence::*;
        let ops = match precedence {
            Additive => vec![(Sub, UnOp::Minus)],
            Multiplicative => vec![],
            Exponential => vec![],
            Last => panic!("tried to get unary operations with unknown precedence"),
        };
        self.parse_op(ops)
    }

    // T ::= ( E ) | V | X
    fn parse_term(&mut self) -> ParseResult<Expr> {
        let save1 = self.save();
        let save2 = self.save();

        let parenthesised_expr: ParseResult<_> = try_block! {
            self.eat(Token::OpenParen)?;
            let expr = self.parse_expr(Precedence::Additive)?;
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
        let token = self.token.clone();
        match token {
            Token::Name(ref n) if n.len() > 1 => {
                self.check_function_name(n)?;
                self.bump();
                self.eat(Token::OpenParen)?;
                let expr = self.parse_expr(Precedence::Additive)?;
                self.eat(Token::CloseParen)?;
                Ok(Expr::Function(n.clone(), box expr))
            }
            _ => Self::err(),
        }
    }

    fn check_function_name(&self, name: &String) -> ParseResult<()> {
        if ["sin", "cos", "tan"].contains(&name.as_ref()) {
            Ok(())
        } else {
            Err(())
        }
    }

    // V ::= 'a' ..= 'z'
    fn parse_var(&mut self) -> ParseResult<Expr> {
        let token = self.token.clone();
        match token {
            Token::Name(ref n) if n.len() == 1 => {
                self.bump();
                Ok(Expr::Var(n.clone()))
            }
            _ => Self::err(),
        }
    }

    // X ::= /\d+(\.\d+)?/
    fn parse_value(&mut self) -> ParseResult<Expr> {
        match self.token {
            Token::Number(v) => {
                self.bump();
                Ok(Expr::Number(v))
            }
            _ => Self::err(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Exp,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnOp {
    Minus,
}

#[derive(Debug)]
pub enum Expr {
    Number(f64),
    Var(String),
    UnOp(UnOp, Box<Expr>),
    BinOp(BinOp, Box<Expr>, Box<Expr>),
    Function(String, Box<Expr>),
}

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
    pub fn evaluate(&self, bindings: &HashMap<char, f64>) -> f64 {
        use self::Expr::*;

        match self {
            Number(x) => *x,
            Var(v) => {
                if let Some(&x) = bindings.get(&v.chars().next().unwrap()) {
                    x
                } else {
                    panic!("no binding for {}", v);
                }
            }
            UnOp(self::UnOp::Minus, x) => -x.evaluate(bindings),
            BinOp(op, lhs, rhs) => {
                use self::BinOp::*;
                let lhs = lhs.evaluate(bindings);
                let rhs = rhs.evaluate(bindings);
                match op {
                    Add => lhs + rhs,
                    Sub => lhs - rhs,
                    Mul => lhs * rhs,
                    Div => lhs / rhs,
                    Exp => lhs.powf(rhs),
                }
            }
            Function(f, x) => {
                let x = x.evaluate(bindings);
                match f.as_ref() {
                    "sin" => x.sin(),
                    "cos" => x.cos(),
                    "tan" => x.tan(),
                    _ => panic!("unknown function {}", f),
                }
            }
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Expr::*;

        match self {
            Number(x) => write!(f, "{}", x),
            UnOp(self::UnOp::Minus, x) => write!(f, "(-{})", x),
            BinOp(op, lhs, rhs) => {
                use self::BinOp::*;
                let op = match op {
                    Add => "+",
                    Sub => "-",
                    Mul => "*",
                    Div => "/",
                    Exp => "^",
                };
                write!(f, "({} {} {})", lhs, op, rhs)
            }
            Var(v) => write!(f, "{}", v),
            Function(fun, x) => write!(f, "{}({})", fun, x),
        }
    }
}
