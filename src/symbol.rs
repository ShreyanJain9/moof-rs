use std::collections::HashMap;

pub type SymId = u32;

pub struct SymbolTable {
    names: Vec<String>,
    lookup: HashMap<String, SymId>,
}

impl SymbolTable {
    pub fn new() -> Self {
        SymbolTable {
            names: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    pub fn intern(&mut self, name: &str) -> SymId {
        if let Some(&id) = self.lookup.get(name) {
            return id;
        }
        let id = self.names.len() as SymId;
        self.names.push(name.to_string());
        self.lookup.insert(name.to_string(), id);
        id
    }

    pub fn name(&self, id: SymId) -> &str {
        &self.names[id as usize]
    }

    pub fn try_name(&self, id: SymId) -> Option<&str> {
        self.names.get(id as usize).map(|s| s.as_str())
    }

    /// Look up a symbol id by name without interning. Returns None if not found.
    pub fn lookup_id(&self, name: &str) -> Option<&SymId> {
        self.lookup.get(name)
    }
}

pub struct KnownSymbols {
    // Special forms
    pub if_: SymId,
    pub define: SymId,
    pub lambda: SymId,
    pub fn_: SymId,
    pub let_: SymId,
    pub do_: SymId,
    pub set_bang: SymId,
    pub quote: SymId,
    pub quasiquote: SymId,
    pub unquote: SymId,
    pub unquote_splice: SymId,
    pub match_: SymId,
    pub class: SymId,
    pub trait_: SymId,
    pub protocol: SymId,
    pub type_: SymId,
    pub try_: SymId,
    pub catch: SymId,
    pub defmacro: SymId,
    pub module: SymId,
    pub use_: SymId,
    pub require: SymId,

    // Internal desugaring
    pub send: SymId,
    pub table: SymId,
    pub table_array: SymId,
    pub str_interp: SymId,

    // Common
    pub self_: SymId,
    pub underscore: SymId,
    pub true_: SymId,
    pub false_: SymId,
    pub nil: SymId,
    pub else_: SymId,
    pub and: SymId,
    pub or: SymId,
    pub cond: SymId,
    pub extends: SymId,
    pub fields: SymId,
    pub method: SymId,
    pub uses: SymId,

    // Class system internal
    pub __name: SymId,
    pub __super: SymId,
    pub __fields: SymId,
    pub __call: SymId,
    pub __meta: SymId,
    pub super_send: SymId,
    pub super_: SymId,
    pub __current_class: SymId,
    pub classmethod: SymId,
    pub __primitive: SymId,
    pub delegates_to: SymId,
    pub signal: SymId,
    pub handler_bind: SymId,
    pub restart_case: SymId,
    pub invoke_restart: SymId,
    pub break_: SymId,
    pub reload: SymId,
}

impl KnownSymbols {
    pub fn new(syms: &mut SymbolTable) -> Self {
        KnownSymbols {
            // Special forms
            if_: syms.intern("if"),
            define: syms.intern("define"),
            lambda: syms.intern("lambda"),
            fn_: syms.intern("fn"),
            let_: syms.intern("let"),
            do_: syms.intern("do"),
            set_bang: syms.intern("set!"),
            quote: syms.intern("quote"),
            quasiquote: syms.intern("quasiquote"),
            unquote: syms.intern("unquote"),
            unquote_splice: syms.intern("unquote-splice"),
            match_: syms.intern("match"),
            class: syms.intern("class"),
            trait_: syms.intern("trait"),
            protocol: syms.intern("protocol"),
            type_: syms.intern("type"),
            try_: syms.intern("try"),
            catch: syms.intern("catch"),
            defmacro: syms.intern("defmacro"),
            module: syms.intern("module"),
            use_: syms.intern("use"),
            require: syms.intern("require"),

            // Internal desugaring
            send: syms.intern("__send"),
            table: syms.intern("__table"),
            table_array: syms.intern("__table-array"),
            str_interp: syms.intern("__str-interp"),

            // Common
            self_: syms.intern("self"),
            underscore: syms.intern("_"),
            true_: syms.intern("true"),
            false_: syms.intern("false"),
            nil: syms.intern("nil"),
            else_: syms.intern("else"),
            and: syms.intern("and"),
            or: syms.intern("or"),
            cond: syms.intern("cond"),
            extends: syms.intern("extends"),
            fields: syms.intern("fields"),
            method: syms.intern("method"),
            uses: syms.intern("uses"),

            // Class system internal
            __name: syms.intern("__name"),
            __super: syms.intern("__super"),
            __fields: syms.intern("__fields"),
            __call: syms.intern("__call"),
            __meta: syms.intern("__meta"),
            super_send: syms.intern("__super-send"),
            super_: syms.intern("super"),
            __current_class: syms.intern("__current_class"),
            classmethod: syms.intern("classmethod"),
            __primitive: syms.intern("__primitive"),
            delegates_to: syms.intern("delegates-to"),
            signal: syms.intern("signal"),
            handler_bind: syms.intern("handler-bind"),
            restart_case: syms.intern("restart-case"),
            invoke_restart: syms.intern("invoke-restart"),
            break_: syms.intern("break"),
            reload: syms.intern("reload"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_returns_same_id() {
        let mut t = SymbolTable::new();
        let a = t.intern("foo");
        let b = t.intern("foo");
        assert_eq!(a, b);
    }

    #[test]
    fn distinct_names_get_distinct_ids() {
        let mut t = SymbolTable::new();
        let a = t.intern("foo");
        let b = t.intern("bar");
        assert_ne!(a, b);
    }

    #[test]
    fn name_roundtrips() {
        let mut t = SymbolTable::new();
        let id = t.intern("hello");
        assert_eq!(t.name(id), "hello");
    }

    #[test]
    fn try_name_returns_none_for_invalid() {
        let t = SymbolTable::new();
        assert_eq!(t.try_name(999), None);
    }

    #[test]
    fn known_symbols_are_interned() {
        let mut t = SymbolTable::new();
        let ks = KnownSymbols::new(&mut t);
        assert_eq!(t.name(ks.if_), "if");
        assert_eq!(t.name(ks.set_bang), "set!");
        assert_eq!(t.name(ks.unquote_splice), "unquote-splice");
        assert_eq!(t.name(ks.send), "__send");
        assert_eq!(t.name(ks.nil), "nil");
    }
}
