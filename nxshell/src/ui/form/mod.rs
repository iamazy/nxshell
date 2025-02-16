use crate::db::Session;
use indexmap::IndexMap;

mod session;
pub use session::AuthType;

#[derive(Default)]
pub struct NxStateManager {
    // db state
    pub sessions: Option<IndexMap<String, Vec<Session>>>,
}
