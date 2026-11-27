//! Exhaustive field writes/constructions and accounting/ownership transitions.
//! No field-name allowlist: newly added state must receive a proof binding.
//!
//! A field can also change without an assignment. Every `&mut` and
//! `&raw mut` of a field place is a `field_borrow` row (this includes the
//! arguments of `mem::swap`, `mem::replace` and `mem::take`), and every field
//! of a struct pattern is a `field_pattern` row (a `ref mut` binding, or a
//! plain binding that default binding modes turn into a `&mut`). Not recorded
//! here: a method call that borrows its field receiver by auto-reference, and
//! the operand of `addr_of_mut!`, which is an unparsed macro body.
use quote::ToTokens;
use serde_json::{Value, json};
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
};

fn proof_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("cfg")
            && match &a.meta {
                syn::Meta::List(m) => {
                    matches!(m.tokens.to_string().as_str(), "kani" | "test")
                        || m.tokens.to_string().starts_with("all (kani ,")
                }
                _ => false,
            }
    })
}
// The members of a field place, outermost last: `self.drain.owner.len` gives
// `["drain", "owner", "len"]`. Empty when the place is not a field access.
fn field_members(mut expr: &syn::Expr) -> Vec<String> {
    let mut members = Vec::new();
    loop {
        match expr {
            syn::Expr::Paren(n) => expr = &n.expr,
            syn::Expr::Group(n) => expr = &n.expr,
            syn::Expr::Field(n) => {
                members.push(n.member.to_token_stream().to_string());
                expr = &n.base;
            }
            _ => break,
        }
    }
    members.reverse();
    members
}
// Whether a pattern binds a name by `ref mut`, at any depth.
fn binds_ref_mut(pat: &syn::Pat) -> bool {
    struct Search(bool);
    impl<'a> Visit<'a> for Search {
        fn visit_pat_ident(&mut self, n: &'a syn::PatIdent) {
            self.0 |= n.by_ref.is_some() && n.mutability.is_some();
            visit::visit_pat_ident(self, n);
        }
    }
    let mut search = Search(false);
    search.visit_pat(pat);
    search.0
}
struct Census {
    file: String,
    context: String,
    function: String,
    rows: Vec<Value>,
    declarations: bool,
}
impl Census {
    fn declaration(&self, attrs: &[syn::Attribute]) -> Value {
        if !self.declarations {
            return json!({});
        }
        let direct_contract = attrs.iter().any(|a| {
            if !a.path().is_ident("cfg_attr") {
                return false;
            }
            let Ok(parts) = a.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            ) else {
                return false;
            };
            matches!(parts.first(), Some(syn::Meta::Path(p)) if p.is_ident("verus_keep_ghost"))
                && parts
                    .iter()
                    .skip(1)
                    .any(|m| matches!(m, syn::Meta::List(l) if l.path.is_ident("verus_spec")))
        });
        json!({"direct_contract":direct_contract,
            "attributes":attrs.iter().map(|a|a.meta.to_token_stream().to_string()).collect::<Vec<_>>()})
    }
    fn record(&mut self, kind: &str, node: &impl ToTokens, span: proc_macro2::Span, extra: Value) {
        if self.declarations
            && !matches!(
                kind,
                "declaration"
                    | "struct_declaration"
                    | "module_declaration"
                    | "item_macro"
                    | "unreviewed_declaration"
            )
        {
            return;
        }
        self.rows.push(
            json!({"file":self.file,"context":self.context,"function":self.function,
            "kind":kind,"line":span.start().line,"column":span.start().column,
            "end_line":span.end().line,"end_column":span.end().column,
            "expression":node.to_token_stream().to_string(),"detail":extra}),
        );
    }
}
impl<'a> Visit<'a> for Census {
    fn visit_item_trait(&mut self, n: &'a syn::ItemTrait) {
        if proof_only(&n.attrs) {
            return;
        }
        // Record the whole sealed protocol, including any future default body.
        // Abstract trait methods are otherwise invisible to the body census.
        if self.file == "src/shared/count.rs" && n.ident == "Counter" {
            self.record("counter_trait", n, n.span(), json!({}));
        }
        visit::visit_item_trait(self, n);
    }
    fn visit_item_mod(&mut self, n: &'a syn::ItemMod) {
        if !proof_only(&n.attrs) {
            if self.declarations {
                self.record(
                    "module_declaration",
                    &n.ident,
                    n.span(),
                    json!({"name":n.ident.to_string()}),
                );
            }
            visit::visit_item_mod(self, n);
        }
    }
    fn visit_item_struct(&mut self, n: &'a syn::ItemStruct) {
        if self.declarations && !proof_only(&n.attrs) {
            let fields: Vec<_> = n
                .fields
                .iter()
                .map(|f| {
                    json!({
                        "name":f.ident.as_ref().map(ToString::to_string),
                        "visibility":f.vis.to_token_stream().to_string(),
                        "type":f.ty.to_token_stream().to_string(),
                    })
                })
                .collect();
            self.record(
                "struct_declaration",
                &n.ident,
                n.span(),
                json!({"name":n.ident.to_string(),"fields":fields}),
            );
        }
        visit::visit_item_struct(self, n);
    }
    fn visit_item_impl(&mut self, n: &'a syn::ItemImpl) {
        if proof_only(&n.attrs) {
            return;
        }
        if self.declarations && n.trait_.is_some() {
            self.record(
                "unreviewed_declaration",
                &n.self_ty,
                n.span(),
                json!({"item":"trait implementation"}),
            );
        }
        let old = std::mem::replace(&mut self.context, n.self_ty.to_token_stream().to_string());
        visit::visit_item_impl(self, n);
        self.context = old;
    }
    fn visit_item_const(&mut self, n: &'a syn::ItemConst) {
        if self.declarations && !proof_only(&n.attrs) {
            self.record(
                "unreviewed_declaration",
                &n.ident,
                n.span(),
                json!({"item":"constant"}),
            );
        }
        visit::visit_item_const(self, n);
    }
    fn visit_impl_item_const(&mut self, n: &'a syn::ImplItemConst) {
        if self.declarations && !proof_only(&n.attrs) {
            self.record(
                "unreviewed_declaration",
                &n.ident,
                n.span(),
                json!({"item":"associated constant"}),
            );
        }
        visit::visit_impl_item_const(self, n);
    }
    fn visit_item_static(&mut self, n: &'a syn::ItemStatic) {
        if self.declarations && !proof_only(&n.attrs) {
            self.record(
                "unreviewed_declaration",
                &n.ident,
                n.span(),
                json!({"item":"static"}),
            );
        }
        visit::visit_item_static(self, n);
    }
    fn visit_item_foreign_mod(&mut self, n: &'a syn::ItemForeignMod) {
        if self.declarations && !proof_only(&n.attrs) {
            self.record(
                "unreviewed_declaration",
                &n.abi,
                n.span(),
                json!({"item":"foreign module"}),
            );
        }
        visit::visit_item_foreign_mod(self, n);
    }
    fn visit_item_fn(&mut self, n: &'a syn::ItemFn) {
        if proof_only(&n.attrs) {
            return;
        }
        let old = std::mem::replace(&mut self.function, n.sig.ident.to_string());
        if self.declarations {
            self.record("declaration", &n.sig, n.span(), self.declaration(&n.attrs));
        } else {
            self.record("body", &n.block, n.span(), json!({}));
        }
        visit::visit_item_fn(self, n);
        self.function = old;
    }
    fn visit_impl_item_fn(&mut self, n: &'a syn::ImplItemFn) {
        if proof_only(&n.attrs) {
            return;
        }
        let old = std::mem::replace(&mut self.function, n.sig.ident.to_string());
        if self.declarations {
            self.record("declaration", &n.sig, n.span(), self.declaration(&n.attrs));
        } else {
            let detail = if self.file == "src/vec/edit.rs"
                && (["try_retain_mut", "try_dedup_by"].contains(&self.function.as_str())
                    || (self.function == "next" && self.context.starts_with("ExtractIf")))
            {
                json!({"statements":n.block.stmts.iter().map(|stmt|stmt.to_token_stream().to_string()).collect::<Vec<_>>()})
            } else {
                json!({})
            };
            self.record("body", &n.block, n.span(), detail);
        }
        visit::visit_impl_item_fn(self, n);
        self.function = old;
    }
    fn visit_expr_assign(&mut self, n: &'a syn::ExprAssign) {
        self.record("write",n,n.span(),json!({"target":n.left.to_token_stream().to_string(),"value":n.right.to_token_stream().to_string()}));
        visit::visit_expr_assign(self, n);
    }
    fn visit_item_macro(&mut self, n: &'a syn::ItemMacro) {
        if self.declarations && !proof_only(&n.attrs) {
            let mut detail = self.declaration(&n.attrs);
            detail["path"] = json!(n.mac.path.to_token_stream().to_string());
            self.record("item_macro", &n.mac.path, n.span(), detail);
        }
        visit::visit_item_macro(self, n);
    }
    fn visit_expr_binary(&mut self, n: &'a syn::ExprBinary) {
        use syn::BinOp::*;
        if matches!(
            n.op,
            AddAssign(_)
                | SubAssign(_)
                | MulAssign(_)
                | DivAssign(_)
                | RemAssign(_)
                | BitXorAssign(_)
                | BitAndAssign(_)
                | BitOrAssign(_)
                | ShlAssign(_)
                | ShrAssign(_)
        ) {
            self.record("compound_write", n, n.span(), json!({}));
        }
        visit::visit_expr_binary(self, n);
    }
    fn visit_expr_reference(&mut self, n: &'a syn::ExprReference) {
        let members = field_members(&n.expr);
        if n.mutability.is_some() && !members.is_empty() {
            self.record(
                "field_borrow",
                n,
                n.span(),
                json!({"member":members.last(),"members":members,"raw":false}),
            );
        }
        visit::visit_expr_reference(self, n);
    }
    fn visit_expr_raw_addr(&mut self, n: &'a syn::ExprRawAddr) {
        let members = field_members(&n.expr);
        if matches!(n.mutability, syn::PointerMutability::Mut(_)) && !members.is_empty() {
            self.record(
                "field_borrow",
                n,
                n.span(),
                json!({"member":members.last(),"members":members,"raw":true}),
            );
        }
        visit::visit_expr_raw_addr(self, n);
    }
    fn visit_pat_struct(&mut self, n: &'a syn::PatStruct) {
        for f in &n.fields {
            // A wildcard binds nothing; every other field pattern can.
            if matches!(&*f.pat, syn::Pat::Wild(_)) {
                continue;
            }
            self.record("field_pattern",f,f.span(),json!({"structure":n.path.to_token_stream().to_string(),"member":f.member.to_token_stream().to_string(),"ref_mut":binds_ref_mut(&f.pat)}));
        }
        visit::visit_pat_struct(self, n);
    }
    fn visit_expr_struct(&mut self, n: &'a syn::ExprStruct) {
        for f in &n.fields {
            self.record("field",f,f.span(),json!({"constructor":n.path.to_token_stream().to_string(),"field":f.member.to_token_stream().to_string(),"value":f.expr.to_token_stream().to_string()}));
        }
        visit::visit_expr_struct(self, n);
    }
    fn visit_expr_method_call(&mut self, n: &'a syn::ExprMethodCall) {
        if ["previous_kept", "keep_live", "remove_live"].contains(&n.method.to_string().as_str()) {
            self.record(
                "admitted_step",
                n,
                n.span(),
                json!({"method":n.method.to_string()}),
            );
        }
        if ["unwrap", "expect", "unwrap_err", "expect_err"].contains(&n.method.to_string().as_str())
        {
            self.record(
                "explicit_exit",
                n,
                n.span(),
                json!({"method":n.method.to_string()}),
            );
        }
        if ["resize_storage", "grow_to"].contains(&n.method.to_string().as_str()) {
            self.record("private_helper",n,n.span(),json!({"method":n.method.to_string(),"receiver":n.receiver.to_token_stream().to_string(),"arguments":n.args.to_token_stream().to_string()}));
        }
        if ["set", "reserve", "release", "commit", "retain"]
            .contains(&n.method.to_string().as_str())
        {
            self.record("protocol_method",n,n.span(),json!({"method":n.method.to_string(),"receiver":n.receiver.to_token_stream().to_string(),"arguments":n.args.to_token_stream().to_string()}));
        }
        visit::visit_expr_method_call(self, n);
    }
    fn visit_item_use(&mut self, n: &'a syn::ItemUse) {
        fn driver(tree: &syn::UseTree) -> bool {
            match tree {
                syn::UseTree::Path(p) => {
                    ["compaction_driver", "staging_driver"].contains(&p.ident.to_string().as_str())
                        || driver(&p.tree)
                }
                syn::UseTree::Name(n) => {
                    ["compaction_driver", "staging_driver"].contains(&n.ident.to_string().as_str())
                }
                syn::UseTree::Rename(n) => {
                    ["compaction_driver", "staging_driver"].contains(&n.ident.to_string().as_str())
                        || ["compaction_driver", "staging_driver"]
                            .contains(&n.rename.to_string().as_str())
                }
                syn::UseTree::Group(g) => g.items.iter().any(driver),
                syn::UseTree::Glob(_) => false,
            }
        }
        if driver(&n.tree) {
            self.record("compaction_driver_import", n, n.span(), json!({}));
        }
        visit::visit_item_use(self, n);
    }
    fn visit_expr_path(&mut self, n: &'a syn::ExprPath) {
        if n.path.segments.iter().any(|part| {
            ["compaction_driver", "staging_driver"].contains(&part.ident.to_string().as_str())
        }) {
            self.record("compaction_driver_reference", n, n.span(), json!({}));
        }

        if n.path.segments.last().is_some_and(|s| {
            ["previous_kept", "keep_live", "remove_live"].contains(&s.ident.to_string().as_str())
        }) {
            self.record("admitted_reference", n, n.span(), json!({}));
        }
        visit::visit_expr_path(self, n);
    }
    fn visit_expr_call(&mut self, n: &'a syn::ExprCall) {
        if let syn::Expr::Path(p) = &*n.func {
            let name = p
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            if [
                "abort",
                "exit",
                "_Exit",
                "_exit",
                "quick_exit",
                "pthread_exit",
                "unreachable_unchecked",
                "assert_unchecked",
                "handle_alloc_error",
            ]
            .contains(&name.as_str())
            {
                self.record(
                    "explicit_exit",
                    n,
                    n.span(),
                    json!({"callee":p.path.to_token_stream().to_string()}),
                );
            }
            if [
                "allocate",
                "allocate_zeroed",
                "deallocate",
                "reallocate",
                "forget",
                "swap",
                "replace",
                "from_raw_parts",
                "into_raw_parts",
                "AllocationGuard",
                "Allocation",
            ]
            .contains(&name.as_str())
            {
                self.record("protocol_call",n,n.span(),json!({"callee":p.path.to_token_stream().to_string(),"arguments":n.args.to_token_stream().to_string()}));
            }
        }
        visit::visit_expr_call(self, n);
    }
    fn visit_stmt_macro(&mut self, n: &'a syn::StmtMacro) {
        if proof_only(&n.attrs) {
            return;
        }
        visit::visit_stmt_macro(self, n);
    }
    fn visit_macro(&mut self, n: &'a syn::Macro) {
        if n.path.segments.last().is_some_and(|s| {
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
                "asm",
                "global_asm",
            ]
            .contains(&s.ident.to_string().as_str())
        }) {
            self.record(
                "explicit_exit",
                n,
                n.path.span(),
                json!({"macro":n.path.to_token_stream().to_string()}),
            );
        }
        visit::visit_macro(self, n);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let declarations = args.iter().any(|a| a == "--declarations");
    for file in args.into_iter().filter(|a| a != "--declarations") {
        let parsed = syn::parse_file(&std::fs::read_to_string(&file)?)?;
        let mut c = Census {
            file,
            context: String::new(),
            function: String::new(),
            rows: Vec::new(),
            declarations,
        };
        c.visit_file(&parsed);
        rows.extend(c.rows);
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
