//! Parse the operations inside every real and macro unsafe block. The verifier
//! must reject unknown intrinsics/FFI/macros, rather than treating them as a
//! representation-only pointer operation.
use quote::ToTokens;
use serde_json::{Value, json};
use syn::visit::{self, Visit};
struct Operations {
    rows: Vec<Value>,
}
impl<'a> Visit<'a> for Operations {
    fn visit_expr_call(&mut self, n: &'a syn::ExprCall) {
        let path = match &*n.func {
            syn::Expr::Path(p) => p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
            _ => n.func.to_token_stream().to_string(),
        };
        self.rows.push(json!({"kind":"call","operation":path}));
        visit::visit_expr_call(self, n);
    }
    fn visit_expr_method_call(&mut self, n: &'a syn::ExprMethodCall) {
        self.rows
            .push(json!({"kind":"method","operation":n.method.to_string()}));
        visit::visit_expr_method_call(self, n);
    }
    fn visit_macro(&mut self, n: &'a syn::Macro) {
        self.rows.push(json!({"kind":"macro","operation":n.path.to_token_stream().to_string().replace(' ',"")}));
        if n.path
            .segments
            .last()
            .is_some_and(|s| s.ident == "addr_of_mut")
        {
            let arg = syn::parse2::<syn::Expr>(n.tokens.clone()).expect("address macro expression");
            self.visit_expr(&arg);
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("unsafe census JSON path");
    let mut sites: Vec<Value> = serde_json::from_slice(&std::fs::read(path)?)?;
    for site in &mut sites {
        if !matches!(
            site["kind"].as_str(),
            Some("unsafe_block" | "macro_unsafe_block")
        ) {
            continue;
        }
        let text = site["expression"]
            .as_str()
            .expect("expression")
            .replace('$', "");
        let expr = syn::parse_str::<syn::Expr>(&text)?;
        let mut operations = Operations { rows: Vec::new() };
        operations.visit_expr(&expr);
        site["operations"] = json!(operations.rows);
    }
    println!("{}", serde_json::to_string(&sites)?);
    Ok(())
}
