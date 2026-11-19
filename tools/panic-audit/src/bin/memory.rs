//! Verification-only AST census: explicit modular arithmetic and actual accesses.
use quote::ToTokens;
use serde_json::{Value, json};
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
};

fn modular(name: &str) -> bool {
    ["wrapping_", "wrap_", "overflowing_", "unchecked_"]
        .iter()
        .any(|p| name.starts_with(p))
}
fn modular_tokens(tokens: proc_macro2::TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        proc_macro2::TokenTree::Ident(name) => {
            modular(&name.to_string()) || name == "WrappingArithmetic"
        }
        proc_macro2::TokenTree::Group(group) => modular_tokens(group.stream()),
        _ => false,
    })
}
fn proof_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("cfg")
            && matches!(&a.meta, syn::Meta::List(m) if m.tokens.to_string() == "kani")
    })
}
struct Census<'a> {
    file: &'a str,
    context: String,
    function: String,
    rows: Vec<Value>,
}
impl Census<'_> {
    fn record(&mut self, kind: &str, node: &impl ToTokens, span: proc_macro2::Span) {
        self.rows.push(
            json!({"file":self.file,"context":self.context,"function":self.function,
            "kind":kind,"line":span.start().line,"column":span.start().column,
            "expression":node.to_token_stream().to_string()}),
        );
    }
}
impl<'ast> Visit<'ast> for Census<'_> {
    fn visit_item_mod(&mut self, n: &'ast syn::ItemMod) {
        if !proof_only(&n.attrs) {
            visit::visit_item_mod(self, n);
        }
    }
    fn visit_item_impl(&mut self, n: &'ast syn::ItemImpl) {
        let saved = core::mem::replace(&mut self.context, n.self_ty.to_token_stream().to_string());
        visit::visit_item_impl(self, n);
        self.context = saved;
    }
    fn visit_impl_item_fn(&mut self, n: &'ast syn::ImplItemFn) {
        if proof_only(&n.attrs) {
            return;
        }
        let saved = core::mem::replace(&mut self.function, n.sig.ident.to_string());
        self.record("body", &n.block, n.span());
        visit::visit_impl_item_fn(self, n);
        self.function = saved;
    }
    fn visit_item_fn(&mut self, n: &'ast syn::ItemFn) {
        if proof_only(&n.attrs) {
            return;
        }
        let saved = core::mem::replace(&mut self.function, n.sig.ident.to_string());
        self.record("body", &n.block, n.span());
        visit::visit_item_fn(self, n);
        self.function = saved;
    }
    fn visit_expr_call(&mut self, n: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*n.func {
            if p.path
                .segments
                .last()
                .is_some_and(|p| modular(&p.ident.to_string()))
            {
                self.record("modular_call", n, n.span());
                for arg in &n.args {
                    self.visit_expr(arg);
                }
                return;
            }
        }
        visit::visit_expr_call(self, n);
    }
    fn visit_expr_path(&mut self, n: &'ast syn::ExprPath) {
        if n.path
            .segments
            .last()
            .is_some_and(|p| modular(&p.ident.to_string()))
        {
            self.record("modular_reference", n, n.span());
        }
        visit::visit_expr_path(self, n);
    }
    fn visit_expr_method_call(&mut self, n: &'ast syn::ExprMethodCall) {
        let name = n.method.to_string();
        if modular(&name) {
            self.record("modular_method", n, n.span());
        }
        if [
            "add",
            "sub",
            "read",
            "write",
            "get_unchecked",
            "get_unchecked_mut",
        ]
        .contains(&name.as_str())
        {
            self.record("memory_method", n, n.span());
        }
        visit::visit_expr_method_call(self, n);
    }
    fn visit_item_use(&mut self, n: &'ast syn::ItemUse) {
        if modular_tokens(n.tree.to_token_stream()) {
            self.record("modular_import", n, n.span());
        }
        visit::visit_item_use(self, n);
    }
    fn visit_macro(&mut self, n: &'ast syn::Macro) {
        if modular_tokens(n.tokens.clone()) {
            self.record("modular_macro", n, n.span());
        }
        visit::visit_macro(self, n);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for file in std::env::args().skip(1) {
        let syntax = syn::parse_file(&std::fs::read_to_string(&file)?)?;
        let mut census = Census {
            file: &file,
            context: String::new(),
            function: String::new(),
            rows: Vec::new(),
        };
        census.visit_file(&syntax);
        rows.extend(census.rows);
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
