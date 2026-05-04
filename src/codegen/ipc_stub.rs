//! V27.5 P2: IPC service stub auto-generation.
//!
//! Generates dispatch loop + client proxy function names for `service {}`
//! blocks. The generated metadata is consumed by:
//! - V29 OS build: emit dispatch loops alongside service ELFs
//! - V29 client code: emit proxy functions for cross-service calls
//!
//! Current scope: provide naming + ID conventions. Full codegen of the
//! actual loop/proxy bodies happens in V29 when service ELFs are split.
//!
//! Naming convention:
//! - Server dispatcher:    `__svc_dispatch_{service_name}`
//! - Client proxy:         `__svc_call_{service_name}_{handler_name}`
//! - Message ID constant:  `__SVC_{SERVICE}_{HANDLER}_MSG_ID`

use crate::parser::ast::ServiceDef;

/// Metadata describing one handler within a service.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceHandler {
    /// Handler function name (e.g., "open", "read").
    pub handler_name: String,
    /// Auto-assigned message ID (1-indexed, sequential per service).
    pub message_id: u32,
    /// Generated dispatch arm name (for the server loop).
    pub dispatch_label: String,
    /// Generated client proxy function name.
    pub proxy_name: String,
    /// Generated message ID constant name.
    pub id_const_name: String,
}

/// Metadata for an entire service: dispatch loop name + handler list.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceStub {
    /// Service name as written in source (e.g., "vfs").
    pub service_name: String,
    /// Generated server dispatch loop function name.
    pub dispatch_fn: String,
    /// Per-handler metadata.
    pub handlers: Vec<ServiceHandler>,
}

impl ServiceStub {
    /// Generate complete service stub metadata from a parsed `ServiceDef`.
    /// Message IDs are assigned 1, 2, 3... in declaration order.
    pub fn from_service_def(svc: &ServiceDef) -> Self {
        let dispatch_fn = format!("__svc_dispatch_{}", svc.name);
        let svc_upper = svc.name.to_uppercase();
        let handlers = svc
            .handlers
            .iter()
            .enumerate()
            .map(|(idx, h)| {
                let id = (idx + 1) as u32;
                ServiceHandler {
                    handler_name: h.name.clone(),
                    message_id: id,
                    dispatch_label: format!("dispatch_{}", h.name),
                    proxy_name: format!("__svc_call_{}_{}", svc.name, h.name),
                    id_const_name: format!("__SVC_{}_{}_MSG_ID", svc_upper, h.name.to_uppercase()),
                }
            })
            .collect();
        ServiceStub {
            service_name: svc.name.clone(),
            dispatch_fn,
            handlers,
        }
    }

    /// Returns the number of handlers in this service.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Look up a handler's message ID by name.
    pub fn message_id_of(&self, handler: &str) -> Option<u32> {
        self.handlers
            .iter()
            .find(|h| h.handler_name == handler)
            .map(|h| h.message_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Span;
    use crate::parser::ast::{FnDef, TypeExpr};

    fn make_handler(name: &str) -> FnDef {
        FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: name.to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![],
            return_type: Some(TypeExpr::Simple {
                name: "i64".to_string(),
                span: Span::new(0, 0),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(crate::parser::ast::Expr::Block {
                stmts: vec![],
                expr: None,
                span: Span::new(0, 0),
            }),
            span: Span::new(0, 0),
        }
    }

    fn make_service(name: &str, handler_names: &[&str]) -> ServiceDef {
        ServiceDef {
            name: name.to_string(),
            annotation: None,
            implements: None,
            handlers: handler_names.iter().map(|n| make_handler(n)).collect(),
            span: Span::new(0, 0),
        }
    }

    #[test]
    fn stub_dispatch_fn_name() {
        let svc = make_service("vfs", &["open", "read"]);
        let stub = ServiceStub::from_service_def(&svc);
        assert_eq!(stub.dispatch_fn, "__svc_dispatch_vfs");
    }

    #[test]
    fn stub_handler_count() {
        let svc = make_service("net", &["bind", "listen", "accept"]);
        let stub = ServiceStub::from_service_def(&svc);
        assert_eq!(stub.handler_count(), 3);
    }

    #[test]
    fn stub_message_ids_sequential() {
        let svc = make_service("vfs", &["open", "read", "write", "close"]);
        let stub = ServiceStub::from_service_def(&svc);
        assert_eq!(stub.handlers[0].message_id, 1);
        assert_eq!(stub.handlers[1].message_id, 2);
        assert_eq!(stub.handlers[2].message_id, 3);
        assert_eq!(stub.handlers[3].message_id, 4);
    }

    #[test]
    fn stub_proxy_naming() {
        let svc = make_service("vfs", &["open"]);
        let stub = ServiceStub::from_service_def(&svc);
        assert_eq!(stub.handlers[0].proxy_name, "__svc_call_vfs_open");
    }

    #[test]
    fn stub_id_const_naming() {
        let svc = make_service("vfs", &["open"]);
        let stub = ServiceStub::from_service_def(&svc);
        assert_eq!(stub.handlers[0].id_const_name, "__SVC_VFS_OPEN_MSG_ID");
    }

    #[test]
    fn stub_lookup_by_handler_name() {
        let svc = make_service("net", &["bind", "listen", "accept"]);
        let stub = ServiceStub::from_service_def(&svc);
        assert_eq!(stub.message_id_of("bind"), Some(1));
        assert_eq!(stub.message_id_of("accept"), Some(3));
        assert_eq!(stub.message_id_of("unknown"), None);
    }

    #[test]
    fn stub_empty_service() {
        let svc = make_service("empty", &[]);
        let stub = ServiceStub::from_service_def(&svc);
        assert_eq!(stub.handler_count(), 0);
        assert_eq!(stub.dispatch_fn, "__svc_dispatch_empty");
    }

    #[test]
    fn stub_multiple_services_independent() {
        // Two services should have independent ID sequences
        let svc1 = make_service("vfs", &["open", "read"]);
        let svc2 = make_service("net", &["bind", "listen"]);
        let stub1 = ServiceStub::from_service_def(&svc1);
        let stub2 = ServiceStub::from_service_def(&svc2);
        assert_eq!(stub1.handlers[0].message_id, 1);
        assert_eq!(stub2.handlers[0].message_id, 1); // independent
        assert_ne!(stub1.dispatch_fn, stub2.dispatch_fn);
    }
}
