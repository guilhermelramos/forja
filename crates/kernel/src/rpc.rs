//! RPC methods contributed by modules.
//!
//! The schema is the protocol-facing half (name, description, params). The
//! handler lives here because it receives `Ctx`, which the protocol crate must
//! not know about. `meta.methods` serializes only the schemas.

use crate::runtime::Ctx;
use forja_protocol::{MethodSchema, RpcError};
use serde_json::{Map, Value};
use std::sync::Arc;

pub type RpcHandler = Arc<
    dyn for<'a, 'b> Fn(&'b mut Ctx<'a>, Map<String, Value>) -> Result<Value, RpcError>
        + Send
        + Sync,
>;

pub struct MethodContribution {
    pub module: String,
    pub schema: MethodSchema,
    pub handler: RpcHandler,
}
