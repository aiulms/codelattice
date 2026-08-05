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
    /// 已成功完成的对话轮次；完成一轮不会关闭 conversation session。
    pub completed_turns: u64,
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
            completed_turns: 0,
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

    /// 验证一次 Chat 请求仍绑定到同一 snapshot/pinned scope。
    /// 前后端任一侧上下文漂移都必须显式重建或重新 pin，禁止静默读取旧图。
    pub fn validate_turn(
        &self,
        session_id: &str,
        snapshot_id: &str,
        pinned_scope: Option<&PinnedScope>,
    ) -> Result<(), String> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| format!("session not found: {session_id}"))?;
        if session.status != SessionStatus::Active {
            return Err(format!("session is not active: {:?}", session.status));
        }
        if session.context.stale {
            return Err(
                "session snapshot is stale; recreate or re-pin before chatting".to_string(),
            );
        }
        if session.context.snapshot_id != snapshot_id {
            return Err(format!(
                "session snapshot mismatch: expected {}, got {}",
                session.context.snapshot_id, snapshot_id
            ));
        }
        if session.context.pinned_scope.as_ref() != pinned_scope {
            return Err("session pinned scope mismatch; re-pin before chatting".to_string());
        }
        Ok(())
    }

    /// 完成单个请求轮次，conversation session 保持 Active 以支持多轮对话。
    pub fn finish_turn(&mut self, session_id: &str) -> bool {
        match self.sessions.get_mut(session_id) {
            Some(session) if session.status == SessionStatus::Active => {
                session.completed_turns += 1;
                true
            }
            _ => false,
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

    /// 关闭并移除 session（返工新增：session close 接口）。
    pub fn close(&mut self, session_id: &str) -> bool {
        self.sessions.remove(session_id).is_some()
    }

    /// 检查 session 是否存在且未关闭。
    pub fn exists(&self, session_id: &str) -> bool {
        self.sessions.contains_key(session_id)
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

    #[test]
    fn active_conversation_accepts_two_completed_turns() {
        let mut m = SessionManager::new();
        let id = m.create("snap:1", 1);
        let scope = PinnedScope {
            scope_type: ConversationScopeType::Node,
            id: "n:a".into(),
        };
        assert!(m.pin(&id, scope.scope_type, &scope.id, "snap:1"));

        m.validate_turn(&id, "snap:1", Some(&scope)).unwrap();
        assert!(m.finish_turn(&id));
        m.validate_turn(&id, "snap:1", Some(&scope)).unwrap();
        assert!(m.finish_turn(&id));

        let session = m.get(&id).unwrap();
        assert_eq!(session.status, SessionStatus::Active);
        assert_eq!(session.completed_turns, 2);
    }

    #[test]
    fn turn_validation_rejects_snapshot_or_scope_mismatch() {
        let mut m = SessionManager::new();
        let id = m.create("snap:1", 1);
        let scope = PinnedScope {
            scope_type: ConversationScopeType::Edge,
            id: "rel:a-b".into(),
        };
        assert!(m.pin(&id, scope.scope_type, &scope.id, "snap:1"));

        assert!(m.validate_turn(&id, "snap:2", Some(&scope)).is_err());
        assert!(m.validate_turn(&id, "snap:1", None).is_err());
    }
}
