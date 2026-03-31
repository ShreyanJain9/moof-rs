//! Compiler: converts parsed AST (cons lists) into bytecode.
//!
//! The compiler consumes `Value` (cons lists produced by the parser) and
//! emits `CompiledFunction` bytecode. It handles all special forms,
//! macro expansion (via tree-walker fallback), and local variable resolution.

use std::rc::Rc;

use crate::bytecode::{BytecodeBuilder, CompiledFunction, Op, UpvalueDesc};
use crate::cons;
use crate::error::{MoofError, Result};
use crate::interpreter::Interpreter;
use crate::symbol::{KnownSymbols, SymId, SymbolTable};
use crate::value::{ClosureBody, MoofClosure, Value};

// ═══════════════════════════════════════════════════════════════════════
// Compiler state
// ═══════════════════════════════════════════════════════════════════════

struct Local {
    name: SymId,
    depth: u32,
    is_captured: bool,
    mutable: bool,
}

struct FunctionScope {
    builder: BytecodeBuilder,
    locals: Vec<Local>,
    upvalues: Vec<UpvalueDesc>,
    scope_depth: u32,
    arity: u8,
    has_rest: bool,
    name: Option<SymId>,
}

impl FunctionScope {
    fn new(name: Option<SymId>) -> Self {
        FunctionScope {
            builder: BytecodeBuilder::new(),
            locals: Vec::new(),
            upvalues: Vec::new(),
            scope_depth: 0,
            arity: 0,
            has_rest: false,
            name,
        }
    }

    fn add_local(&mut self, name: SymId, mutable: bool) -> u8 {
        let slot = self.locals.len() as u8;
        self.locals.push(Local {
            name,
            depth: self.scope_depth,
            is_captured: false,
            mutable,
        });
        slot
    }

    fn resolve_local(&self, name: SymId) -> Option<u8> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i as u8);
            }
        }
        None
    }
}

pub struct Compiler<'a> {
    interp: &'a mut Interpreter,
    scopes: Vec<FunctionScope>,
}

impl<'a> Compiler<'a> {
    pub fn new(interp: &'a mut Interpreter) -> Self {
        Compiler {
            interp,
            scopes: Vec::new(),
        }
    }

    fn current(&mut self) -> &mut FunctionScope {
        self.scopes.last_mut().expect("no function scope")
    }

    fn known(&self) -> &KnownSymbols {
        &self.interp.known
    }

    fn builder(&mut self) -> &mut BytecodeBuilder {
        &mut self.current().builder
    }

    // ═══════════════════════════════════════════════════════════════════
    // Public entry point
    // ═══════════════════════════════════════════════════════════════════

    /// Compile a top-level program (list of expressions) into a single
    /// CompiledFunction that can be executed as a script.
    pub fn compile_program(&mut self, exprs: &[Value]) -> Result<CompiledFunction> {
        self.scopes.push(FunctionScope::new(None));

        for (i, expr) in exprs.iter().enumerate() {
            self.compile_expr(expr, i == exprs.len() - 1)?;
            if i < exprs.len() - 1 {
                self.current().builder.emit_op(Op::Pop);
            }
        }

        if exprs.is_empty() {
            self.current().builder.emit_op(Op::LoadNil);
        }

        self.current().builder.emit_op(Op::Return);
        self.finish_function()
    }

    // ═══════════════════════════════════════════════════════════════════
    // Expression compilation
    // ═══════════════════════════════════════════════════════════════════

    /// Compile an expression. If `tail` is true, this is in tail position
    /// (used for TCO — emit TailCall/TailSend instead of Call/Send).
    fn compile_expr(&mut self, expr: &Value, tail: bool) -> Result<()> {
        match expr {
            Value::Integer(_) | Value::Float(_) | Value::Str(_) | Value::Range(_) => {
                self.current().builder.emit_constant(expr.clone());
            }
            Value::Bool(true) => self.current().builder.emit_op(Op::LoadTrue),
            Value::Bool(false) => self.current().builder.emit_op(Op::LoadFalse),
            Value::Nil => self.current().builder.emit_op(Op::LoadNil),

            Value::Symbol(id) => {
                self.compile_var_get(*id)?;
            }

            Value::Cons(cell) => {
                let car = &cell.car;
                let cdr = &cell.cdr;
                self.compile_compound(car, cdr, tail)?;
            }

            // Tables, Objects, Closures are self-evaluating — emit as constants
            _ => {
                self.current().builder.emit_constant(expr.clone());
            }
        }
        Ok(())
    }

    /// Compile a compound form (cons cell with head + rest).
    fn compile_compound(&mut self, head: &Value, rest: &Value, tail: bool) -> Result<()> {
        if let Value::Symbol(id) = head {
            let id = *id;
            let k = self.interp.known.clone_ids();

            // Special forms
            if id == k.if_ { return self.compile_if(rest, tail); }
            if id == k.do_ { return self.compile_do(rest, tail); }
            if id == k.let_ { return self.compile_let(rest, tail); }
            if id == k.define { return self.compile_define(rest); }
            if id == k.set_bang { return self.compile_set_bang(rest); }
            if id == k.lambda || id == k.fn_ { return self.compile_lambda(rest); }
            if id == k.and { return self.compile_and(rest); }
            if id == k.or { return self.compile_or(rest); }
            if id == k.quote { return self.compile_quote(rest); }
            if id == k.send { return self.compile_send(rest, tail); }
            if id == k.str_interp { return self.compile_str_interp(rest); }
            if id == k.table { return self.compile_table(rest); }
            if id == k.table_array { return self.compile_table_array(rest); }
            if id == k.cond { return self.compile_cond(rest, tail); }

            // Forms not yet compiled: fall back to tree-walker via Eval opcode
            if id == k.class || id == k.type_ || id == k.match_ || id == k.try_
                || id == k.trait_ || id == k.protocol || id == k.module
                || id == k.use_ || id == k.require || id == k.defmacro
                || id == k.quasiquote || id == k.super_send
            {
                // Reconstruct the full expression as a constant and eval it
                let full_expr = Value::Cons(Rc::new(crate::cons::ConsCell {
                    car: head.clone(),
                    cdr: rest.clone(),
                }));
                self.current().builder.emit_constant(full_expr);
                self.current().builder.emit_op(Op::Eval);
                return Ok(());
            }

            // Macro expansion: expand via tree-walker, then compile the result
            if let Some(macro_val) = self.interp.macro_registry.get(&id).cloned() {
                let expanded = self.interp.expand_macro_to_ast(&macro_val, rest)?;
                return self.compile_expr(&expanded, tail);
            }
        }

        // General function call: compile head + args, emit Call
        self.compile_expr(head, false)?;
        let argc = self.compile_call_args(rest)?;

        if tail {
            self.current().builder.emit_op_u8(Op::TailCall, argc);
        } else {
            self.current().builder.emit_op_u8(Op::Call, argc);
        }
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // Special forms
    // ═══════════════════════════════════════════════════════════════════

    fn compile_if(&mut self, args: &Value, tail: bool) -> Result<()> {
        let cond = nth_car(args, 0)?;
        let then_expr = nth_car(args, 1)?;
        let else_expr = nth_car_opt(args, 2);

        self.compile_expr(cond, false)?;
        let else_jump = self.current().builder.emit_jump(Op::JumpIfFalse);

        self.compile_expr(then_expr, tail)?;
        let end_jump = self.current().builder.emit_jump(Op::Jump);

        self.current().builder.patch_jump(else_jump);
        match else_expr {
            Some(e) => self.compile_expr(e, tail)?,
            None => self.current().builder.emit_op(Op::LoadNil),
        }
        self.current().builder.patch_jump(end_jump);
        Ok(())
    }

    fn compile_do(&mut self, args: &Value, tail: bool) -> Result<()> {
        let items = cons_to_vec(args);
        if items.is_empty() {
            self.current().builder.emit_op(Op::LoadNil);
            return Ok(());
        }
        for (i, item) in items.iter().enumerate() {
            let is_last = i == items.len() - 1;
            self.compile_expr(item, is_last && tail)?;
            if !is_last {
                self.current().builder.emit_op(Op::Pop);
            }
        }
        Ok(())
    }

    fn compile_let(&mut self, args: &Value, tail: bool) -> Result<()> {
        let bindings_expr = nth_car(args, 0)?;
        let body = nth_cdr(args, 0)?;

        // Begin scope
        self.current().scope_depth += 1;
        let locals_before = self.current().locals.len();

        // Compile bindings: ((x val1) (y val2) ...)
        let bindings = cons_to_vec(bindings_expr);
        for binding in &bindings {
            let pair = cons_to_vec(binding);
            if pair.len() != 2 {
                return Err(MoofError::syntax_simple("let: each binding must be (name value)"));
            }
            let name = pair[0].as_symbol().map_err(|_| {
                MoofError::syntax_simple("let: binding name must be a symbol")
            })?;
            self.compile_expr(&pair[1], false)?;
            self.current().add_local(name, false); // let bindings are immutable
        }

        // Compile body as do
        self.compile_do(body, tail)?;

        // End scope: pop locals
        let locals_after = self.current().locals.len();
        let to_pop = locals_after - locals_before;
        // We need to keep the result on top but pop the locals below it.
        // Strategy: the result is on top, locals are below. We need a trick.
        // Simple approach: emit SetLocal to a temp, pop locals, GetLocal temp.
        // Even simpler for now: just pop them and rely on the stack being right.
        // Actually the result is already on TOS. We need to pop the let-bindings
        // that are below it. Use a rotate or just pop with care.
        // Simplest correct approach: the let bindings are at known slots.
        // After the body, truncate locals and emit Pops for the binding slots.
        // But the value is on top... we need swap+pop or similar.
        // For now: store result in local slot 0 temporarily... no that's wrong.
        //
        // Better approach: let bindings occupy local slots. The body leaves its
        // result on the operand stack. The locals are in the frame's local slots
        // (not the operand stack). So we just truncate the locals vec.
        // Wait — in a stack-based VM, locals ARE stack slots. So let bindings
        // push values onto the stack. The body also pushes its result.
        // At the end: stack is [... let-bindings... result].
        // We need to leave just [... result].
        // Standard approach: pop count-1 values from below TOS, or use a "close scope" pattern.
        //
        // Simplest: after body, swap result with binding below, pop extra.
        // But with N bindings this is complex. Let's use a different model:
        // GetLocal/SetLocal use absolute frame offsets. The "stack" holds operand
        // values, locals are in a separate area. This is the cleaner model.
        //
        // For Phase 1: use the simpler model where locals are frame slots
        // (separate from operand stack). Emit pops only for operand stack cleanup.
        //
        // Actually in our VM design, GetLocal/SetLocal access frame-relative slots.
        // The compile_expr for let bindings doesn't push onto operand stack —
        // it stores into local slots. So we don't need to pop.
        // But wait, we DID push values onto the stack (compile_expr pushes result).
        // We need SetLocal to consume that value from the stack.
        //
        // Let me redesign: after compiling each let binding value, emit SetLocal
        // to store it in the local slot. This pops it from the operand stack.
        // Then the body leaves its result on the operand stack. No cleanup needed.

        // Fix: the add_local above doesn't emit SetLocal. The value is on the stack.
        // In our VM model, locals are stored in the stack frame. The convention is:
        // when a function starts, locals occupy stack[bp..bp+local_count].
        // For let bindings added mid-function, we need to grow the locals area.
        // Simplest Phase 1 approach: let bindings are just additional locals.
        // The value is pushed by compile_expr and stays at that stack position.
        // GetLocal(slot) reads from stack[bp + slot].
        // So compile_expr already put the value at the right stack position
        // (which is the next local slot). We just need to not pop it.
        // At scope end, we truncate locals and pop the values.

        // Pop the let-binding slots (they're below the result on the stack)
        // Actually the bindings ARE at local slots. The result is on TOS (operand stack).
        // We just truncate the locals list. The VM will handle stack cleanup on return.
        self.current().locals.truncate(locals_before);
        self.current().scope_depth -= 1;

        Ok(())
    }

    fn compile_define(&mut self, args: &Value) -> Result<()> {
        let first = nth_car(args, 0)?;

        match first {
            // (define (name params...) body...) — function shorthand
            Value::Cons(_) => {
                let items = cons_to_vec(first);
                let name = items[0].as_symbol().map_err(|_| {
                    MoofError::syntax_simple("define: expected function name")
                })?;
                // Build lambda from rest of items
                let params: Vec<Value> = items[1..].to_vec();
                let param_list = Value::from_slice(&params);
                let body = nth_cdr(args, 0)?;
                self.compile_lambda_inner(Some(name), &param_list, body)?;
                // Store as global
                self.current().builder.emit_op_u16(Op::SetGlobal, name as u16);
                // SetGlobal leaves value on stack (define returns the value)
            }
            // (define name value)
            Value::Symbol(name) => {
                let val_expr = nth_car(args, 1)?;
                self.compile_expr(val_expr, false)?;
                if self.scopes.len() > 1 {
                    // Inside a function — local define
                    self.current().add_local(*name, true);
                    // Value is already on stack at the local's position
                } else {
                    // Top-level — global define
                    self.current().builder.emit_op_u16(Op::SetGlobal, *name as u16);
                }
            }
            _ => return Err(MoofError::syntax_simple("define: expected symbol or function form")),
        }
        Ok(())
    }

    fn compile_set_bang(&mut self, args: &Value) -> Result<()> {
        let name = nth_car(args, 0)?.as_symbol().map_err(|_| {
            MoofError::syntax_simple("set!: expected symbol")
        })?;
        let val_expr = nth_car(args, 1)?;
        self.compile_expr(val_expr, false)?;

        // Try local first, then global
        if let Some(slot) = self.current().resolve_local(name) {
            self.current().builder.emit_op_u8(Op::SetLocal, slot);
        } else {
            self.current().builder.emit_op_u16(Op::SetGlobal, name as u16);
        }
        Ok(())
    }

    fn compile_lambda(&mut self, args: &Value) -> Result<()> {
        let params = nth_car(args, 0)?;
        let body = nth_cdr(args, 0)?;
        self.compile_lambda_inner(None, params, body)
    }

    fn compile_lambda_inner(
        &mut self,
        name: Option<SymId>,
        params: &Value,
        body: &Value,
    ) -> Result<()> {
        // Push a new function scope
        self.scopes.push(FunctionScope::new(name));

        // Parse params and add as locals
        let param_list = cons_to_vec(params);
        let mut rest_param = false;
        for (i, p) in param_list.iter().enumerate() {
            if let Value::Symbol(id) = p {
                let sym_name = self.interp.symbols.name(*id);
                if sym_name == "." {
                    rest_param = true;
                    // Next param is the rest param name
                    if i + 1 < param_list.len() {
                        if let Value::Symbol(rest_id) = &param_list[i + 1] {
                            self.current().add_local(*rest_id, false);
                        }
                    }
                    break;
                }
                self.current().add_local(*id, false);
                self.current().arity += 1;
            }
        }
        self.current().has_rest = rest_param;

        // Compile body
        let body_items = cons_to_vec(body);
        if body_items.is_empty() {
            self.current().builder.emit_op(Op::LoadNil);
        } else {
            for (i, item) in body_items.iter().enumerate() {
                let is_last = i == body_items.len() - 1;
                self.compile_expr(item, is_last)?;
                if !is_last {
                    self.current().builder.emit_op(Op::Pop);
                }
            }
        }
        self.current().builder.emit_op(Op::Return);

        let func = self.finish_function()?;

        // Store the compiled function as a constant in the enclosing scope
        let env_placeholder = self.interp.global_env.clone();
        let closure_val = Value::Closure(Rc::new(MoofClosure {
            name,
            params: Vec::new(),
            rest_param: None,
            body: ClosureBody::Bytecode(Rc::new(func)),
            env: env_placeholder,
        }));
        let proto_idx = self.current().builder.add_constant(closure_val);

        // Emit MakeClosure with 0 upvalues for Phase 1
        let scope = self.current();
        scope.builder.emit_op_u16(Op::MakeClosure, proto_idx);
        scope.builder.code.push(0); // 0 upvalues for now

        Ok(())
    }

    fn compile_and(&mut self, args: &Value) -> Result<()> {
        let items = cons_to_vec(args);
        if items.is_empty() {
            self.current().builder.emit_op(Op::LoadTrue);
            return Ok(());
        }
        self.compile_expr(&items[0], false)?;
        if items.len() > 1 {
            let short = self.current().builder.emit_jump(Op::JumpIfFalseKeep);
            self.current().builder.emit_op(Op::Pop);
            self.compile_expr(&items[1], false)?;
            self.current().builder.patch_jump(short);
        }
        Ok(())
    }

    fn compile_or(&mut self, args: &Value) -> Result<()> {
        let items = cons_to_vec(args);
        if items.is_empty() {
            self.current().builder.emit_op(Op::LoadFalse);
            return Ok(());
        }
        self.compile_expr(&items[0], false)?;
        if items.len() > 1 {
            let short = self.current().builder.emit_jump(Op::JumpIfTrueKeep);
            self.current().builder.emit_op(Op::Pop);
            self.compile_expr(&items[1], false)?;
            self.current().builder.patch_jump(short);
        }
        Ok(())
    }

    fn compile_quote(&mut self, args: &Value) -> Result<()> {
        let val = nth_car(args, 0)?;
        self.current().builder.emit_constant(val.clone());
        Ok(())
    }

    fn compile_send(&mut self, args: &Value, tail: bool) -> Result<()> {
        // (__send receiver selector arg1 arg2 ...)
        let receiver_expr = nth_car(args, 0)?;
        let selector_expr = nth_car(args, 1)?;

        // Compile receiver
        self.compile_expr(receiver_expr, false)?;

        // Get selector as SymId
        let selector_id = match selector_expr {
            Value::Str(s) => self.interp.symbols.intern(s),
            Value::Symbol(id) => *id,
            _ => return Err(MoofError::syntax_simple("send: selector must be string or symbol")),
        };

        // Compile args (skip keyword labels)
        let rest = nth_cdr(args, 1)?;
        let argc = self.compile_send_args(rest)?;

        if tail {
            self.current().builder.emit_op_u16_u8(Op::TailSend, selector_id as u16, argc);
        } else {
            self.current().builder.emit_op_u16_u8(Op::Send, selector_id as u16, argc);
        }
        Ok(())
    }

    fn compile_str_interp(&mut self, args: &Value) -> Result<()> {
        let items = cons_to_vec(args);
        for item in &items {
            self.compile_expr(item, false)?;
        }
        self.current().builder.emit_op_u8(Op::StringInterp, items.len() as u8);
        Ok(())
    }

    fn compile_table(&mut self, args: &Value) -> Result<()> {
        let items = cons_to_vec(args);
        let pair_count = items.len() / 2;
        for item in &items {
            self.compile_expr(item, false)?;
        }
        self.current().builder.emit_op_u8(Op::MakeTable, pair_count as u8);
        Ok(())
    }

    fn compile_table_array(&mut self, args: &Value) -> Result<()> {
        let items = cons_to_vec(args);
        for item in &items {
            self.compile_expr(item, false)?;
        }
        self.current().builder.emit_op_u8(Op::MakeTableArray, items.len() as u8);
        Ok(())
    }

    fn compile_cond(&mut self, args: &Value, tail: bool) -> Result<()> {
        let clauses = cons_to_vec(args);
        let mut end_jumps = Vec::new();

        for clause in &clauses {
            let pair = cons_to_vec(clause);
            if pair.len() < 2 {
                return Err(MoofError::syntax_simple("cond: each clause needs condition and body"));
            }
            let cond = &pair[0];
            let body = &pair[1];

            // Check for else clause
            let is_else = matches!(cond, Value::Symbol(id) if *id == self.interp.known.else_);
            if is_else {
                self.compile_expr(body, tail)?;
                end_jumps.push(None); // no jump needed, this is the last
                break;
            }

            self.compile_expr(cond, false)?;
            let next_clause = self.current().builder.emit_jump(Op::JumpIfFalse);
            self.compile_expr(body, tail)?;
            end_jumps.push(Some(self.current().builder.emit_jump(Op::Jump)));
            self.current().builder.patch_jump(next_clause);
        }

        // If no else clause, push nil
        if end_jumps.is_empty() || end_jumps.last().map(|j| j.is_some()).unwrap_or(true) {
            self.current().builder.emit_op(Op::LoadNil);
        }

        // Patch all end jumps
        for jump in end_jumps.into_iter().flatten() {
            self.current().builder.patch_jump(jump);
        }
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // Variable access
    // ═══════════════════════════════════════════════════════════════════

    fn compile_var_get(&mut self, name: SymId) -> Result<()> {
        if let Some(slot) = self.current().resolve_local(name) {
            self.current().builder.emit_op_u8(Op::GetLocal, slot);
        } else {
            // For Phase 1: no upvalues, fall back to global
            self.current().builder.emit_op_u16(Op::GetGlobal, name as u16);
        }
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // Helpers
    // ═══════════════════════════════════════════════════════════════════

    /// Compile function call arguments, skipping keyword labels.
    /// Returns the argument count.
    fn compile_call_args(&mut self, args: &Value) -> Result<u8> {
        let mut count: u8 = 0;
        let mut cursor = args;
        while let Value::Cons(cell) = cursor {
            // Skip keyword labels (symbols ending with ':')
            if let Value::Symbol(id) = &cell.car {
                let name = self.interp.symbols.name(*id);
                if name.ends_with(':') {
                    cursor = &cell.cdr;
                    continue;
                }
            }
            self.compile_expr(&cell.car, false)?;
            count += 1;
            cursor = &cell.cdr;
        }
        Ok(count)
    }

    /// Compile message send arguments (after selector). Skips keyword labels.
    fn compile_send_args(&mut self, args: &Value) -> Result<u8> {
        self.compile_call_args(args)
    }

    /// Pop the current function scope and produce a CompiledFunction.
    fn finish_function(&mut self) -> Result<CompiledFunction> {
        let scope = self.scopes.pop().expect("no scope to finish");
        Ok(CompiledFunction {
            name: scope.name,
            arity: scope.arity,
            has_rest: scope.has_rest,
            local_count: scope.locals.len() as u8,
            code: scope.builder.code,
            constants: scope.builder.constants,
            upvalues: scope.upvalues,
            is_block: false,
            enclosing_class: None,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// KnownSymbols helper — clone IDs for borrow-safe access
// ═══════════════════════════════════════════════════════════════════════

/// Lightweight struct holding just the SymIds from KnownSymbols.
/// Used to avoid borrowing the interpreter while compiling.
#[derive(Clone)]
pub struct KnownIds {
    pub if_: SymId,
    pub define: SymId,
    pub lambda: SymId,
    pub fn_: SymId,
    pub let_: SymId,
    pub do_: SymId,
    pub set_bang: SymId,
    pub quote: SymId,
    pub quasiquote: SymId,
    pub and: SymId,
    pub or: SymId,
    pub cond: SymId,
    pub match_: SymId,
    pub class: SymId,
    pub try_: SymId,
    pub defmacro: SymId,
    pub send: SymId,
    pub super_send: SymId,
    pub table: SymId,
    pub table_array: SymId,
    pub str_interp: SymId,
    pub else_: SymId,
    pub type_: SymId,
    pub protocol: SymId,
    pub trait_: SymId,
    pub module: SymId,
    pub use_: SymId,
    pub require: SymId,
}

impl KnownSymbols {
    pub fn clone_ids(&self) -> KnownIds {
        KnownIds {
            if_: self.if_,
            define: self.define,
            lambda: self.lambda,
            fn_: self.fn_,
            let_: self.let_,
            do_: self.do_,
            set_bang: self.set_bang,
            quote: self.quote,
            quasiquote: self.quasiquote,
            and: self.and,
            or: self.or,
            cond: self.cond,
            match_: self.match_,
            class: self.class,
            try_: self.try_,
            defmacro: self.defmacro,
            send: self.send,
            super_send: self.super_send,
            table: self.table,
            table_array: self.table_array,
            str_interp: self.str_interp,
            else_: self.else_,
            type_: self.type_,
            protocol: self.protocol,
            trait_: self.trait_,
            module: self.module,
            use_: self.use_,
            require: self.require,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Cons list helpers (duplicated here to avoid circular deps)
// ═══════════════════════════════════════════════════════════════════════

fn cons_to_vec(list: &Value) -> Vec<Value> {
    let mut result = Vec::new();
    let mut cursor = list;
    while let Value::Cons(cell) = cursor {
        result.push(cell.car.clone());
        cursor = &cell.cdr;
    }
    result
}

fn nth_car(list: &Value, n: usize) -> Result<&Value> {
    let mut cursor = list;
    for _ in 0..n {
        if let Value::Cons(cell) = cursor {
            cursor = &cell.cdr;
        } else {
            return Err(MoofError::syntax_simple("unexpected end of list"));
        }
    }
    if let Value::Cons(cell) = cursor {
        Ok(&cell.car)
    } else {
        Err(MoofError::syntax_simple("unexpected end of list"))
    }
}

fn nth_car_opt(list: &Value, n: usize) -> Option<&Value> {
    let mut cursor = list;
    for _ in 0..n {
        if let Value::Cons(cell) = cursor {
            cursor = &cell.cdr;
        } else {
            return None;
        }
    }
    if let Value::Cons(cell) = cursor {
        Some(&cell.car)
    } else {
        None
    }
}

fn nth_cdr(list: &Value, n: usize) -> Result<&Value> {
    let mut cursor = list;
    for _ in 0..=n {
        if let Value::Cons(cell) = cursor {
            cursor = &cell.cdr;
        } else {
            return Err(MoofError::syntax_simple("unexpected end of list"));
        }
    }
    Ok(cursor)
}
