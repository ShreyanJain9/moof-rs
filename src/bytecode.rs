//! Bytecode definitions for the Moof VM.
//!
//! The instruction set is designed around Smalltalk-style message dispatch
//! as the primary operation. Send opcodes outnumber all other categories.

use std::cell::UnsafeCell;
use std::fmt;
use std::rc::Rc;

use crate::symbol::SymId;
use crate::value::Value;

// ═══════════════════════════════════════════════════════════════════════
// Opcodes
// ═══════════════════════════════════════════════════════════════════════

/// Bytecode opcodes. Each is a single u8 followed by operands of known widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Op {
    // ── Stack manipulation ──────────────────────────────────────────
    Nop           = 0x00,
    Pop           = 0x01,  // discard TOS
    Dup           = 0x02,  // duplicate TOS

    // ── Constants / Literals ────────────────────────────────────────
    LoadConst     = 0x10,  // u16: constant pool index
    LoadNil       = 0x11,
    LoadTrue      = 0x12,
    LoadFalse     = 0x13,

    // ── Variable access ────────────────────────────────────────────
    GetLocal      = 0x20,  // u8: slot index
    SetLocal      = 0x21,  // u8: slot index
    GetUpvalue    = 0x22,  // u8: upvalue index
    SetUpvalue    = 0x23,  // u8: upvalue index
    GetGlobal     = 0x24,  // u16: symbol id
    SetGlobal     = 0x25,  // u16: symbol id

    // ── Message sends ──────────────────────────────────────────────
    Send          = 0x30,  // u16: selector symid, u8: argc
    SendSuper     = 0x31,  // u16: selector symid, u8: argc
    TailSend      = 0x33,  // u16: selector symid, u8: argc
    TailSendSuper = 0x34,  // u16: selector symid, u8: argc

    // ── Control flow ───────────────────────────────────────────────
    Jump          = 0x40,  // u16: absolute target
    JumpIfFalse   = 0x41,  // u16: absolute target; pops TOS
    JumpIfTrue    = 0x42,  // u16: absolute target; pops TOS
    JumpIfFalseKeep = 0x43, // u16: absolute target; does NOT pop (for `and`)
    JumpIfTrueKeep  = 0x44, // u16: absolute target; does NOT pop (for `or`)

    // ── Function calls / closures ──────────────────────────────────
    Call          = 0x50,  // u8: argc. Stack: [callee arg1 ... argN] → [result]
    TailCall      = 0x51,  // u8: argc. Like Call but reuses frame.
    MakeClosure   = 0x52,  // u16: proto index in constants, u8: upvalue count
                           // followed by upvalue_count × (u8 is_local, u8 index)
    Return        = 0x53,

    // ── Object / list / table ──────────────────────────────────────
    MakeList      = 0x60,  // u8: count. Pops count values → pushes cons list
    MakeTable     = 0x61,  // u8: pair_count. Pops 2*N values (k,v) → pushes Table
    MakeTableArray = 0x62, // u8: count. Pops N values → pushes array Table

    // ── Special ────────────────────────────────────────────────────
    StringInterp  = 0x70,  // u8: segment count. Pops N values → concat → push Str
    Eval          = 0x71,  // pop TOS (an AST Value), eval via tree-walker, push result
}

impl Op {
    pub fn from_u8(byte: u8) -> Option<Op> {
        // Safety: we validate the byte value
        match byte {
            0x00 => Some(Op::Nop),
            0x01 => Some(Op::Pop),
            0x02 => Some(Op::Dup),
            0x10 => Some(Op::LoadConst),
            0x11 => Some(Op::LoadNil),
            0x12 => Some(Op::LoadTrue),
            0x13 => Some(Op::LoadFalse),
            0x20 => Some(Op::GetLocal),
            0x21 => Some(Op::SetLocal),
            0x22 => Some(Op::GetUpvalue),
            0x23 => Some(Op::SetUpvalue),
            0x24 => Some(Op::GetGlobal),
            0x25 => Some(Op::SetGlobal),
            0x30 => Some(Op::Send),
            0x31 => Some(Op::SendSuper),
            0x33 => Some(Op::TailSend),
            0x34 => Some(Op::TailSendSuper),
            0x40 => Some(Op::Jump),
            0x41 => Some(Op::JumpIfFalse),
            0x42 => Some(Op::JumpIfTrue),
            0x43 => Some(Op::JumpIfFalseKeep),
            0x44 => Some(Op::JumpIfTrueKeep),
            0x50 => Some(Op::Call),
            0x51 => Some(Op::TailCall),
            0x52 => Some(Op::MakeClosure),
            0x53 => Some(Op::Return),
            0x60 => Some(Op::MakeList),
            0x61 => Some(Op::MakeTable),
            0x62 => Some(Op::MakeTableArray),
            0x70 => Some(Op::StringInterp),
            0x71 => Some(Op::Eval),
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Compiled function
// ═══════════════════════════════════════════════════════════════════════

/// A compiled function, method, or block.
#[derive(Clone)]
pub struct CompiledFunction {
    /// Name for debugging (None for anonymous lambdas/blocks)
    pub name: Option<SymId>,

    /// Number of positional parameters
    pub arity: u8,

    /// Whether this function has a rest parameter
    pub has_rest: bool,

    /// Number of local variable slots (params + let-bindings + temporaries)
    pub local_count: u8,

    /// Bytecode instructions
    pub code: Vec<u8>,

    /// Constant pool: Values referenced by LoadConst.
    /// Nested CompiledFunctions are stored here as closures.
    pub constants: Vec<Value>,

    /// Upvalue descriptors: how to capture each upvalue at closure creation
    pub upvalues: Vec<UpvalueDesc>,

    /// Is this a block (supports non-local return)?
    pub is_block: bool,

    /// For super sends: the class this method was compiled for
    pub enclosing_class: Option<SymId>,
}

impl fmt::Debug for CompiledFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CompiledFunction({:?}, arity={}, locals={}, code_len={})",
            self.name, self.arity, self.local_count, self.code.len()
        )
    }
}

/// Describes how to capture an upvalue when creating a closure at runtime.
#[derive(Debug, Clone, Copy)]
pub struct UpvalueDesc {
    /// If true, capture from enclosing function's locals[index].
    /// If false, capture from enclosing function's upvalues[index].
    pub is_local: bool,
    pub index: u8,
}

// ═══════════════════════════════════════════════════════════════════════
// Bytecode builder
// ═══════════════════════════════════════════════════════════════════════

/// Builder for constructing bytecode sequences.
pub struct BytecodeBuilder {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
}

impl BytecodeBuilder {
    pub fn new() -> Self {
        BytecodeBuilder {
            code: Vec::new(),
            constants: Vec::new(),
        }
    }

    /// Current offset in the code buffer.
    pub fn offset(&self) -> usize {
        self.code.len()
    }

    /// Emit a single opcode with no operands.
    pub fn emit_op(&mut self, op: Op) {
        self.code.push(op as u8);
    }

    /// Emit an opcode followed by a u8 operand.
    pub fn emit_op_u8(&mut self, op: Op, operand: u8) {
        self.code.push(op as u8);
        self.code.push(operand);
    }

    /// Emit an opcode followed by a u16 operand (big-endian).
    pub fn emit_op_u16(&mut self, op: Op, operand: u16) {
        self.code.push(op as u8);
        self.code.push((operand >> 8) as u8);
        self.code.push((operand & 0xFF) as u8);
    }

    /// Emit an opcode followed by u16 then u8 (for Send: selector, argc).
    pub fn emit_op_u16_u8(&mut self, op: Op, a: u16, b: u8) {
        self.code.push(op as u8);
        self.code.push((a >> 8) as u8);
        self.code.push((a & 0xFF) as u8);
        self.code.push(b);
    }

    /// Emit a jump instruction with a placeholder target. Returns the offset
    /// of the u16 target bytes so it can be patched later.
    pub fn emit_jump(&mut self, op: Op) -> usize {
        self.code.push(op as u8);
        let patch_offset = self.code.len();
        self.code.push(0x00); // placeholder hi
        self.code.push(0x00); // placeholder lo
        patch_offset
    }

    /// Patch a previously-emitted jump to point to the current offset.
    pub fn patch_jump(&mut self, patch_offset: usize) {
        let target = self.code.len() as u16;
        self.code[patch_offset] = (target >> 8) as u8;
        self.code[patch_offset + 1] = (target & 0xFF) as u8;
    }

    /// Add a constant to the pool, returning its index.
    pub fn add_constant(&mut self, val: Value) -> u16 {
        let idx = self.constants.len();
        self.constants.push(val);
        idx as u16
    }

    /// Emit LoadConst for a value, adding it to the constant pool.
    pub fn emit_constant(&mut self, val: Value) {
        let idx = self.add_constant(val);
        self.emit_op_u16(Op::LoadConst, idx);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Bytecode reader helpers (used by VM)
// ═══════════════════════════════════════════════════════════════════════

/// Read a u8 from code at the given offset.
#[inline]
pub fn read_u8(code: &[u8], ip: usize) -> u8 {
    code[ip]
}

/// Read a u16 (big-endian) from code at the given offset.
#[inline]
pub fn read_u16(code: &[u8], ip: usize) -> u16 {
    ((code[ip] as u16) << 8) | (code[ip + 1] as u16)
}
