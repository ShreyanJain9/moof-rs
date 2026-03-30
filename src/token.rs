#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Delimiters
    LParen, RParen,
    LBracket, RBracket,
    LBrace, RBrace,

    // Literals
    Integer(i64),
    Float(f64),
    Str(String),
    True, False, Nil,

    // Identifiers and keywords
    Identifier(String),
    ColonId(String),    // keyword selector: "insertValue:"

    // Special
    Quote,              // '
    Dot,                // .
    Pipe,               // |
    Backtick,           // `
    Comma,              // ,
    CommaAt,            // ,@
    Ampersand,          // &
    InterpString(Vec<InterpSegment>), // $"..."

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpSegment {
    Str(String),
    Expr(String),       // source code to be re-parsed
}

#[derive(Debug, Clone)]
pub struct Token {
    pub ty: TokenType,
    pub line: usize,
    pub column: usize,
}

impl Token {
    pub fn new(ty: TokenType, line: usize, column: usize) -> Self {
        Token { ty, line, column }
    }
}
