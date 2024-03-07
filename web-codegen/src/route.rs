use syn::{ItemFn, Meta};
use syn::visit::Visit;

pub struct FnVisitor {
    pub route_fns: Vec<String>,
}

impl FnVisitor {
    pub fn new() -> Self {
        Self {
            route_fns: vec![],
        }
    }
}


impl Visit<'_> for FnVisitor {
    fn visit_item_fn(&mut self, item_fn: &ItemFn) {
        for x in &item_fn.attrs {
            if let Meta::List(list) = &x.meta {
                if let Some(ident) = list.path.get_ident() {
                    if ["post", "get", "put", "delete"].contains(&ident.to_string().as_str()) {
                        self.route_fns.push(item_fn.sig.ident.to_string());
                    }
                }
            }
        }
    }
}