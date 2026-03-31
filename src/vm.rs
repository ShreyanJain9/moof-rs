//! Stack-based virtual machine for executing Moof bytecode.

use std::cell::RefCell;
use std::rc::Rc;

use crate::bytecode::{self, CompiledFunction, Op};
use crate::error::{MoofError, Result};
use crate::interpreter::{call_closure, Interpreter};
use crate::moofint::MoofInt;
use crate::value::{ClosureBody, MoofClosure, MoofTable, Value};

// ═══════════════════════════════════════════════════════════════════════
// VM types
// ═══════════════════════════════════════════════════════════════════════

struct CallFrame {
    /// The compiled function being executed
    func: Rc<CompiledFunction>,
    /// Instruction pointer
    ip: usize,
    /// Base pointer: index into the value stack where this frame's locals start
    bp: usize,
}

impl CallFrame {
    fn read_u8(&mut self) -> u8 {
        let v = self.func.code[self.ip];
        self.ip += 1;
        v
    }

    fn read_u16(&mut self) -> u16 {
        let v = bytecode::read_u16(&self.func.code, self.ip);
        self.ip += 2;
        v
    }
}

/// Monomorphic inline cache entry for a Send site.
struct InlineCache {
    /// Raw pointer to the class (for identity comparison)
    class_ptr: usize,
    /// Cached method value
    method: Value,
}

pub struct VM {
    /// The tree-walking interpreter (for builtins, class system, symbol table)
    pub interp: Interpreter,
    /// Operand stack
    stack: Vec<Value>,
    /// Call frame stack
    frames: Vec<CallFrame>,
    /// Inline cache: keyed by (func_ptr, bytecode_offset) pair
    /// Using a flat Vec indexed by a hash for speed.
    send_cache: Vec<Option<InlineCache>>,
}

impl VM {
    pub fn new(interp: Interpreter) -> Self {
        // Pre-allocate cache with 1024 slots (power of 2 for fast modulo)
        let mut send_cache = Vec::with_capacity(1024);
        send_cache.resize_with(1024, || None);
        VM {
            interp,
            stack: Vec::with_capacity(256),
            frames: Vec::with_capacity(64),
            send_cache,
        }
    }

    /// Execute a compiled function as the top-level script.
    pub fn execute(&mut self, func: CompiledFunction) -> Result<Value> {
        let func = Rc::new(func);

        // Reserve space for locals
        let bp = self.stack.len();
        for _ in 0..func.local_count {
            self.stack.push(Value::Nil);
        }

        self.frames.push(CallFrame { func, ip: 0, bp });
        self.run()
    }

    // ═══════════════════════════════════════════════════════════════════
    // Main execution loop
    // ═══════════════════════════════════════════════════════════════════

    fn run(&mut self) -> Result<Value> {
        loop {
            let frame = self.frames.last_mut().unwrap();
            let op_byte = frame.read_u8();

            let op = Op::from_u8(op_byte).ok_or_else(|| {
                MoofError::runtime(format!("VM: unknown opcode 0x{:02x}", op_byte))
            })?;

            match op {
                Op::Nop => {}

                Op::Pop => {
                    self.stack.pop();
                }

                Op::Dup => {
                    let val = self.stack.last().cloned().unwrap_or(Value::Nil);
                    self.stack.push(val);
                }

                Op::LoadConst => {
                    let idx = frame.read_u16() as usize;
                    let val = frame.func.constants[idx].clone();
                    self.stack.push(val);
                }

                Op::LoadNil => self.stack.push(Value::Nil),
                Op::LoadTrue => self.stack.push(Value::Bool(true)),
                Op::LoadFalse => self.stack.push(Value::Bool(false)),

                Op::GetLocal => {
                    let slot = frame.read_u8() as usize;
                    let bp = frame.bp;
                    let val = self.stack[bp + slot].clone();
                    self.stack.push(val);
                }

                Op::SetLocal => {
                    let slot = frame.read_u8() as usize;
                    let bp = frame.bp;
                    let val = self.stack.last().cloned().unwrap_or(Value::Nil);
                    self.stack[bp + slot] = val;
                    // SetLocal leaves value on stack
                }

                Op::GetUpvalue => {
                    let _idx = frame.read_u8();
                    // Phase 1: upvalues not yet implemented
                    self.stack.push(Value::Nil);
                }

                Op::SetUpvalue => {
                    let _idx = frame.read_u8();
                    // Phase 1: upvalues not yet implemented
                }

                Op::GetGlobal => {
                    let sym_id = frame.read_u16() as u32;
                    let val = self.interp.global_env.get(sym_id).map_err(|_| {
                        let name = self.interp.symbols.name(sym_id);
                        MoofError::name(format!("Undefined variable: {name}"))
                    })?;
                    self.stack.push(val);
                }

                Op::SetGlobal => {
                    let sym_id = frame.read_u16() as u32;
                    let val = self.stack.last().cloned().unwrap_or(Value::Nil);
                    self.interp.global_env.define(sym_id, val, true);
                    // SetGlobal leaves value on stack
                }

                Op::Send => {
                    // Cache key: the bytecode offset of this Send instruction
                    let cache_key = (Rc::as_ptr(&frame.func) as usize)
                        .wrapping_add(frame.ip - 1); // ip was advanced past opcode
                    let selector = frame.read_u16() as u32;
                    let argc = frame.read_u8() as usize;
                    self.dispatch_send_cached(selector, argc, cache_key)?;
                }

                Op::TailSend => {
                    let selector = frame.read_u16() as u32;
                    let argc = frame.read_u8() as usize;
                    // For now, same as Send (message sends go through interp which has its own TCO)
                    self.dispatch_send(selector, argc)?;
                }

                Op::SendSuper | Op::TailSendSuper => {
                    let _selector = frame.read_u16();
                    let _argc = frame.read_u8();
                    // Phase 1: super sends not yet in VM
                    return Err(MoofError::runtime("VM: super sends not yet implemented"));
                }

                Op::Jump => {
                    let target = frame.read_u16() as usize;
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target;
                }

                Op::JumpIfFalse => {
                    let target = frame.read_u16() as usize;
                    let val = self.stack.pop().unwrap_or(Value::Nil);
                    if !val.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target;
                    }
                }

                Op::JumpIfTrue => {
                    let target = frame.read_u16() as usize;
                    let val = self.stack.pop().unwrap_or(Value::Nil);
                    if val.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target;
                    }
                }

                Op::JumpIfFalseKeep => {
                    let target = frame.read_u16() as usize;
                    let val = self.stack.last().cloned().unwrap_or(Value::Nil);
                    if !val.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target;
                    }
                }

                Op::JumpIfTrueKeep => {
                    let target = frame.read_u16() as usize;
                    let val = self.stack.last().cloned().unwrap_or(Value::Nil);
                    if val.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target;
                    }
                }

                Op::Call => {
                    let argc = frame.read_u8() as usize;
                    self.dispatch_call(argc, false)?;
                }

                Op::TailCall => {
                    let argc = frame.read_u8() as usize;
                    self.dispatch_tail_call(argc)?;
                }

                Op::MakeClosure => {
                    let proto_idx = frame.read_u16() as usize;
                    let upvalue_count = frame.read_u8() as usize;

                    // Skip upvalue descriptors for now
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip += upvalue_count * 2;

                    let proto = frame.func.constants[proto_idx].clone();
                    // The proto is already a closure Value with Bytecode body
                    self.stack.push(proto);
                }

                Op::Return => {
                    let result = self.stack.pop().unwrap_or(Value::Nil);
                    let frame = self.frames.pop().unwrap();

                    // Pop locals and any operand stack values from this frame
                    self.stack.truncate(frame.bp);

                    if self.frames.is_empty() {
                        return Ok(result);
                    }

                    self.stack.push(result);
                }

                Op::MakeList => {
                    let count = frame.read_u8() as usize;
                    let start = self.stack.len() - count;
                    let items: Vec<Value> = self.stack.drain(start..).collect();
                    self.stack.push(Value::from_slice(&items));
                }

                Op::MakeTable => {
                    let pair_count = frame.read_u8() as usize;
                    let start = self.stack.len() - pair_count * 2;
                    let items: Vec<Value> = self.stack.drain(start..).collect();
                    let mut table = MoofTable::new();
                    for i in (0..items.len()).step_by(2) {
                        let key = items[i].as_str().map_err(|_| {
                            MoofError::runtime("table key must be a string")
                        })?;
                        table.hash.insert(key.to_string(), items[i + 1].clone());
                    }
                    self.stack.push(Value::Table(Rc::new(RefCell::new(table))));
                }

                Op::MakeTableArray => {
                    let count = frame.read_u8() as usize;
                    let start = self.stack.len() - count;
                    let items: Vec<Value> = self.stack.drain(start..).collect();
                    let mut table = MoofTable::new();
                    table.array = items;
                    self.stack.push(Value::Table(Rc::new(RefCell::new(table))));
                }

                Op::StringInterp => {
                    let count = frame.read_u8() as usize;
                    let start = self.stack.len() - count;
                    let segments: Vec<Value> = self.stack.drain(start..).collect();
                    let mut result = String::new();
                    for seg in segments {
                        result.push_str(&format!("{seg}"));
                    }
                    self.stack.push(Value::Str(Rc::from(result.as_str())));
                }

                Op::Eval => {
                    let ast = self.stack.pop().unwrap_or(Value::Nil);
                    let env = self.interp.global_env.clone();
                    let result = self.interp.eval(&ast, &env)?;
                    self.stack.push(result);
                }
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Dispatch helpers
    // ═══════════════════════════════════════════════════════════════════

    /// Dispatch a function call. Stack: [callee arg1 ... argN]
    fn dispatch_call(&mut self, argc: usize, _tail: bool) -> Result<()> {
        let callee_idx = self.stack.len() - 1 - argc;
        let callee = self.stack[callee_idx].clone();

        match &callee {
            Value::Closure(c) => match &c.body {
                ClosureBody::Bytecode(func) => {
                    // Fast path: rearrange stack in-place.
                    // Stack: [... callee arg0 arg1 ... argN]
                    // Need:  [... arg0 arg1 ... argN nil nil ...] (local_count slots)
                    let bp = callee_idx; // callee slot becomes local[0]
                    // Shift args down over callee slot
                    for i in 0..argc {
                        self.stack[callee_idx + i] = self.stack[callee_idx + 1 + i].clone();
                    }
                    // Set remaining locals to nil and truncate/extend
                    let needed = func.local_count as usize;
                    let current = argc;
                    if needed > current {
                        // Pad with nil
                        self.stack.truncate(bp + current);
                        for _ in current..needed {
                            self.stack.push(Value::Nil);
                        }
                    } else {
                        self.stack.truncate(bp + needed);
                    }
                    self.frames.push(CallFrame {
                        func: func.clone(),
                        ip: 0,
                        bp,
                    });
                    Ok(())
                }
                ClosureBody::Native(f) => {
                    let args: Vec<Value> = self.stack[callee_idx + 1..].to_vec();
                    self.stack.truncate(callee_idx);
                    let result = f(&mut self.interp, args)?;
                    self.stack.push(result);
                    Ok(())
                }
                ClosureBody::Expr(_) => {
                    let args: Vec<Value> = self.stack[callee_idx + 1..].to_vec();
                    self.stack.truncate(callee_idx);
                    let result = call_closure(&mut self.interp, c, args)?;
                    self.stack.push(result);
                    Ok(())
                }
            },
            _ => {
                let args: Vec<Value> = self.stack[callee_idx + 1..].to_vec();
                self.stack.truncate(callee_idx);
                let result = self.interp.invoke(callee, args)?;
                self.stack.push(result);
                Ok(())
            }
        }
    }

    /// Dispatch a tail call — reuse current frame for bytecode-to-bytecode calls.
    fn dispatch_tail_call(&mut self, argc: usize) -> Result<()> {
        let callee_idx = self.stack.len() - 1 - argc;
        let callee = self.stack[callee_idx].clone();

        // Only optimize bytecode-to-bytecode tail calls
        if let Value::Closure(ref c) = callee {
            if let ClosureBody::Bytecode(ref func) = c.body {
                let args: Vec<Value> = self.stack[callee_idx + 1..].to_vec();
                self.stack.truncate(callee_idx);

                let frame = self.frames.last_mut().unwrap();
                let bp = frame.bp;

                // Overwrite locals with new args, pad with nil
                for i in 0..func.local_count as usize {
                    let val = args.get(i).cloned().unwrap_or(Value::Nil);
                    if bp + i < self.stack.len() {
                        self.stack[bp + i] = val;
                    } else {
                        self.stack.push(val);
                    }
                }
                // Truncate operand stack above locals
                self.stack.truncate(bp + func.local_count as usize);
                frame.func = func.clone();
                frame.ip = 0;
                return Ok(());
            }
        }

        // Non-bytecode: fall back to regular call
        self.dispatch_call(argc, false)
    }

    /// Dispatch a message send with inline caching.
    fn dispatch_send_cached(&mut self, selector: u32, argc: usize, cache_key: usize) -> Result<()> {
        let receiver_idx = self.stack.len() - 1 - argc;
        let receiver = self.stack[receiver_idx].clone();

        let class = self.interp.class_of(&receiver);
        let class_ptr = Rc::as_ptr(&class) as usize;
        let cache_slot = cache_key & 0x3FF; // mod 1024

        // Check inline cache
        let cached_method = if let Some(ref entry) = self.send_cache[cache_slot] {
            if entry.class_ptr == class_ptr {
                Some(entry.method.clone())
            } else {
                None
            }
        } else {
            None
        };

        let method = if let Some(m) = cached_method {
            m
        } else {
            // Cache miss: full lookup
            let m = class.borrow().lookup(selector);
            if let Some(ref method_val) = m {
                // Update cache
                self.send_cache[cache_slot] = Some(InlineCache {
                    class_ptr,
                    method: method_val.clone(),
                });
            }
            match m {
                Some(method_val) => method_val,
                None => {
                    // Fall back to full send_message (handles field access, doesNotUnderstand:)
                    let args: Vec<Value> = self.stack[receiver_idx + 1..].to_vec();
                    self.stack.truncate(receiver_idx);
                    let result = self.interp.send_message(receiver, selector, args)?;
                    self.stack.push(result);
                    return Ok(());
                }
            }
        };

        // Fast path: call the method directly
        let args: Vec<Value> = self.stack[receiver_idx + 1..].to_vec();
        self.stack.truncate(receiver_idx);

        let mut full_args = Vec::with_capacity(args.len() + 1);
        full_args.push(receiver);
        full_args.extend(args);

        let result = match method {
            Value::Closure(ref c) => call_closure(&mut self.interp, c, full_args)?,
            _ => return Err(MoofError::runtime("method is not callable")),
        };
        self.stack.push(result);
        Ok(())
    }

    /// Dispatch a message send without caching (for TailSend, etc.)
    fn dispatch_send(&mut self, selector: u32, argc: usize) -> Result<()> {
        let receiver_idx = self.stack.len() - 1 - argc;
        let receiver = self.stack[receiver_idx].clone();
        let args: Vec<Value> = self.stack[receiver_idx + 1..].to_vec();
        self.stack.truncate(receiver_idx);

        let result = self.interp.send_message(receiver, selector, args)?;
        self.stack.push(result);
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Standalone bytecode execution (used by tree-walker interop)
// ═══════════════════════════════════════════════════════════════════════

/// Execute a single bytecode function with the given interpreter.
/// Used when the tree-walker encounters a bytecode closure (e.g., a compiled
/// block passed to a native method like map:).
pub fn execute_bytecode(
    interp: &mut Interpreter,
    func: &Rc<CompiledFunction>,
    stack: &mut Vec<Value>,
    bp: usize,
) -> Result<Value> {
    let mut ip: usize = 0;
    let mut current_func = func.clone();
    let mut current_bp = bp;

    loop {
        let code = &current_func.code;
        let op_byte = code[ip];
        ip += 1;

        let op = Op::from_u8(op_byte).ok_or_else(|| {
            MoofError::runtime(format!("VM: unknown opcode 0x{:02x}", op_byte))
        })?;

        match op {
            Op::Nop => {}
            Op::Pop => { stack.pop(); }
            Op::Dup => {
                let v = stack.last().cloned().unwrap_or(Value::Nil);
                stack.push(v);
            }
            Op::LoadConst => {
                let idx = bytecode::read_u16(code, ip) as usize;
                ip += 2;
                stack.push(current_func.constants[idx].clone());
            }
            Op::LoadNil => stack.push(Value::Nil),
            Op::LoadTrue => stack.push(Value::Bool(true)),
            Op::LoadFalse => stack.push(Value::Bool(false)),
            Op::GetLocal => {
                let slot = code[ip] as usize; ip += 1;
                stack.push(stack[current_bp + slot].clone());
            }
            Op::SetLocal => {
                let slot = code[ip] as usize; ip += 1;
                let val = stack.last().cloned().unwrap_or(Value::Nil);
                stack[current_bp + slot] = val;
            }
            Op::GetGlobal => {
                let sym = bytecode::read_u16(code, ip) as u32; ip += 2;
                let val = interp.global_env.get(sym).map_err(|_| {
                    MoofError::name(format!("Undefined variable: {}", interp.symbols.name(sym)))
                })?;
                stack.push(val);
            }
            Op::SetGlobal => {
                let sym = bytecode::read_u16(code, ip) as u32; ip += 2;
                let val = stack.last().cloned().unwrap_or(Value::Nil);
                interp.global_env.define(sym, val, true);
            }
            Op::GetUpvalue | Op::SetUpvalue => {
                ip += 1; // skip index
                stack.push(Value::Nil);
            }
            Op::Send | Op::TailSend => {
                let sel = bytecode::read_u16(code, ip) as u32; ip += 2;
                let argc = code[ip] as usize; ip += 1;
                let recv_idx = stack.len() - 1 - argc;
                let receiver = stack[recv_idx].clone();
                let args: Vec<Value> = stack[recv_idx + 1..].to_vec();
                stack.truncate(recv_idx);
                let result = interp.send_message(receiver, sel, args)?;
                stack.push(result);
            }
            Op::SendSuper | Op::TailSendSuper => {
                ip += 3; // skip selector + argc
                return Err(MoofError::runtime("VM mini: super sends not implemented"));
            }
            Op::Jump => {
                let target = bytecode::read_u16(code, ip) as usize;
                ip = target;
            }
            Op::JumpIfFalse => {
                let target = bytecode::read_u16(code, ip) as usize; ip += 2;
                let val = stack.pop().unwrap_or(Value::Nil);
                if !val.is_truthy() { ip = target; }
            }
            Op::JumpIfTrue => {
                let target = bytecode::read_u16(code, ip) as usize; ip += 2;
                let val = stack.pop().unwrap_or(Value::Nil);
                if val.is_truthy() { ip = target; }
            }
            Op::JumpIfFalseKeep => {
                let target = bytecode::read_u16(code, ip) as usize; ip += 2;
                if !stack.last().map(|v| v.is_truthy()).unwrap_or(false) { ip = target; }
            }
            Op::JumpIfTrueKeep => {
                let target = bytecode::read_u16(code, ip) as usize; ip += 2;
                if stack.last().map(|v| v.is_truthy()).unwrap_or(false) { ip = target; }
            }
            Op::Call => {
                let argc = code[ip] as usize; ip += 1;
                let callee_idx = stack.len() - 1 - argc;
                let callee = stack[callee_idx].clone();
                let args: Vec<Value> = stack[callee_idx + 1..].to_vec();
                stack.truncate(callee_idx);
                let result = interp.invoke(callee, args)?;
                stack.push(result);
            }
            Op::TailCall => {
                let argc = code[ip] as usize; ip += 1;
                let callee_idx = stack.len() - 1 - argc;
                let callee = stack[callee_idx].clone();
                let args: Vec<Value> = stack[callee_idx + 1..].to_vec();
                stack.truncate(callee_idx);

                // TCO: if callee is bytecode, swap function and restart
                if let Value::Closure(ref c) = callee {
                    if let ClosureBody::Bytecode(ref new_func) = c.body {
                        for i in 0..new_func.local_count as usize {
                            let val = args.get(i).cloned().unwrap_or(Value::Nil);
                            if current_bp + i < stack.len() {
                                stack[current_bp + i] = val;
                            } else {
                                stack.push(val);
                            }
                        }
                        stack.truncate(current_bp + new_func.local_count as usize);
                        current_func = new_func.clone();
                        ip = 0;
                        continue;
                    }
                }
                // Non-bytecode: regular call
                let result = interp.invoke(callee, args)?;
                stack.push(result);
            }
            Op::MakeClosure => {
                let proto_idx = bytecode::read_u16(code, ip) as usize; ip += 2;
                let upvalue_count = code[ip] as usize; ip += 1;
                ip += upvalue_count * 2; // skip upvalue descriptors
                stack.push(func.constants[proto_idx].clone());
            }
            Op::Return => {
                let result = stack.pop().unwrap_or(Value::Nil);
                return Ok(result);
            }
            Op::MakeList => {
                let count = code[ip] as usize; ip += 1;
                let start = stack.len() - count;
                let items: Vec<Value> = stack.drain(start..).collect();
                stack.push(Value::from_slice(&items));
            }
            Op::MakeTable => {
                let pair_count = code[ip] as usize; ip += 1;
                let start = stack.len() - pair_count * 2;
                let items: Vec<Value> = stack.drain(start..).collect();
                let mut table = MoofTable::new();
                for i in (0..items.len()).step_by(2) {
                    if let Ok(key) = items[i].as_str() {
                        table.hash.insert(key.to_string(), items[i + 1].clone());
                    }
                }
                stack.push(Value::Table(Rc::new(RefCell::new(table))));
            }
            Op::MakeTableArray => {
                let count = code[ip] as usize; ip += 1;
                let start = stack.len() - count;
                let items: Vec<Value> = stack.drain(start..).collect();
                let mut table = MoofTable::new();
                table.array = items;
                stack.push(Value::Table(Rc::new(RefCell::new(table))));
            }
            Op::StringInterp => {
                let count = code[ip] as usize; ip += 1;
                let start = stack.len() - count;
                let segs: Vec<Value> = stack.drain(start..).collect();
                let mut result = String::new();
                for seg in segs { result.push_str(&format!("{seg}")); }
                stack.push(Value::Str(Rc::from(result.as_str())));
            }
            Op::Eval => {
                let ast = stack.pop().unwrap_or(Value::Nil);
                let env = interp.global_env.clone();
                let result = interp.eval(&ast, &env)?;
                stack.push(result);
            }
        }
    }
}
