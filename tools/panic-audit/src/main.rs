//! Syntax inventory complements the concrete final-link fixture. A listed site
//! is a proof obligation, not evidence that panic is reachable.
use quote::ToTokens;
use serde_json::{Value, json};
use syn::{
    parse::Parser,
    spanned::Spanned,
    visit::{self, Visit},
};

fn pure_invariant(expr: &syn::Expr) -> bool {
    pure_expression(expr, false)
}
fn pure_expression(expr: &syn::Expr, metadata: bool) -> bool {
    match expr {
        syn::Expr::Path(_) | syn::Expr::Lit(_) => true,
        syn::Expr::Field(e) => pure_expression(&e.base, metadata),
        syn::Expr::Paren(e) => pure_expression(&e.expr, metadata),
        syn::Expr::Binary(e) => {
            use syn::BinOp::*;
            matches!(
                e.op,
                Add(_)
                    | Sub(_)
                    | Mul(_)
                    | And(_)
                    | Or(_)
                    | Eq(_)
                    | Ne(_)
                    | Lt(_)
                    | Le(_)
                    | Gt(_)
                    | Ge(_)
            ) && pure_expression(&e.left, metadata)
                && pure_expression(&e.right, metadata)
        }
        syn::Expr::Call(e) => {
            matches!(&*e.func, syn::Expr::Path(p) if p.path.is_ident("on_entry"))
                && e.args.len() == 1
                && pure_expression(&e.args[0], metadata)
        }
        syn::Expr::Unary(e) if metadata => {
            matches!(e.op, syn::UnOp::Not(_)) && pure_expression(&e.expr, true)
        }
        syn::Expr::MethodCall(e) if metadata => {
            let receiver = e.receiver.to_token_stream().to_string().replace(' ', "");
            let method = e.method.to_string();
            let scalar_get = method == "get" && e.args.is_empty() && receiver == "prefix.len";
            let predicate = method == "valid_for"
                && ["state", "g.state"].contains(&receiver.as_str())
                && e.args.len() == 1
                && pure_expression(&e.args[0], false);
            let query = e.args.is_empty()
                && ["state", "g.state", "self.state"].contains(&receiver.as_str())
                && [
                    "read",
                    "written",
                    "end",
                    "remaining",
                    "untouched",
                    "valid",
                    "total",
                ]
                .contains(&method.as_str());
            scalar_get || predicate || query
        }
        _ => false,
    }
}

// Only immutable, single-expression scalar getters may appear in proof
// predicates. Validate the implementations before accepting their call syntax.
fn scalar_accessor(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(e) => {
            matches!(&*e.func, syn::Expr::Path(p) if ["wrapping_add", "wrapping_sub", "wrapping_mul", "crate::arithmetic::bounded::wrapping_add", "crate::arithmetic::bounded::wrapping_sub", "crate::arithmetic::bounded::wrapping_mul"].contains(&p.path.to_token_stream().to_string().replace(' ', "").as_str()))
                && e.args.len() == 2
                && e.args.iter().all(scalar_accessor)
        }
        syn::Expr::Path(_) | syn::Expr::Lit(_) => true,
        syn::Expr::Field(e) => scalar_accessor(&e.base),
        syn::Expr::Paren(e) => scalar_accessor(&e.expr),
        syn::Expr::Binary(e) => {
            use syn::BinOp::*;
            matches!(
                e.op,
                Add(_)
                    | Sub(_)
                    | Mul(_)
                    | And(_)
                    | Or(_)
                    | Eq(_)
                    | Ne(_)
                    | Lt(_)
                    | Le(_)
                    | Gt(_)
                    | Ge(_)
            ) && scalar_accessor(&e.left)
                && scalar_accessor(&e.right)
        }
        syn::Expr::Unary(e) => matches!(e.op, syn::UnOp::Not(_)) && scalar_accessor(&e.expr),
        syn::Expr::MethodCall(e) => {
            e.args.is_empty()
                && matches!(&*e.receiver, syn::Expr::Field(_))
                && scalar_accessor(&e.receiver)
                && (e.method == "get"
                    || (["read", "written", "remaining"].contains(&e.method.to_string().as_str())
                        && e.receiver.to_token_stream().to_string().replace(' ', "")
                            == "self.scan"))
        }
        _ => false,
    }
}
fn verus_proof_only(node: &syn::StmtMacro) -> bool {
    node.mac.path.is_ident("proof")
        && node.attrs.len() == 1
        && node.attrs[0].path().is_ident("cfg")
        && matches!(&node.attrs[0].meta, syn::Meta::List(cfg) if cfg.tokens.to_string() == "verus_keep_ghost")
}
// This observer has a checked subtraction and an early return, outside the
// scalar accessor grammar. Review its complete definition rather than allowing
// those operations in arbitrary predicates.
fn reviewed_extraction_observer(method: &syn::ImplItemFn) -> bool {
    let expected = quote::quote! {
        #[cfg(kani)]
        pub(crate) fn valid_progress(
            &self,
            end: usize,
            total: usize,
            minimum_read: usize,
            minimum_write: usize,
        ) -> bool {
            let Some(write) = self.read.get().checked_sub(self.deleted.get()) else {
                return false;
            };
            self.valid()
                && self.end.get() == end
                && self.total.get() == total
                && minimum_read <= self.read.get()
                && minimum_write <= write
        }
    };
    method.to_token_stream().to_string() == expected.to_string()
}

// A loop predicate that names a field of the owner makes the verifier's
// inferred loop write set cover the words next to that field
// (docs/premise-proofs.md, unit 1 steps 7 and 8). These predicates read the
// owner whole through the reviewed getters; each is accepted only as this
// exact text at this loop. `kani::index` is the verifier's counter of a `for`
// loop and `on_entry` its value before the loop.
const OWNER_INVARIANTS: [(&str, &str, &str); 5] = [
    (
        "src/vec.rs",
        "try_resize_with_init",
        "original<=guard.owner.len()&&guard.owner.len()<=new_len",
    ),
    (
        "src/vec.rs",
        "try_extend_fallible",
        "original<=guard.owner.len()&&guard.owner.len()<=guard.owner.capacity()&&guard.owner.capacity()<=guard.owner.max_len()&&guard.owner.max_len()==on_entry(guard.owner.max_len())&&!guard.owner.as_ptr().is_null()",
    ),
    (
        "src/vec/convenience.rs",
        "try_resize_owned",
        "original<=guard.owner.len()&&guard.owner.len()<=last",
    ),
    (
        "src/vec/convenience.rs",
        "try_extend_from_within",
        "original<=guard.owner.len()&&guard.owner.len()-original==kani::index",
    ),
    (
        "src/deque/edit.rs",
        "try_retain_mut",
        "guard.state.valid_for(guard.owner.len())&&guard.owner.len()<=guard.owner.capacity()&&guard.owner.capacity()<=guard.owner.max_len()&&guard.owner.max_len()==on_entry(guard.owner.max_len())&&!guard.owner.probe_ptr().is_null()",
    ),
];
// The getters those predicates call, as their whole reviewed definitions:
// (owner type, attributes and visibility, name, output, body).
// `VecDeque::max_len` has no `#[inline]`; `VecDeque::probe_ptr` exists only
// under `cfg(kani)` (owner decision, 2026-11-28). Each text is compared
// exactly, never as a pattern.
const OWNER_GETTERS: [(&str, &str, &str, &str, &str); 8] = [
    (
        "Vec<T,P,L>",
        "#[inline] pub",
        "len",
        "usize",
        "self.len.get()",
    ),
    (
        "Vec<T,P,L>",
        "#[inline] pub",
        "capacity",
        "usize",
        "self.cap.get()",
    ),
    (
        "Vec<T,P,L>",
        "#[inline] pub",
        "max_len",
        "usize",
        "self.limit.max_len()",
    ),
    (
        "Vec<T,P,L>",
        "#[inline] pub",
        "as_ptr",
        "*const T",
        "self.ptr.as_ptr()",
    ),
    (
        "VecDeque<T,P,L>",
        "#[inline] pub",
        "len",
        "usize",
        "self.len.get()",
    ),
    (
        "VecDeque<T,P,L>",
        "#[inline] pub",
        "capacity",
        "usize",
        "self.cap.get()",
    ),
    (
        "VecDeque<T,P,L>",
        "pub",
        "max_len",
        "usize",
        "self.limit.max_len()",
    ),
    (
        "VecDeque<T,P,L>",
        "#[cfg(kani)]",
        "probe_ptr",
        "*const T",
        "self.ptr.as_ptr()",
    ),
];
fn reviewed_owner_getter(owner: &str, method: &syn::ImplItemFn) -> bool {
    OWNER_GETTERS
        .iter()
        .filter(|row| row.0 == owner)
        .any(|(_, attributes, name, output, body)| {
            let expected: syn::ImplItemFn = syn::parse_str(&format!(
                "{attributes} fn {name}(&self) -> {output} {{ {body} }}"
            ))
            .expect("reviewed getter text");
            method.to_token_stream().to_string() == expected.to_token_stream().to_string()
        })
}
/// The number of reviewed getters of `owner` (for example `Vec<T,P,L>`) in
/// `text`; a getter of that name with any other definition fails.
fn owner_getters(text: &str, owner: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let mut reviewed = 0;
    for item in syn::parse_file(text)?.items {
        if let syn::Item::Impl(block) = item {
            let matches = block.trait_.is_none()
                && block.self_ty.to_token_stream().to_string().replace(' ', "") == owner;
            for item in block.items {
                if let syn::ImplItem::Fn(method) = item {
                    let name = method.sig.ident.to_string();
                    if matches
                        && OWNER_GETTERS
                            .iter()
                            .any(|row| row.0 == owner && row.2 == name)
                    {
                        assert!(
                            reviewed_owner_getter(owner, &method),
                            "{owner}::{name} requires review"
                        );
                        reviewed += 1;
                    }
                }
            }
        }
    }
    Ok(reviewed)
}

fn validate_metadata_getters() -> Result<bool, Box<dyn std::error::Error>> {
    // Arithmetic in predicates is read-only. Its verifier-only precondition
    // assertions and exact product operation are checked before authorizing it.
    let bounded = syn::parse_file(&std::fs::read_to_string("src/arithmetic/bounded.rs")?)?;
    let mut operations = 0;
    for item in bounded.items {
        if let syn::Item::Fn(function) = item {
            let name = function.sig.ident.to_string();
            assert!(["wrapping_add", "wrapping_sub", "wrapping_mul"].contains(&name.as_str()));
            // Direct Verus annotations add an erased proof macro. Only this
            // exact cfg removes it; executable/Kani statements remain checked.
            let statements: Vec<_> = function
                .block
                .stmts
                .iter()
                .filter(|stmt| !matches!(stmt, syn::Stmt::Macro(m) if verus_proof_only(m)))
                .collect();
            assert_eq!(statements.len(), 2);
            assert!(
                matches!(statements[0], syn::Stmt::Macro(m) if kani_only(&m.attrs) && m.mac.path.is_ident("assert"))
            );
            assert!(
                matches!(statements[1], syn::Stmt::Expr(syn::Expr::MethodCall(m), None) if m.method == name.as_str() && m.receiver.to_token_stream().to_string() == "left" && m.args.to_token_stream().to_string() == "right")
            );
            if let syn::Stmt::Macro(check) = statements[0] {
                let condition = check.mac.tokens.to_string().replace(' ', "");
                let primitive = name.strip_prefix("wrapping_").expect("bounded operator");
                assert!(
                    condition.starts_with(&format!("left.checked_{primitive}(right).is_some(),")),
                    "missing overflow proof assertion"
                );
            }
            operations += 1;
        }
    }
    assert_eq!(operations, 3);

    let paths = [
        "src/arithmetic/checked.rs",
        "src/compaction.rs",
        "src/capacity.rs",
    ];
    if !paths.iter().all(|p| std::path::Path::new(p).exists())
        || !std::path::Path::new("src/vec.rs").exists()
        || !std::path::Path::new("src/deque.rs").exists()
    {
        return Ok(false);
    }
    for (file, owner) in [
        ("src/vec.rs", "Vec<T,P,L>"),
        ("src/deque.rs", "VecDeque<T,P,L>"),
    ] {
        assert_eq!(
            owner_getters(&std::fs::read_to_string(file)?, owner)?,
            OWNER_GETTERS.iter().filter(|row| row.0 == owner).count(),
            "expected each reviewed {owner} getter once"
        );
    }
    let mut checked = 0;
    let mut extraction_observers = 0;
    for path in paths {
        let text = std::fs::read_to_string(path)?;
        if path == "src/compaction.rs" {
            assert!(
                text.replace([' ', '\n', '\r', '\t'], "")
                    .contains("usecrate::arithmetic::bounded::{wrapping_add,wrapping_sub};")
            );
        }
        let syntax = syn::parse_file(&text)?;
        for item in syntax.items {
            if let syn::Item::Impl(block) = item {
                let extraction = path == "src/compaction.rs"
                    && block.trait_.is_none()
                    && matches!(&*block.self_ty, syn::Type::Path(t)
                        if t.qself.is_none() && t.path.is_ident("Extraction"));
                for item in block.items {
                    if let syn::ImplItem::Fn(method) = item {
                        if extraction && method.sig.ident == "valid_progress" {
                            assert!(
                                reviewed_extraction_observer(&method),
                                "Extraction::valid_progress requires review"
                            );
                            extraction_observers += 1;
                        }
                        if [
                            "get",
                            "read",
                            "written",
                            "end",
                            "remaining",
                            "untouched",
                            "valid",
                            "valid_for",
                            "total",
                        ]
                        .contains(&method.sig.ident.to_string().as_str())
                        {
                            assert!(
                                method
                                    .sig
                                    .receiver()
                                    .is_some_and(|r| r.mutability.is_none()),
                                "mutable metadata predicate receiver"
                            );
                            assert!(
                                matches!(method.block.stmts.as_slice(),[syn::Stmt::Expr(expr,None)] if scalar_accessor(expr)),
                                "metadata predicate getter has effects or unknown calls: {path}::{}",
                                method.sig.ident
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 10, "metadata getter definitions missing");
    assert_eq!(
        extraction_observers, 1,
        "expected one reviewed Extraction observer"
    );
    Ok(true)
}
fn kani_only(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|a| {
        a.path().is_ident("cfg")
            && matches!(&a.meta,syn::Meta::List(l) if l.tokens.to_string()=="kani")
    })
}

struct Inventory<'a> {
    file: &'a str,
    context: String,
    function: String,
    rows: Vec<Value>,
    metadata_getters: bool,
}

impl Inventory<'_> {
    fn record(&mut self, kind: &str, node: &impl ToTokens, span: proc_macro2::Span) {
        let start = span.start();
        let end = span.end();
        self.rows.push(json!({
            "file": self.file, "line": start.line, "column": start.column + 1,
            "end_line": end.line, "end_column": end.column + 1,
            "context": self.context, "function": self.function,
            "kind": kind, "expression": node.to_token_stream().to_string(),
        }));
    }

    fn record_invocation(&mut self, mac: &syn::Macro, span: proc_macro2::Span) {
        let panic = mac.path.segments.last().is_some_and(|p| {
            [
                "panic",
                "assert",
                "assert_eq",
                "assert_ne",
                "debug_assert",
                "debug_assert_eq",
                "debug_assert_ne",
                "unreachable",
                "todo",
                "unimplemented",
            ]
            .contains(&p.ident.to_string().as_str())
        });
        self.record(
            if panic {
                "panic_macro"
            } else {
                "macro_invocation"
            },
            mac,
            span,
        );
    }
}

impl<'ast> Visit<'ast> for Inventory<'_> {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if !kani_only(&node.attrs) {
            visit::visit_item_mod(self, node);
        }
    }

    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        if node.path().is_ident("cfg_attr") {
            if let syn::Meta::List(cfg) = &node.meta {
                let metas =
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
                        .parse2(cfg.tokens.clone())
                        .expect("valid cfg_attr syntax");
                let extraction_cfg = metas.len() == 2
                    && metas.first().is_some_and(|cfg| {
                        cfg.to_token_stream().to_string().replace(' ', "")
                            == "all(kani,jzap_loop_contracts)"
                    });
                for meta in metas {
                    if let syn::Meta::List(list) = meta {
                        if list
                            .path
                            .segments
                            .last()
                            .is_some_and(|p| p.ident == "loop_invariant")
                        {
                            let extraction_invariant = self.metadata_getters
                                && extraction_cfg
                                && list.path.to_token_stream().to_string().replace(' ', "")
                                    == "kani::loop_invariant"
                                && self.file == "src/vec/compaction_driver.rs"
                                && self.function == "extract_next"
                                && list.tokens.to_string().replace(' ', "")
                                    == "state.valid_progress(end,total,minimum_read,minimum_write)";
                            let expr: syn::Expr =
                                syn::parse2(list.tokens).expect("invariant expression");
                            // This predicate uses the directly verified Checked getter.
                            // Keep its receiver and scope exact rather than admitting get()
                            // on arbitrary user values in metadata expressions.
                            let initialization_cursor = self.metadata_getters
                                && self.file == "src/init/driver.rs"
                                && self.function == "run"
                                && expr.to_token_stream().to_string().replace(' ', "")
                                    == "begin<=state.get()&&state.get()<=end";
                            let owner_invariant = self.metadata_getters
                                && extraction_cfg
                                && list.path.to_token_stream().to_string().replace(' ', "")
                                    == "kani::loop_invariant"
                                && OWNER_INVARIANTS.contains(&(
                                    self.file,
                                    self.function.as_str(),
                                    expr.to_token_stream().to_string().replace(' ', "").as_str(),
                                ));
                            assert!(
                                initialization_cursor
                                    || extraction_invariant
                                    || owner_invariant
                                    || pure_invariant(&expr)
                                    || pure_expression(
                                        &expr,
                                        self.metadata_getters
                                            && ((self.file == "src/vec/edit.rs"
                                                && self.function == "next")
                                                || (self.file == "src/vec/compaction_driver.rs"
                                                    && ["prefix", "sweep", "extract_next"]
                                                        .contains(&self.function.as_str()))
                                                || (self.file == "src/boxed/slice.rs"
                                                    && self.function == "try_init_slice_in"))
                                    ),
                                "loop invariant contains an unapproved operation: {}::{}",
                                self.file,
                                self.function
                            );
                            self.record("pure_loop_invariant", &expr, expr.span());
                        }
                    }
                }
            }
        }
        visit::visit_attribute(self, node);
    }
    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let saved = std::mem::replace(
            &mut self.context,
            node.self_ty.to_token_stream().to_string(),
        );
        visit::visit_item_impl(self, node);
        self.context = saved;
    }
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let saved = std::mem::replace(&mut self.function, node.sig.ident.to_string());
        visit::visit_impl_item_fn(self, node);
        self.function = saved;
    }
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let saved = std::mem::replace(&mut self.function, node.sig.ident.to_string());
        visit::visit_item_fn(self, node);
        self.function = saved;
    }
    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        let saved = std::mem::replace(&mut self.function, node.sig.ident.to_string());
        visit::visit_trait_item_fn(self, node);
        self.function = saved;
    }
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            let name = path.path.segments.last().map(|s| s.ident.to_string());
            let kind = match name.as_deref() {
                Some("wrapping_add") => Some("add"),
                Some("wrapping_sub") => Some("subtract"),
                Some("wrapping_mul") => Some("multiply"),
                _ => None,
            };
            if let Some(kind) = kind {
                assert_eq!(node.args.len(), 2, "bounded arithmetic operands");
                self.record(kind, node, node.span());
                self.rows.last_mut().expect("recorded call")["bounded_wrapping"] = json!(true);
            }
        }
        visit::visit_expr_call(self, node);
    }
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        use syn::BinOp::*;
        let kind = match node.op {
            Add(_) | AddAssign(_) => Some("add"),
            Sub(_) | SubAssign(_) => Some("subtract"),
            Mul(_) | MulAssign(_) => Some("multiply"),
            Div(_) | DivAssign(_) => Some("divide"),
            Rem(_) | RemAssign(_) => Some("remainder"),
            Shl(_) | ShlAssign(_) | Shr(_) | ShrAssign(_) => Some("shift"),
            _ => None,
        };
        if let Some(kind) = kind {
            self.record(kind, node, node.span());
        }
        visit::visit_expr_binary(self, node);
    }
    fn visit_expr_unary(&mut self, node: &'ast syn::ExprUnary) {
        if matches!(node.op, syn::UnOp::Neg(_)) {
            self.record("negate", node, node.span());
        }
        visit::visit_expr_unary(self, node);
    }
    fn visit_expr_index(&mut self, node: &'ast syn::ExprIndex) {
        self.record("index", node, node.span());
        visit::visit_expr_index(self, node);
    }
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let name = node.method.to_string();
        if ["unwrap", "expect", "unwrap_err", "expect_err"].contains(&name.as_str()) {
            self.record("unwrap", node, node.span());
        }
        visit::visit_expr_method_call(self, node);
    }
    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        self.record_invocation(&node.mac, node.span());
        visit::visit_expr_macro(self, node);
    }
    fn visit_stmt_macro(&mut self, node: &'ast syn::StmtMacro) {
        if kani_only(&node.attrs) || verus_proof_only(node) {
            return;
        }
        self.record_invocation(&node.mac, node.span());
        visit::visit_stmt_macro(self, node);
    }
    fn visit_impl_item_macro(&mut self, node: &'ast syn::ImplItemMacro) {
        self.record_invocation(&node.mac, node.span());
        visit::visit_impl_item_macro(self, node);
    }
    fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
        // Definitions contain unexpanded tokens. Keep them visible instead of
        // claiming that a syntax traversal proved their expanded operations.
        self.record(
            if node.ident.is_some() {
                "macro_definition"
            } else {
                "macro_invocation"
            },
            node,
            node.span(),
        );
        visit::visit_item_macro(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_erased_verus_proof_macros_are_omitted() {
        for (source, erased) in [
            ("#[cfg(verus_keep_ghost)] proof! { assert(false); }", true),
            ("proof! { assert(false); }", false),
            ("#[cfg(not(kani))] proof! { assert(false); }", false),
            ("#[cfg(verus_keep_ghost)] panic!(\"runtime\");", false),
        ] {
            let function: syn::ItemFn = syn::parse_str(&format!("fn f() {{ {source} }}")).unwrap();
            let syn::Stmt::Macro(statement) = &function.block.stmts[0] else {
                panic!("macro fixture")
            };
            assert_eq!(verus_proof_only(statement), erased, "{source}");
            let mut inventory = Inventory {
                file: "control.rs",
                context: String::new(),
                function: String::new(),
                rows: Vec::new(),
                metadata_getters: false,
            };
            inventory.visit_item_fn(&function);
            assert_eq!(inventory.rows.is_empty(), erased, "{source}");
        }
    }

    #[test]
    fn inventories_statement_expression_and_impl_macros() {
        let source = syn::parse_file(
            "fn f() { assert!(false); let _ = panic!(\"test\"); } impl T { generated!(); } trait U { fn g() { debug_assert!(false); } }",
        )
        .unwrap();
        let mut inventory = Inventory {
            file: "control.rs",
            context: String::new(),
            function: String::new(),
            rows: Vec::new(),
            metadata_getters: false,
        };
        inventory.visit_file(&source);
        assert_eq!(
            inventory
                .rows
                .iter()
                .filter(|r| r["kind"] == "panic_macro")
                .count(),
            3
        );
        assert_eq!(
            inventory
                .rows
                .iter()
                .filter(|r| r["kind"] == "macro_invocation")
                .count(),
            1
        );
    }

    #[test]
    fn rejects_mutating_or_unknown_invariant_expressions() {
        for source in ["x = 1", "{ x += 1; true }", "callback()", "x.update()"] {
            assert!(
                !pure_invariant(&syn::parse_str(source).unwrap()),
                "{source}"
            );
        }
        let expression =
            syn::parse_str("g.write <= g.read && g.read <= original && g.ptr == on_entry(g.ptr)")
                .unwrap();
        assert!(pure_invariant(&expression));
        for source in [
            "state.keep_next()",
            "state.valid_for(callback())",
            "external.read()",
            "self.state.stop()",
        ] {
            assert!(
                !pure_expression(&syn::parse_str(source).unwrap(), true),
                "{source}"
            );
        }
        let expression = syn::parse_str(
            "state.valid_for(original) && state.read() <= original && !self.state.untouched()",
        )
        .unwrap();
        assert!(pure_expression(&expression, true));
        // The owner's words are not readable by field in a predicate, and the
        // length getter is not part of the general grammar.
        for source in [
            "guard.owner.len.get() <= new_len",
            "guard.owner.cap.get() == on_entry(guard.owner.cap.get())",
            "guard.owner.len() <= new_len",
        ] {
            assert!(
                !pure_expression(&syn::parse_str(source).unwrap(), true),
                "{source}"
            );
        }
    }

    fn invariant_rows(file: &str, source: &str) -> usize {
        let syntax = syn::parse_file(source).unwrap();
        let mut inventory = Inventory {
            file,
            context: String::new(),
            function: String::new(),
            rows: Vec::new(),
            metadata_getters: true,
        };
        inventory.visit_file(&syntax);
        inventory
            .rows
            .iter()
            .filter(|r| r["kind"] == "pure_loop_invariant")
            .count()
    }
    fn resize_loop(predicate: &str, function: &str) -> String {
        format!(
            "impl V {{ fn {function}(&mut self) {{ #[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant({predicate}))] while a < b {{}} }} }}"
        )
    }

    #[test]
    fn accepts_each_reviewed_owner_predicate_only_at_its_loop() {
        let spaced = |text: &str| text.replace("&&", " && ");
        for (file, function, predicate) in OWNER_INVARIANTS {
            assert_eq!(
                invariant_rows(file, &resize_loop(&spaced(predicate), function)),
                1,
                "{file}::{function}"
            );
            // The same text at another function or file, or with one more
            // conjunct, is refused.
            for (file, function, predicate) in [
                ("src/vec/edit.rs", function, spaced(predicate)),
                (file, "other", spaced(predicate)),
                (
                    file,
                    function,
                    spaced(predicate) + " && guard.owner.clear()",
                ),
            ] {
                let source = resize_loop(&predicate, function);
                assert!(
                    std::panic::catch_unwind(|| invariant_rows(file, &source)).is_err(),
                    "{file}::{function}: {predicate}"
                );
            }
        }
        let by_field = resize_loop(
            "original <= guard.owner.len.get() && guard.owner.len.get() <= new_len",
            "try_resize_with_init",
        );
        assert!(std::panic::catch_unwind(|| invariant_rows("src/vec.rs", &by_field)).is_err());
    }

    #[test]
    fn reviews_the_definitions_of_the_owner_getters() {
        let owner = |body: &str| {
            format!("impl<T, P: AllocationPolicy, L: LengthPolicy> Vec<T, P, L> {{ {body} }}")
        };
        let owner_getters = |text: &str| owner_getters(text, "Vec<T,P,L>");
        let reviewed = "#[inline] pub fn len(&self) -> usize { self.len.get() } \
            #[inline] pub fn capacity(&self) -> usize { self.cap.get() } \
            #[inline] pub fn max_len(&self) -> usize { self.limit.max_len() } \
            #[inline] pub fn as_ptr(&self) -> *const T { self.ptr.as_ptr() }";
        assert_eq!(owner_getters(&owner(reviewed)).unwrap(), 4);
        assert_eq!(owner_getters(&owner("fn other(&self) {}")).unwrap(), 0);
        for changed in [
            "#[inline] pub fn len(&self) -> usize { self.cap.get() }",
            "#[inline] pub fn len(&mut self) -> usize { self.len.get() }",
            "#[inline] pub fn len(&self) -> usize { self.touch(); self.len.get() }",
            "#[inline] pub fn capacity(&self) -> usize { self.len.get() }",
            "#[inline] pub fn max_len(&self) -> usize { usize::MAX }",
            "#[inline] pub fn as_ptr(&self) -> *const T { core::ptr::null() }",
        ] {
            let source = owner(changed);
            assert!(
                std::panic::catch_unwind(|| owner_getters(&source)).is_err(),
                "{changed}"
            );
        }
    }

    #[test]
    fn reviews_the_definitions_of_the_deque_getters() {
        let owner = |body: &str| {
            format!("impl<T, P: AllocationPolicy, L: LengthPolicy> VecDeque<T, P, L> {{ {body} }}")
        };
        let getters = |text: &str| owner_getters(text, "VecDeque<T,P,L>");
        let reviewed = "#[inline] pub fn len(&self) -> usize { self.len.get() } \
            #[inline] pub fn capacity(&self) -> usize { self.cap.get() } \
            pub fn max_len(&self) -> usize { self.limit.max_len() } \
            #[cfg(kani)] fn probe_ptr(&self) -> *const T { self.ptr.as_ptr() }";
        assert_eq!(getters(&owner(reviewed)).unwrap(), 4);
        // The Vec texts are not the VecDeque review: as_ptr is not listed.
        assert_eq!(
            getters(&owner(
                "pub fn as_ptr(&self) -> *const T { core::ptr::null() }"
            ))
            .unwrap(),
            0
        );
        for changed in [
            "#[inline] pub fn len(&self) -> usize { self.head.get() }",
            "#[inline] pub fn len(&mut self) -> usize { self.len.get() }",
            "#[inline] pub fn capacity(&self) -> usize { self.len.get() }",
            "#[inline] pub fn max_len(&self) -> usize { self.limit.max_len() }",
            "pub fn max_len(&self) -> usize { usize::MAX }",
            "#[cfg(kani)] fn probe_ptr(&self) -> *const T { core::ptr::null() }",
            "fn probe_ptr(&self) -> *const T { self.ptr.as_ptr() }",
            "#[cfg(kani)] pub fn probe_ptr(&self) -> *const T { self.ptr.as_ptr() }",
        ] {
            let source = owner(changed);
            assert!(
                std::panic::catch_unwind(|| getters(&source)).is_err(),
                "{changed}"
            );
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    let approved_metadata_getters = validate_metadata_getters()?;
    for file in std::env::args().skip(1) {
        let source = std::fs::read_to_string(&file)?;
        let syntax = syn::parse_file(&source)?;
        let mut inventory = Inventory {
            file: &file,
            context: String::new(),
            function: String::new(),
            rows: Vec::new(),
            metadata_getters: approved_metadata_getters,
        };
        inventory.visit_file(&syntax);
        rows.extend(inventory.rows);
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
