//! Session Manager —— sessionId、上下文、历史 trace、取消状态（P0 §5.3）。

use std::collections::HashMap;

use crate::dto::{ConversationContext, ConversationScopeType, PinnedScope, ToolTrace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Active,
    Cancelled,
    Completed,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub context: ConversationContext,
    pub trace: Vec<ToolTrace>,
    pub status: SessionStatus,
    pub created_at: u64,
}

impl Session {
    fn new(session_id: String, snapshot_id: String, now: u64) -> Self {
        let ctx = ConversationContext {
            session_id: session_id.clone(),
            pinned_scope: None,
            snapshot_id,
            stale: false,
        };
        Self {
            session_id,
            context: ctx,
            trace: Vec::new(),
            status: SessionStatus::Active,
            created_at: now,
        }
    }
}

#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<String, Session>,
    counter: u64,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&mut self, snapshot_id: &str, now: u64) -> String {
        self.counter += 1;
        let id = format!("sess:{:x}:{}", now, self.counter);
        self.sessions.insert(
            id.clone(),
            Session::new(id.clone(), snapshot_id.to_string(), now),
        );
        id
    }

    pub fn get(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id)
    }

    pub fn pin(
        &mut self,
        session_id: &str,
        scope_type: ConversationScopeType,
        id: &str,
        snapshot_id: &str,
    ) -> bool {
        let Some(s) = self.sessions.get_mut(session_id) else {
            return false;
        };
        s.context.pinned_scope = Some(PinnedScope {
            scope_type,
            id: id.to_string(),
        });
        s.context.snapshot_id = snapshot_id.to_string();
        s.context.stale = false;
        true
    }

    pub fn mark_snapshot_changed(&mut self, snapshot_id: &str) {
        for s in self.sessions.values_mut() {
            s.context.snapshot_id = snapshot_id.to_string();
            s.context.stale = s.context.pinned_scope.is_some();
        }
    }

    pub fn append_trace(&mut self, session_id: &str, trace: ToolTrace) {
        if let Some(s) = self.sessions.get_mut(session_id) {
            s.trace.push(trace);
        }
    }

    pub fn trace(&self, session_id: &str) -> Vec<ToolTrace> {
        self.sessions
            .get(session_id)
            .map(|s| s.trace.clone())
            .unwrap_or_default()
    }

    pub fn cancel(&mut self, session_id: &str) -> bool {
        match self.sessions.get_mut(session_id) {
            Some(s) if s.status == SessionStatus::Active => {
                s.status = SessionStatus::Cancelled;
                true
            }
            _ => false,
        }
    }

    pub fn complete(&mut self, session_id: &str) -> bool {
        match self.sessions.get_mut(session_id) {
            Some(s) if s.status == SessionStatus::Active => {
                s.status = SessionStatus::Completed;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_creation_and_pin() {
        let mut m = SessionManager::new();
        let id = m.create("snap:1", 1);
        assert!(m.pin(&id, ConversationScopeType::Node, "n:a", "snap:1"));
        let s = m.get(&id).unwrap();
        assert_eq!(s.context.pinned_scope.as_ref().unwrap().id, "n:a");
        assert!(!s.context.stale);
    }

    #[test]
    fn snapshot_change_marks_pinned_sessions_stale() {
        let mut m = SessionManager::new();
        let id = m.create("snap:1", 1);
        m.pin(&id, ConversationScopeType::Node, "n:a", "snap:1");
        m.mark_snapshot_changed("snap:2");
        let s = m.get(&id).unwrap();
        assert!(s.context.stale);
        assert_eq!(s.context.snapshot_id, "snap:2");
        // pinned scope 保留，不自动重绑定
        assert_eq!(s.context.pinned_scope.as_ref().unwrap().id, "n:a");
    }

    #[test]
    fn trace_is_append_only_and_cancel_flips_status() {
        let mut m = SessionManager::new();
        let id = m.create("snap:1", 1);
        m.append_trace(
            &id,
            ToolTrace {
                tool: "search_nodes".into(),
                params: serde_json::json!({"q": "main"}),
                returned_bytes: 128,
                truncated: false,
            },
        );
        assert_eq!(m.trace(&id).len(), 1);
        assert!(m.cancel(&id));
        assert!(!m.cancel(&id), "已取消不能再次取消");
        assert_eq!(m.get(&id).unwrap().status, SessionStatus::Cancelled);
    }
}
