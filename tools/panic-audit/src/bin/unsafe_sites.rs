//! Source-level unsafe/metadata census, including unexpanded macro definition sites.
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
fn metadata(member: &syn::Member) -> bool {
    matches!(member, syn::Member::Named(n) if ["ptr","len","cap","layout","front","back","read","write","original","total","count"].contains(&n.to_string().as_str()))
}
struct Census {
    file: String,
    function: String,
    context: String,
    rows: Vec<Value>,
}
impl Census {
    fn record(&mut self, kind: &str, span: proc_macro2::Span, expression: String) {
        self.rows.push(
            json!({"file":self.file,"function":self.function,"context":self.context,
            "kind":kind,"line":span.start().line,"column":span.start().column,
            "end_line":span.end().line,"end_column":span.end().column,"expression":expression}),
        );
    }
    fn macro_tokens(&mut self, tokens: proc_macro2::TokenStream, name: &str) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        for (i, token) in tokens.iter().enumerate() {
            if matches!(token,proc_macro2::TokenTree::Ident(n) if n=="unsafe") {
                let kind = match tokens.get(i + 1) {
                    Some(proc_macro2::TokenTree::Group(g))
                        if g.delimiter() == proc_macro2::Delimiter::Brace =>
                    {
                        "macro_unsafe_block"
                    }
                    Some(proc_macro2::TokenTree::Ident(n)) if n == "fn" => "macro_unsafe_fn",
                    _ => "macro_unsafe_other",
                };
                let expression = if kind == "macro_unsafe_fn" {
                    tokens.iter().skip(i + 1).take_while(|t| !matches!(t, proc_macro2::TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Brace)).cloned().collect::<proc_macro2::TokenStream>().to_string()
                } else {
                    tokens
                        .get(i + 1)
                        .map(ToString::to_string)
                        .unwrap_or_default()
                };
                let old = std::mem::replace(&mut self.function, name.to_owned());
                self.record(kind, token.span(), expression);
                self.function = old;
            }
            if let proc_macro2::TokenTree::Group(group) = token {
                self.macro_tokens(group.stream(), name);
            }
        }
    }
}
impl<'a> Visit<'a> for Census {
    fn visit_item_mod(&mut self, n: &'a syn::ItemMod) {
        if !proof_only(&n.attrs) {
            visit::visit_item_mod(self, n);
        }
    }
    fn visit_item_impl(&mut self, n: &'a syn::ItemImpl) {
        if proof_only(&n.attrs) {
            return;
        }
        let old = std::mem::replace(&mut self.context, n.self_ty.to_token_stream().to_string());
        if n.unsafety.is_some() {
            self.record(
                "unsafe_impl",
                n.impl_token.span(),
                n.to_token_stream().to_string(),
            );
        }
        visit::visit_item_impl(self, n);
        self.context = old;
    }
    fn visit_item_fn(&mut self, n: &'a syn::ItemFn) {
        if proof_only(&n.attrs) {
            return;
        }
        let old = std::mem::replace(&mut self.function, n.sig.ident.to_string());
        if n.sig.unsafety.is_some() {
            self.record(
                "unsafe_fn",
                n.sig.span(),
                n.sig.to_token_stream().to_string(),
            );
        }
        visit::visit_item_fn(self, n);
        self.function = old;
    }
    fn visit_impl_item_fn(&mut self, n: &'a syn::ImplItemFn) {
        if proof_only(&n.attrs) {
            return;
        }
        let old = std::mem::replace(&mut self.function, n.sig.ident.to_string());
        if n.sig.unsafety.is_some() {
            self.record(
                "unsafe_fn",
                n.sig.span(),
                n.sig.to_token_stream().to_string(),
            );
        }
        visit::visit_impl_item_fn(self, n);
        self.function = old;
    }
    fn visit_expr_unsafe(&mut self, n: &'a syn::ExprUnsafe) {
        self.record("unsafe_block", n.span(), n.to_token_stream().to_string());
        visit::visit_expr_unsafe(self, n);
    }
    fn visit_expr_assign(&mut self, n: &'a syn::ExprAssign) {
        if matches!(&*n.left,syn::Expr::Field(f) if metadata(&f.member)) {
            self.record("metadata_write", n.span(), n.to_token_stream().to_string());
        }
        visit::visit_expr_assign(self, n);
    }
    fn visit_expr_struct(&mut self, n: &'a syn::ExprStruct) {
        for field in &n.fields {
            if metadata(&field.member) {
                self.record(
                    "metadata_init",
                    field.span(),
                    field.to_token_stream().to_string(),
                );
            }
        }
        visit::visit_expr_struct(self, n);
    }
    fn visit_item_macro(&mut self, n: &'a syn::ItemMacro) {
        let name = n
            .ident
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        self.macro_tokens(n.mac.tokens.clone(), &name);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for file in std::env::args().skip(1) {
        let syntax = syn::parse_file(&std::fs::read_to_string(&file)?)?;
        let mut census = Census {
            file,
            function: String::new(),
            context: String::new(),
            rows: Vec::new(),
        };
        census.visit_file(&syntax);
        rows.extend(census.rows);
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
