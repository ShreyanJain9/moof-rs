/// Source location for error reporting
#[derive(Debug, Clone, Default)]
pub struct Loc {
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl Loc {
    pub fn new(line: usize, col: usize) -> Self {
        Loc { line: Some(line), column: Some(col) }
    }
    pub fn none() -> Self {
        Loc { line: None, column: None }
    }
}

/// All AST node types for Moof.
#[derive(Debug, Clone)]
pub enum Expr {
    // -- Literals --
    Integer(i64, Loc),
    Float(f64, Loc),
    Str(String, Loc),
    Bool(bool, Loc),
    Nil(Loc),
    Identifier(String, Loc),

    // -- Compound --
    MapLiteral(Vec<(String, Box<Expr>)>, Loc),
    Quote(Box<Expr>, Loc),
    Quasiquote(Box<Expr>, Loc),
    Unquote(Box<Expr>, Loc),
    UnquoteSplice(Box<Expr>, Loc),

    // -- Calls --
    Call(Box<Expr>, Vec<Expr>, Loc),                    // (func arg1 arg2)
    MessageSend(Box<Expr>, String, Vec<Expr>, Loc),     // [receiver selector args...]
    KeywordArg(String, Box<Expr>, Loc),                 // port: 443 inside a call

    // -- Special Forms --
    Define(String, Box<Expr>, Loc),                     // (define name value)
    DefineFunction(String, Vec<String>, Option<String>, Box<Expr>, Loc), // name, params, rest_param, body
    Lambda(Vec<String>, Option<String>, Box<Expr>, Loc), // params, rest_param, body
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>, Loc),   // condition, then, else
    Let(Vec<(String, Expr)>, Box<Expr>, Loc),           // bindings, body
    Do(Vec<Expr>, Loc),
    SetBang(String, Box<Expr>, Loc),
    TryCatch(Box<Expr>, String, Box<Expr>, Loc),        // body, error_name, catch_body
    Cond(Vec<(CondTest, Expr)>, Loc),
    And(Box<Expr>, Box<Expr>, Loc),
    Or(Box<Expr>, Box<Expr>, Loc),

    // -- Object System --
    ClassDef {
        name: String,
        superclass: Option<String>,
        fields: Vec<String>,
        methods: Vec<MethodDef>,
        traits: Vec<String>,
        loc: Loc,
    },
    TraitDef {
        name: String,
        methods: Vec<MethodDef>,
        loc: Loc,
    },

    // -- Pattern Matching --
    Match(Box<Expr>, Vec<MatchClause>, Loc),
    TypeDef(String, Vec<TypeVariant>, Loc),

    // -- Pipeline --
    Pipeline(Box<Expr>, Vec<Expr>, Loc),                // (-> value step1 step2)

    // -- String Interpolation --
    StringInterp(Vec<Expr>, Loc),                       // segments

    // -- Protocol --
    ProtocolDef(String, Vec<String>, Loc),               // name, selectors

    // -- Selector Ref --
    SelectorRef(String, Vec<Expr>, Loc),                 // selector, partial_args

    // -- Macros --
    DefMacro(String, Vec<String>, Box<Expr>, Loc),       // name, params, body

    // -- Module System --
    ModuleDef(String, Vec<String>, Vec<Expr>, Loc),      // name, exports, body
    UseModule(String, Option<Vec<String>>, Option<String>, Loc), // module_name, imports, alias
    Require(String, Loc),
}

#[derive(Debug, Clone)]
pub enum CondTest {
    Expr(Expr),
    Else,
}

#[derive(Debug, Clone)]
pub struct MethodDef {
    pub selector: String,
    pub params: Vec<String>,
    pub body: Box<Expr>,
    pub loc: Loc,
}

#[derive(Debug, Clone)]
pub struct MatchClause {
    pub pattern: Pattern,
    pub guard: Option<Box<Expr>>,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,
    Bind(String),
    Literal(Box<Expr>),         // integer, string, bool, nil
    List(Vec<Pattern>, Option<Box<Pattern>>),  // elements, rest
    Map(Vec<(String, Pattern)>),
    Constructor(String, Vec<Pattern>),   // class_name, bindings
}

#[derive(Debug, Clone)]
pub struct TypeVariant {
    pub name: String,
    pub fields: Vec<String>,
}

/// A program is a list of top-level expressions.
#[derive(Debug, Clone)]
pub struct Program {
    pub expressions: Vec<Expr>,
}

impl Expr {
    pub fn loc(&self) -> &Loc {
        match self {
            Expr::Integer(_, l) | Expr::Float(_, l) | Expr::Str(_, l) |
            Expr::Bool(_, l) | Expr::Nil(l) | Expr::Identifier(_, l) |
            Expr::MapLiteral(_, l) | Expr::Quote(_, l) | Expr::Quasiquote(_, l) |
            Expr::Unquote(_, l) | Expr::UnquoteSplice(_, l) |
            Expr::Call(_, _, l) | Expr::MessageSend(_, _, _, l) |
            Expr::KeywordArg(_, _, l) | Expr::Define(_, _, l) |
            Expr::DefineFunction(_, _, _, _, l) | Expr::Lambda(_, _, _, l) |
            Expr::If(_, _, _, l) | Expr::Let(_, _, l) | Expr::Do(_, l) |
            Expr::SetBang(_, _, l) | Expr::TryCatch(_, _, _, l) |
            Expr::Cond(_, l) | Expr::And(_, _, l) | Expr::Or(_, _, l) |
            Expr::Match(_, _, l) | Expr::TypeDef(_, _, l) |
            Expr::Pipeline(_, _, l) | Expr::StringInterp(_, l) |
            Expr::ProtocolDef(_, _, l) | Expr::SelectorRef(_, _, l) |
            Expr::DefMacro(_, _, _, l) | Expr::ModuleDef(_, _, _, l) |
            Expr::UseModule(_, _, _, l) | Expr::Require(_, l) => l,
            Expr::ClassDef { loc, .. } | Expr::TraitDef { loc, .. } => loc,
        }
    }
}
