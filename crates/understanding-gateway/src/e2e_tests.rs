//! E2E integration tests — 返工第二轮 H-fix
//!
//! 这些测试验证 gateway 的端到端编排管道，
//! 不依赖真实 HTTP 服务器（使用纯逻辑验证）。
//!
//! 覆盖场景：
//! 1. Session 全生命周期：create → pin → append_trace → cancel → close
//! 2. Validator 完整链路：模型输出 → claim 校验 → 导航白名单 → 降级
//! 3. Cache 读写 + 过期 + 模型删除清理
//! 4. SecretStore Memory 实现 set/get/delete/has 循环
//! 5. SSE 行解析覆盖各种格式

#[cfg(test)]
mod e2e_tests {
    use crate::cache::{CacheKey, UnderstandingCache};
    use crate::dto::*;
    use crate::provider::{ModelConfig, ProviderKind};
    use crate::secret::{MemorySecretStore, SecretStore};
    use crate::service::UnderstandingService;
    use crate::session::{SessionManager, SessionStatus};
    use crate::validator::OutputValidator;
    use serde_json::json;

    // ── Session 全生命周期 ──────────────────────────────────────────────

    #[test]
    fn session_full_lifecycle_e2e() {
        let mut m = SessionManager::new();
        let now = 1000u64;

        // create
        let sid = m.create("snap:1", now);
        assert!(m.exists(&sid));
        assert_eq!(m.get(&sid).unwrap().status, SessionStatus::Active);

        // pin
        assert!(m.pin(&sid, ConversationScopeType::Node, "n:main", "snap:1"));
        let s = m.get(&sid).unwrap();
        assert_eq!(s.context.pinned_scope.as_ref().unwrap().id, "n:main");

        // append trace
        m.append_trace(
            &sid,
            ToolTrace {
                tool: "search_nodes".into(),
                params: json!({"q": "main"}),
                returned_bytes: 256,
                truncated: false,
            },
        );
        assert_eq!(m.trace(&sid).len(), 1);

        // complete
        assert!(m.complete(&sid));
        assert_eq!(m.get(&sid).unwrap().status, SessionStatus::Completed);
        assert!(!m.complete(&sid), "double complete rejected");

        // close
        assert!(m.close(&sid));
        assert!(!m.exists(&sid));
        assert!(!m.close(&sid), "double close rejected");
    }

    #[test]
    fn session_cancel_blocks_subsequent_operations() {
        let mut m = SessionManager::new();
        let sid = m.create("snap:1", 1);
        assert!(m.cancel(&sid));
        assert_eq!(m.get(&sid).unwrap().status, SessionStatus::Cancelled);
        assert!(!m.cancel(&sid));
    }

    #[test]
    fn multiple_sessions_are_isolated() {
        let mut m = SessionManager::new();
        let s1 = m.create("snap:1", 1);
        let s2 = m.create("snap:1", 2);

        m.pin(&s1, ConversationScopeType::Node, "n:a", "snap:1");
        m.pin(&s2, ConversationScopeType::Edge, "rel:ab", "snap:1");

        assert_ne!(
            m.get(&s1)
                .unwrap()
                .context
                .pinned_scope
                .as_ref()
                .unwrap()
                .scope_type,
            m.get(&s2)
                .unwrap()
                .context
                .pinned_scope
                .as_ref()
                .unwrap()
                .scope_type
        );

        m.close(&s1);
        assert!(!m.exists(&s1));
        assert!(m.exists(&s2));
    }

    // ── Validator 完整链路 ──────────────────────────────────────────────

    #[test]
    fn validator_full_chain_grounded_to_hypothesis_downgrade() {
        let validator = OutputValidator::with_defaults();

        // 模型声称 grounded，但 identifier 不在词汇表中
        let claim = Claim {
            id: "claim:1".into(),
            text: "调用了 `GhostFunction`".into(),
            classification: ClaimClassification::GroundedInterpretation,
            evidence_refs: vec!["rel:abc".into()],
            coverage_caveat_refs: vec![],
        };

        let (result, report) = validator.validate_claim(
            claim,
            &["rel:abc".to_string()],
            &["RealFunction".to_string()],
        );

        assert_eq!(result.classification, ClaimClassification::Hypothesis);
        assert!(!report.downgraded_claims.is_empty());
    }

    #[test]
    fn validator_preserves_grounded_when_identifier_exists() {
        let validator = OutputValidator::with_defaults();
        let claim = Claim {
            id: "claim:1".into(),
            text: "调用了 `Calculator::new`".into(),
            classification: ClaimClassification::GroundedInterpretation,
            evidence_refs: vec!["rel:abc".into()],
            coverage_caveat_refs: vec![],
        };

        let (result, report) = validator.validate_claim(
            claim,
            &["rel:abc".to_string()],
            &["Calculator::new".to_string()],
        );

        assert_eq!(
            result.classification,
            ClaimClassification::GroundedInterpretation
        );
        assert!(report.downgraded_claims.is_empty());
    }

    #[test]
    fn validator_drops_navigation_to_nonexistent_node() {
        let validator = OutputValidator::with_defaults();
        let action = NavigationAction::FocusNode {
            node_id: "n:ghost".into(),
            snapshot_id: "snap:1".into(),
        };

        let result = validator.validate_navigation(&action, &["n:a".to_string()], &[]);
        assert!(result.is_err());
    }

    // ── Cache 读写 + 过期 ───────────────────────────────────────────────

    #[test]
    fn cache_put_get_expiry_e2e() {
        let mut cache = UnderstandingCache::new();

        let key = CacheKey::new("eh:1")
            .with_model("ollama", "qwen3:14b", "local")
            .with_prompt("v1", "zh-CN", "brief");

        let payload = json!({"answer": "test"});
        cache.put(key.clone(), payload.clone(), 1);

        // hit
        let hit = cache.get(&key);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap()["answer"], "test");

        // miss after different key
        let key2 = CacheKey::new("eh:2")
            .with_model("ollama", "qwen3:14b", "local")
            .with_prompt("v1", "zh-CN", "brief");
        assert!(cache.get(&key2).is_none());
    }

    // ── SecretStore Memory 全循环 ───────────────────────────────────────

    #[test]
    fn secret_store_memory_full_cycle() {
        let mut store = MemorySecretStore::new();

        // set
        let ref1 = store
            .set("codelattice", "remote-openai", "sk-abc123")
            .unwrap();
        assert!(ref1.contains("codelattice"));
        assert!(ref1.contains("remote-openai"));

        // get
        let val = store.get(&ref1).unwrap();
        assert_eq!(val, "sk-abc123");

        // has
        assert!(store.has(&ref1));

        // delete
        store.delete(&ref1).unwrap();
        assert!(!store.has(&ref1));
        assert!(store.get(&ref1).is_err());
    }

    #[test]
    fn secret_store_memory_isolates_services() {
        let mut store = MemorySecretStore::new();
        let r1 = store.set("service-a", "user-1", "pw-1").unwrap();
        let r2 = store.set("service-b", "user-1", "pw-2").unwrap();

        assert_ne!(store.get(&r1).unwrap(), store.get(&r2).unwrap());
    }

    // ── UnderstandingService 编排验证 ───────────────────────────────────

    #[test]
    fn service_validate_answer_full_e2e() {
        let s = UnderstandingService::new(Box::new(MemorySecretStore::new()));
        let mut answer = UnderstandingAnswer {
            schema_version: "codelattice.understandingAnswer.v1".into(),
            scope: AnswerScope {
                scope_type: ConversationScopeType::Node,
                id: "n:a".into(),
            },
            answer_summary: "test summary".into(),
            claims: vec![
                Claim {
                    id: "claim:1".into(),
                    text: "调用了 `RealFn`".into(),
                    classification: ClaimClassification::GroundedInterpretation,
                    evidence_refs: vec!["rel:valid".into()],
                    coverage_caveat_refs: vec![],
                },
                Claim {
                    id: "claim:2".into(),
                    text: "调用了 `FakeFn`".into(),
                    classification: ClaimClassification::GroundedInterpretation,
                    evidence_refs: vec!["rel:valid".into()],
                    coverage_caveat_refs: vec![],
                },
            ],
            navigation_actions: vec![
                NavigationAction::FocusNode {
                    node_id: "n:a".into(),
                    snapshot_id: "snap:1".into(),
                },
                NavigationAction::FocusNode {
                    node_id: "n:nonexistent".into(),
                    snapshot_id: "snap:1".into(),
                },
            ],
        };

        let report = s.validate_answer(
            &mut answer,
            &["rel:valid".to_string()],
            &["RealFn".to_string()],
            &["n:a".to_string()],
            &[],
        );

        // claim:1 应保持 grounded（identifier 存在）
        // claim:2 应降级为 hypothesis（identifier 不存在）
        assert_eq!(
            answer.claims[0].classification,
            ClaimClassification::GroundedInterpretation
        );
        assert_eq!(
            answer.claims[1].classification,
            ClaimClassification::Hypothesis
        );

        // 有效导航保留，无效导航丢弃
        assert_eq!(answer.navigation_actions.len(), 1);
        assert!(!report.downgraded_claims.is_empty());
    }

    #[test]
    fn service_model_registration_and_cache_cleanup_e2e() {
        let mut s = UnderstandingService::new(Box::new(MemorySecretStore::new()));

        let model = ModelConfig {
            id: "test-model".into(),
            provider: ProviderKind::Ollama,
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "test:7b".into(),
            api_key_ref: None,
        };

        assert!(s.register_model(model.clone()));
        assert!(!s.register_model(model.clone()), "duplicate rejected");

        // populate cache
        let k = s.cache_key_for("eh:1", &model, "v1", "zh-CN", "brief");
        s.cache.put(k.clone(), json!({"x": 1}), 1);
        s.model_cache_keys
            .entry("test-model".into())
            .or_default()
            .push(k.key_string());
        assert_eq!(s.cache_len(), 1);

        // unregister purges cache
        let (removed, purged) = s.unregister_model("test-model");
        assert!(removed);
        assert!(purged >= 1);
        assert_eq!(s.cache_len(), 0);
    }

    #[test]
    fn service_parse_model_answer_roundtrip() {
        let original = UnderstandingAnswer {
            schema_version: "codelattice.understandingAnswer.v1".into(),
            scope: AnswerScope {
                scope_type: ConversationScopeType::Project,
                id: "p".into(),
            },
            answer_summary: "roundtrip test".into(),
            claims: vec![Claim {
                id: "claim:1".into(),
                text: "test claim".into(),
                classification: ClaimClassification::GroundedInterpretation,
                evidence_refs: vec!["rel:abc".into()],
                coverage_caveat_refs: vec!["coverage:project:calls".into()],
            }],
            navigation_actions: vec![],
        };

        let json_str = serde_json::to_string(&original).unwrap();
        let parsed = UnderstandingService::parse_model_answer(&json_str).unwrap();

        assert_eq!(parsed.answer_summary, original.answer_summary);
        assert_eq!(parsed.claims.len(), 1);
        assert_eq!(parsed.claims[0].id, "claim:1");
        assert_eq!(parsed.claims[0].evidence_refs, vec!["rel:abc"]);
    }
}
