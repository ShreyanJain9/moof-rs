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

pub struct VM {
    /// The tree-walking interpreter (for builtins, class system, symbol table)
    pub interp: Interpreter,
    /// Operand stack
    stack: Vec<Value>,
    /// Call frame stack
    frames: Vec<CallFrame>,
}

impl VM {
    pub fn new(interp: Interpreter) -> Self {
        VM {
            interp,
            stack: Vec::with_capacity(256),
            frames: Vec::with_capacity(64),
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
                    let selector = frame.read_u16() as u32;
                    let argc = frame.read_u8() as usize;
                    self.dispatch_send(selector, argc)?;
                }

                Op::TailSend => {
                    let selector = frame.read_u16() as u32;
                    let argc = frame.read_u8() as usize;
                    // For Phase 1: just do a normal send (no TCO yet)
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
                    // Phase 1: for now, just do a regular call
                    self.dispatch_call(argc, false)?;
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
        let args: Vec<Value> = self.stack[callee_idx + 1..].to_vec();
        self.stack.truncate(callee_idx);

        match &callee {
            Value::Closure(c) => match &c.body {
                ClosureBody::Bytecode(func) => {
                    // Push new frame for bytecode closure
                    let bp = self.stack.len();
                    // Reserve local slots, fill with args then nil
                    for i in 0..func.local_count as usize {
                        let val = args.get(i).cloned().unwrap_or(Value::Nil);
                        self.stack.push(val);
                    }
                    self.frames.push(CallFrame {
                        func: func.clone(),
                        ip: 0,
                        bp,
                    });
                    // Execution continues in the main loop
                    Ok(())
                }
                ClosureBody::Native(f) => {
                    let result = f(&mut self.interp, args)?;
                    self.stack.push(result);
                    Ok(())
                }
                ClosureBody::Expr(_) => {
                    // Fall back to tree-walker for Expr closures
                    let result = call_closure(&mut self.interp, c, args)?;
                    self.stack.push(result);
                    Ok(())
                }
            },
            _ => {
                // Try invoke for class constructors etc.
                let result = self.interp.invoke(callee, args)?;
                self.stack.push(result);
                Ok(())
            }
        }
    }

    /// Dispatch a message send. Stack: [receiver arg1 ... argN]
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
    let code = &func.code;

    loop {
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
                stack.push(func.constants[idx].clone());
            }
            Op::LoadNil => stack.push(Value::Nil),
            Op::LoadTrue => stack.push(Value::Bool(true)),
            Op::LoadFalse => stack.push(Value::Bool(false)),
            Op::GetLocal => {
                let slot = code[ip] as usize; ip += 1;
                stack.push(stack[bp + slot].clone());
            }
            Op::SetLocal => {
                let slot = code[ip] as usize; ip += 1;
                let val = stack.last().cloned().unwrap_or(Value::Nil);
                stack[bp + slot] = val;
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
            Op::Call | Op::TailCall => {
                let argc = code[ip] as usize; ip += 1;
                let callee_idx = stack.len() - 1 - argc;
                let callee = stack[callee_idx].clone();
                let args: Vec<Value> = stack[callee_idx + 1..].to_vec();
                stack.truncate(callee_idx);
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
