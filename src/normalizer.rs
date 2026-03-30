use crate::ast::*;

pub fn normalize(program: Program) -> Program {
    Program {
        expressions: program.expressions.into_iter().map(normalize_expr).collect(),
    }
}

fn normalize_expr(expr: Expr) -> Expr {
    match expr {
        // MessageSend -> Call(__send, [receiver, selector_str, ...args])
        Expr::MessageSend(receiver, selector, args, loc) => {
            let recv = normalize_expr(*receiver);
            let norm_args: Vec<Expr> = args.into_iter().map(normalize_expr).collect();
            let ln = loc.line.unwrap_or(0);
            let col = loc.column.unwrap_or(0);
            make_send(recv, &selector, norm_args, ln, col)
        }

        // Call: normalize callee and args recursively
        Expr::Call(callee, args, loc) => {
            let callee = normalize_expr(*callee);
            let args: Vec<Expr> = args.into_iter().map(normalize_expr).collect();
            Expr::Call(Box::new(callee), args, loc)
        }

        // Define
        Expr::Define(name, value, loc) => {
            Expr::Define(name, Box::new(normalize_expr(*value)), loc)
        }

        // Lambda
        Expr::Lambda(params, rest, body, loc) => {
            Expr::Lambda(params, rest, Box::new(normalize_expr(*body)), loc)
        }

        // If
        Expr::If(cond, then_br, else_br, loc) => {
            Expr::If(
                Box::new(normalize_expr(*cond)),
                Box::new(normalize_expr(*then_br)),
                else_br.map(|e| Box::new(normalize_expr(*e))),
                loc,
            )
        }

        // Let
        Expr::Let(bindings, body, loc) => {
            let bindings = bindings
                .into_iter()
                .map(|(name, val)| (name, normalize_expr(val)))
                .collect();
            Expr::Let(bindings, Box::new(normalize_expr(*body)), loc)
        }

        // Do
        Expr::Do(exprs, loc) => {
            Expr::Do(exprs.into_iter().map(normalize_expr).collect(), loc)
        }

        // SetBang
        Expr::SetBang(name, value, loc) => {
            Expr::SetBang(name, Box::new(normalize_expr(*value)), loc)
        }

        // Quote: do NOT recurse into contents
        e @ Expr::Quote(_, _) => e,

        // Quasiquote: pass through (do not normalize contents per Ruby prototype comment)
        Expr::Quasiquote(inner, loc) => {
            Expr::Quasiquote(Box::new(normalize_expr(*inner)), loc)
        }

        // Unquote
        Expr::Unquote(inner, loc) => {
            Expr::Unquote(Box::new(normalize_expr(*inner)), loc)
        }

        // UnquoteSplice
        Expr::UnquoteSplice(inner, loc) => {
            Expr::UnquoteSplice(Box::new(normalize_expr(*inner)), loc)
        }

        // TryCatch
        Expr::TryCatch(body, err_name, catch_body, loc) => {
            Expr::TryCatch(
                Box::new(normalize_expr(*body)),
                err_name,
                Box::new(normalize_expr(*catch_body)),
                loc,
            )
        }

        // MapLiteral
        Expr::MapLiteral(pairs, loc) => {
            let pairs = pairs
                .into_iter()
                .map(|(k, v)| (k, Box::new(normalize_expr(*v))))
                .collect();
            Expr::MapLiteral(pairs, loc)
        }

        // ClassDef
        Expr::ClassDef { name, superclass, fields, methods, traits, loc } => {
            let methods = methods
                .into_iter()
                .map(|m| MethodDef {
                    selector: m.selector,
                    params: m.params,
                    body: Box::new(normalize_expr(*m.body)),
                    loc: m.loc,
                })
                .collect();
            Expr::ClassDef { name, superclass, fields, methods, traits, loc }
        }

        // TraitDef
        Expr::TraitDef { name, methods, loc } => {
            let methods = methods
                .into_iter()
                .map(|m| MethodDef {
                    selector: m.selector,
                    params: m.params,
                    body: Box::new(normalize_expr(*m.body)),
                    loc: m.loc,
                })
                .collect();
            Expr::TraitDef { name, methods, loc }
        }

        // Pipeline: desugar into nested calls
        Expr::Pipeline(value, steps, loc) => {
            desugar_pipeline(*value, steps, &loc)
        }

        // StringInterp: desugar into chained concat calls
        Expr::StringInterp(segments, loc) => {
            desugar_string_interp(segments, &loc)
        }

        // SelectorRef: desugar into lambda
        Expr::SelectorRef(selector, partial_args, loc) => {
            desugar_selector_ref(&selector, partial_args, &loc)
        }

        // Match
        Expr::Match(expr, clauses, loc) => {
            let clauses = clauses
                .into_iter()
                .map(|c| MatchClause {
                    pattern: normalize_pattern(c.pattern),
                    guard: c.guard.map(|g| Box::new(normalize_expr(*g))),
                    body: Box::new(normalize_expr(*c.body)),
                })
                .collect();
            Expr::Match(Box::new(normalize_expr(*expr)), clauses, loc)
        }

        // TypeDef: pass through
        e @ Expr::TypeDef(_, _, _) => e,

        // ProtocolDef: pass through
        e @ Expr::ProtocolDef(_, _, _) => e,

        // DefMacro: normalize body
        Expr::DefMacro(name, params, body, loc) => {
            Expr::DefMacro(name, params, Box::new(normalize_expr(*body)), loc)
        }

        // ModuleDef
        Expr::ModuleDef(name, exports, body, loc) => {
            let body = body.into_iter().map(normalize_expr).collect();
            Expr::ModuleDef(name, exports, body, loc)
        }

        // UseModule, Require: pass through
        e @ Expr::UseModule(_, _, _, _) => e,
        e @ Expr::Require(_, _) => e,

        // Leaf nodes: pass through unchanged
        e @ (Expr::Integer(_, _)
        | Expr::Float(_, _)
        | Expr::Str(_, _)
        | Expr::Bool(_, _)
        | Expr::Nil(_)
        | Expr::Identifier(_, _)) => e,
    }
}

/// Normalize pattern nodes (most are leaf nodes, but some contain sub-patterns).
fn normalize_pattern(pat: Pattern) -> Pattern {
    match pat {
        Pattern::List(elements, rest) => {
            let elements = elements.into_iter().map(normalize_pattern).collect();
            let rest = rest.map(|r| Box::new(normalize_pattern(*r)));
            Pattern::List(elements, rest)
        }
        Pattern::Map(pairs) => {
            let pairs = pairs
                .into_iter()
                .map(|(k, p)| (k, normalize_pattern(p)))
                .collect();
            Pattern::Map(pairs)
        }
        Pattern::Constructor(name, bindings) => {
            let bindings = bindings.into_iter().map(normalize_pattern).collect();
            Pattern::Constructor(name, bindings)
        }
        // Wildcard, Bind, Literal: pass through
        other => other,
    }
}

// ── Desugaring helpers ───────────────────────────────────────────────

/// Desugar (-> val step1 step2) into nested calls.
fn desugar_pipeline(value: Expr, steps: Vec<Expr>, loc: &Loc) -> Expr {
    let mut result = normalize_expr(value);
    let ln = loc.line.unwrap_or(0);
    let col = loc.column.unwrap_or(0);
    for step in steps {
        result = pipeline_inject(step, result, ln, col);
    }
    result
}

fn pipeline_inject(step: Expr, prev: Expr, ln: usize, col: usize) -> Expr {
    match step {
        // MessageSend: inject prev as receiver, then normalize
        Expr::MessageSend(_, selector, args, step_loc) => {
            let injected = Expr::MessageSend(
                Box::new(prev),
                selector,
                args,
                step_loc,
            );
            normalize_expr(injected)
        }
        // Call: inject prev as first argument
        Expr::Call(callee, args, step_loc) => {
            let normalized_callee = normalize_expr(*callee);
            let mut normalized_args = vec![prev];
            normalized_args.extend(args.into_iter().map(normalize_expr));
            Expr::Call(Box::new(normalized_callee), normalized_args, step_loc)
        }
        // Identifier: wrap as Call(step, [prev])
        e @ Expr::Identifier(_, _) => {
            Expr::Call(
                Box::new(e),
                vec![prev],
                Loc::new(ln, col),
            )
        }
        // Any other expression: wrap as a call with prev as argument
        other => {
            let normalized_step = normalize_expr(other);
            Expr::Call(
                Box::new(normalized_step),
                vec![prev],
                Loc::new(ln, col),
            )
        }
    }
}

/// Desugar $"hello \(name)" into chained concat calls via __send.
fn desugar_string_interp(segments: Vec<Expr>, loc: &Loc) -> Expr {
    let ln = loc.line.unwrap_or(0);
    let col = loc.column.unwrap_or(0);

    let parts: Vec<Expr> = segments
        .into_iter()
        .map(|seg| {
            let is_str = matches!(&seg, Expr::Str(_, _));
            let normalized = normalize_expr(seg);
            if is_str {
                normalized
            } else {
                // Call to_s on expression: (__send expr "to_s")
                make_send(normalized, "to_s", vec![], ln, col)
            }
        })
        .collect();

    if parts.is_empty() {
        return Expr::Str(String::new(), Loc::new(ln, col));
    }
    if parts.len() == 1 {
        return parts.into_iter().next().unwrap();
    }

    // Chain concat calls: (__send (__send part1 "concat:" part2) "concat:" part3)
    let mut iter = parts.into_iter();
    let mut result = iter.next().unwrap();
    for part in iter {
        result = make_send(result, "concat:", vec![part], ln, col);
    }
    result
}

/// Desugar &name into lambda wrapping __send.
fn desugar_selector_ref(selector: &str, partial_args: Vec<Expr>, loc: &Loc) -> Expr {
    let ln = loc.line.unwrap_or(0);
    let col = loc.column.unwrap_or(0);
    let obj_param = "__obj".to_string();
    let obj_id = Expr::Identifier(obj_param.clone(), Loc::new(ln, col));

    let norm_args: Vec<Expr> = partial_args.into_iter().map(normalize_expr).collect();
    let body = make_send(obj_id, selector, norm_args, ln, col);

    Expr::Lambda(vec![obj_param], None, Box::new(body), Loc::new(ln, col))
}

/// Helper: construct a __send call node.
fn make_send(receiver: Expr, selector: &str, args: Vec<Expr>, ln: usize, col: usize) -> Expr {
    let mut all_args = vec![
        receiver,
        Expr::Str(selector.to_string(), Loc::new(ln, col)),
    ];
    all_args.extend(args);
    Expr::Call(
        Box::new(Expr::Identifier("__send".to_string(), Loc::new(ln, col))),
        all_args,
        Loc::new(ln, col),
    )
}
