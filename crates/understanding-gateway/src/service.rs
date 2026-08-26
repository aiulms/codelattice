//! UnderstandingService —— Gateway 主服务（P0 §5.3）。
//!
//! 组装：SessionManager + ToolDispatcher(BudgetTracker) + OutputValidator +
//! UnderstandingCache + SecretStore。P0-B1/B2 接入真实 provider 流式输出；
//! 本模块先冻结编排契约与可测试的纯逻辑路径。

use std::collections::HashMap;

use crate::cache::{CacheKey, UnderstandingCache};
use crate::dto::{Claim, ClaimClassification, GatewayEvent, NavigationAction, UnderstandingAnswer};
use crate::provider::{ModelAdapter, ModelConfig, ModelPool};
use crate::secret::SecretStore;
use crate::session::SessionManager;
use crate::validator::{OutputValidator, ValidationReport};

/// 从模型输出提取第一个平衡的 JSON 对象（容忍 markdown 围栏与前后缀文本）。
/// 逐字符扫描，正确处理字符串内的花括号与转义。
pub fn extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, ch) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = start + i + ch.len_utf8();
                    return Some(text[start..end].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// 去掉厂商工具调用 XML，避免把 `<longcat_tool_call>` 原文展示给用户。
pub fn strip_tool_call_markup(text: &str) -> String {
    let mut out = text.to_string();
    for (open, close) in [
        ("<longcat_tool_call>", "</longcat_tool_call>"),
        ("<tool_call>", "</tool_call>"),
    ] {
        loop {
            let Some(start) = out.find(open) else { break };
            if let Some(rel) = out[start..].find(close) {
                let end = start + rel + close.len();
                out.replace_range(start..end, "");
            } else {
                out.replace_range(start.., "");
                break;
            }
        }
    }
    out.split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_arg_value(raw: &str) -> serde_json::Value {
    let trimmed = raw.trim();
    serde_json::from_str(trimmed).unwrap_or_else(|_| serde_json::Value::String(trimmed.to_string()))
}

fn take_tagged<'a>(src: &'a str, open: &str, close: &str) -> Option<(&'a str, &'a str)> {
    let start = src.find(open)?;
    let after = &src[start + open.len()..];
    let end = after.find(close)?;
    Some((after[..end].trim(), &after[end + close.len()..]))
}

/// 龙猫等模型会把工具调用写成 XML，而不是我们 prompt 里要求的 JSON。
fn parse_markup_tool_calls(text: &str) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<longcat_tool_call>") {
        let after = &rest[start + "<longcat_tool_call>".len()..];
        // 流式截断时可能没有闭合标签，仍按一次工具调用处理。
        let end = after.find("</longcat_tool_call>").unwrap_or(after.len());
        let body = after[..end].trim();
        rest = if end < after.len() {
            &after[end + "</longcat_tool_call>".len()..]
        } else {
            ""
        };
        let name_end = body.find('<').unwrap_or(body.len());
        let name = body[..name_end].trim();
        if name.is_empty() {
            continue;
        }
        let mut arguments = serde_json::Map::new();
        let mut cursor = &body[name_end..];
        while let Some((key, after_key)) =
            take_tagged(cursor, "<longcat_arg_key>", "</longcat_arg_key>")
        {
            if let Some((value, after_value)) =
                take_tagged(after_key, "<longcat_arg_value>", "</longcat_arg_value>")
            {
                arguments.insert(key.to_string(), parse_arg_value(value));
                cursor = after_value;
            } else {
                break;
            }
        }
        if arguments.is_empty() {
            cursor = &body[name_end..];
            while let Some(p0) = cursor.find("<parameter name=\"") {
                let after_name = &cursor[p0 + "<parameter name=\"".len()..];
                let Some(q) = after_name.find('"') else { break };
                let key = &after_name[..q];
                let after_q = &after_name[q + 1..];
                let Some(gt) = after_q.find('>') else { break };
                let after_gt = &after_q[gt + 1..];
                let Some(close) = after_gt.find("</parameter>") else {
                    break;
                };
                arguments.insert(key.to_string(), parse_arg_value(&after_gt[..close]));
                cursor = &after_gt[close + "</parameter>".len()..];
            }
        }
        out.push(serde_json::json!({"name": name, "arguments": arguments}));
    }
    rest = text;
    while let Some(start) = rest.find("<tool_call>") {
        let after = &rest[start + "<tool_call>".len()..];
        let Some(end) = after.find("</tool_call>") else {
            break;
        };
        let body = after[..end].trim();
        rest = &after[end + "</tool_call>".len()..];
        if let Some(obj) = extract_json_object(body) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&obj) {
                if v.get("name").and_then(serde_json::Value::as_str).is_some() {
                    out.push(v);
                    continue;
                }
            }
        }
        let mut lines = body.lines().map(str::trim).filter(|l| !l.is_empty());
        if let Some(name) = lines.next() {
            let rest_body = lines.collect::<Vec<_>>().join("\n");
            let arguments = extract_json_object(&rest_body)
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            out.push(serde_json::json!({"name": name, "arguments": arguments}));
        }
    }
    out
}

pub struct UnderstandingService {
    pub sessions: SessionManager,
    pub pool: ModelPool,
    pub validator: OutputValidator,
    pub cache: UnderstandingCache,
    pub secret_store: Box<dyn SecretStore>,
    /// 模型归属索引（model id -> cacheKeys），用于模型删除后的关联清理。
    pub(crate) model_cache_keys: HashMap<String, Vec<String>>,
}

impl UnderstandingService {
    pub fn new(secret_store: Box<dyn SecretStore>) -> Self {
        Self {
            sessions: SessionManager::new(),
            pool: ModelPool::new(),
            validator: OutputValidator::with_defaults(),
            cache: UnderstandingCache::new(),
            secret_store,
            model_cache_keys: HashMap::new(),
        }
    }

    pub fn register_model(&mut self, config: ModelConfig) -> bool {
        self.pool.add(config)
    }

    pub fn unregister_model(&mut self, id: &str) -> (bool, usize) {
        let removed = self.pool.remove(id);
        // 模型删除后的关联缓存清理（验收 21：删除模型配置可以清理关联缓存，
        // 但不得触碰事实 Store）。
        let keys = self.model_cache_keys.remove(id).unwrap_or_default();
        let purged = self.cache.remove_by_keys(&keys);
        (removed, purged)
    }

    /// 计算 evidenceHash（缓存键的成分之一）。P0 用 FNV-1a 确定性哈希；
    /// 生产可换 sha256（Python 生成器侧一致）。
    pub fn evidence_hash(payload: &str) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in payload.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        format!("{:016x}", h)
    }

    /// 构建完整 cacheKey（§6.5）。
    pub fn cache_key_for(
        &self,
        evidence: &str,
        model: &ModelConfig,
        prompt_version: &str,
        locale: &str,
        level: &str,
    ) -> CacheKey {
        CacheKey::new(evidence)
            .with_model(
                match model.provider {
                    crate::provider::ProviderKind::Ollama => "ollama",
                    crate::provider::ProviderKind::OpenaiCompatible => "openai-compatible",
                },
                &model.model,
                "local",
            )
            .with_prompt(prompt_version, locale, level)
    }

    /// 校验 answer：claims 逐条校验 + 导航白名单（验收 11/12）。
    pub fn validate_answer(
        &self,
        answer: &mut UnderstandingAnswer,
        valid_refs: &[String],
        identifier_vocabulary: &[String],
        valid_nodes: &[String],
        valid_relations: &[String],
    ) -> ValidationReport {
        let mut report = ValidationReport::default();
        for claim in &mut answer.claims {
            let (out, r) =
                self.validator
                    .validate_claim(claim.clone(), valid_refs, identifier_vocabulary);
            *claim = out;
            report.issues.extend(r.issues);
            report.downgraded_claims.extend(r.downgraded_claims);
        }
        let mut kept = Vec::new();
        for action in answer.navigation_actions.drain(..) {
            if self
                .validator
                .validate_navigation(&action, valid_nodes, valid_relations)
                .is_ok()
            {
                kept.push(action);
            } else {
                report
                    .issues
                    .push(crate::validator::ValidationIssue::UnknownNavigationTarget(
                        "action dropped".to_string(),
                    ));
            }
        }
        answer.navigation_actions = kept;
        report
    }

    /// 构造 Gateway 事件流（骨架；provider 流式接入在 B1）。
    pub fn mock_answer_event(request_id: &str, summary: &str) -> Vec<GatewayEvent> {
        vec![
            GatewayEvent::AnswerChunk {
                text: summary.to_string(),
                request_id: request_id.to_string(),
            },
            GatewayEvent::AnswerComplete {
                request_id: request_id.to_string(),
                answer: UnderstandingAnswer {
                    schema_version: "codelattice.understandingAnswer.v1".to_string(),
                    scope: crate::dto::AnswerScope {
                        scope_type: crate::dto::ConversationScopeType::Project,
                        id: "p".to_string(),
                    },
                    answer_summary: summary.to_string(),
                    claims: vec![Claim {
                        id: "claim:0".to_string(),
                        text: "模型服务未接入（P0-B1）".to_string(),
                        classification: ClaimClassification::Unknown,
                        evidence_refs: vec![],
                        coverage_caveat_refs: vec![],
                    }],
                    navigation_actions: vec![],
                },
            },
        ]
    }

    /// 构造单次解释 prompt（P0-B1：输入冻结 evidence bundle，不允许工具循环）。
    /// evidence 以 JSON 内嵌；要求模型输出合法 JSON（§6.3 UnderstandingAnswer）。
    pub fn explain_prompt(&self, evidence: &serde_json::Value, level: &str) -> String {
        let level_instruction = match level {
            "detailed" => "请给出较详细的解释：包括职责、关键实现点、调用方与被调方关系。",
            _ => "请给出简洁解释：一句话职责 + 2-3 个关键点。",
        };
        format!(
            r#"你是 CodeLattice 软件理解助手。请只基于下面的证据回答，不要声称证据之外的事实。

{level_instruction}

输出必须是合法 JSON，格式：
{{"answerSummary":"一句话总结","claims":[{{"id":"claim:1","text":"断言文本","classification":"grounded_interpretation|hypothesis|unknown","evidenceRefs":["rel:..."]}}],"navigationActions":[{{"type":"focusRelation","relationKey":"rel:..."}}]}}

要求：
1. claims 的 evidenceRefs 只能引用证据 JSON 中真实出现的 rel:/src:/limit: id；
2. 提到的代码标识用反引号包裹（如 `Calculator::new`）；
3. navigationActions 只能引用证据中的 rel: id（focusRelation）或 src: id（focusSource）；
4. 无法从证据得出的内容写为 hypothesis，明确不知道的写 unknown。

证据 JSON：
{evidence}
"#
        )
    }

    /// 解析模型输出 JSON 为 UnderstandingAnswer（P0-B1）。
    /// 解析失败返回 Err（调用方按 hypothesis 降级处理）。
    ///
    /// 模型 prompt 示例不含 snapshotId / schemaVersion / scope（这些由工作台注入），
    /// 缺字段不得判整段回答失败。
    pub fn parse_model_answer(text: &str) -> Result<UnderstandingAnswer, String> {
        let cleaned =
            extract_json_object(text).ok_or_else(|| "no JSON object found".to_string())?;
        let mut v: serde_json::Value =
            serde_json::from_str(&cleaned).map_err(|e| format!("invalid model JSON: {e}"))?;
        if let Some(obj) = v.as_object_mut() {
            obj.entry("schemaVersion".to_string())
                .or_insert(serde_json::json!("codelattice.understandingAnswer.v1"));
            obj.entry("scope".to_string())
                .or_insert(serde_json::json!({"type": "project", "id": "project"}));
            obj.entry("claims".to_string())
                .or_insert(serde_json::json!([]));
            obj.entry("navigationActions".to_string())
                .or_insert(serde_json::json!([]));
            obj.entry("answerSummary".to_string())
                .or_insert(serde_json::json!(""));
            // 模型常把单个 action/claim 写成对象而不是数组。
            if obj
                .get("navigationActions")
                .map(|n| n.is_object())
                .unwrap_or(false)
            {
                let one = obj.remove("navigationActions").unwrap();
                obj.insert("navigationActions".to_string(), serde_json::json!([one]));
            }
            if obj.get("claims").map(|n| n.is_object()).unwrap_or(false) {
                let one = obj.remove("claims").unwrap();
                obj.insert("claims".to_string(), serde_json::json!([one]));
            }
            if let Some(claims) = obj
                .get_mut("claims")
                .and_then(serde_json::Value::as_array_mut)
            {
                for (i, item) in claims.iter_mut().enumerate() {
                    if let Some(o) = item.as_object_mut() {
                        o.entry("id".to_string())
                            .or_insert(serde_json::json!(format!("claim:{}", i + 1)));
                        o.entry("classification".to_string())
                            .or_insert(serde_json::json!("hypothesis"));
                        o.entry("evidenceRefs".to_string())
                            .or_insert(serde_json::json!([]));
                        o.entry("text".to_string()).or_insert(serde_json::json!(""));
                    }
                }
            }
            if let Some(nav) = obj
                .get_mut("navigationActions")
                .and_then(serde_json::Value::as_array_mut)
            {
                for item in nav {
                    if let Some(o) = item.as_object_mut() {
                        o.entry("snapshotId".to_string())
                            .or_insert(serde_json::json!(""));
                    }
                }
            }
        }
        serde_json::from_value(v).map_err(|e| format!("answer schema mismatch: {e}"))
    }

    /// 优先按 UnderstandingAnswer JSON 解析；模型只回了中文/Markdown 时保留原文，
    /// 不要把「有内容但不是 JSON」判成服务不可用。
    pub fn answer_from_model_text(text: &str) -> Result<UnderstandingAnswer, String> {
        match Self::parse_model_answer(text) {
            Ok(answer) => {
                let empty = answer.answer_summary.trim().is_empty() && answer.claims.is_empty();
                if !empty {
                    return Ok(answer);
                }
            }
            Err(_) => {}
        }
        let trimmed = strip_tool_call_markup(text);
        if trimmed.is_empty() {
            return Err("no JSON object found".to_string());
        }
        Ok(UnderstandingAnswer {
            schema_version: "codelattice.understandingAnswer.v1".to_string(),
            scope: crate::dto::AnswerScope {
                scope_type: crate::dto::ConversationScopeType::Project,
                id: "project".to_string(),
            },
            answer_summary: trimmed,
            claims: vec![],
            navigation_actions: vec![],
        })
    }

    /// 把当前 snapshot 填进模型未给出的 navigationActions.snapshotId。
    pub fn bind_answer_snapshot(answer: &mut UnderstandingAnswer, snapshot_id: &str) {
        if snapshot_id.is_empty() {
            return;
        }
        for action in &mut answer.navigation_actions {
            match action {
                NavigationAction::FocusNode {
                    snapshot_id: sid, ..
                }
                | NavigationAction::FocusRelation {
                    snapshot_id: sid, ..
                }
                | NavigationAction::FocusSource {
                    snapshot_id: sid, ..
                } => {
                    if sid.is_empty() {
                        *sid = snapshot_id.to_string();
                    }
                }
            }
        }
    }

    /// 解析模型输出中的工具调用（P0-B2）：支持
    /// `{"toolCalls":[...]}`、原生 OpenAI `tool_calls`，以及龙猫 `<longcat_tool_call>` XML。
    pub fn parse_tool_calls(text: &str) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        if let Some(cleaned) = extract_json_object(text) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cleaned) {
                if let Some(calls) = v.get("toolCalls").and_then(serde_json::Value::as_array) {
                    for c in calls {
                        let name = c
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("");
                        let arguments = c
                            .get("arguments")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        out.push(serde_json::json!({"name": name, "arguments": arguments}));
                    }
                }
                if let Some(calls) = v
                    .pointer("/choices/0/message/tool_calls")
                    .and_then(serde_json::Value::as_array)
                {
                    for c in calls {
                        let name = c
                            .pointer("/function/name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("");
                        let args = c
                            .pointer("/function/arguments")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                            .or_else(|| c.pointer("/function/arguments").cloned())
                            .unwrap_or(serde_json::Value::Null);
                        out.push(serde_json::json!({"name": name, "arguments": args}));
                    }
                }
            }
        }
        out.extend(parse_markup_tool_calls(text));
        out
    }

    /// 构造 Chat 系统 prompt（P0-B2）：工具 schema + 输出 JSON 契约。
    pub fn chat_system_prompt(tools: &serde_json::Value) -> String {
        format!(
            r#"你是 CodeLattice 软件理解助手。你可以迭代调用只读工具取证，然后给出基于证据的回答。

可用工具（只读，参数受限）：
{tools}

输出规则：
1. 用户消息里已经带了「[已讨论的图元素]」「[当前图谱选择]」或「请解释这条 / 请分别解释 / 请解释这个」时：不要调用工具，不要输出任何 XML 或 tool_call 标签，直接根据这些事实用中文作答。
2. 需要取证时只输出 JSON：{{"toolCalls":[{{"name":"<工具名>","arguments":{{...}}}}]}}；不要输出 <longcat_tool_call> 或其他 XML。
3. 完成取证后优先输出最终回答 JSON：
{{"answerSummary":"一句话总结","claims":[{{"id":"claim:1","text":"断言","classification":"grounded_interpretation|hypothesis|unknown","evidenceRefs":["rel:..."]}}],"navigationActions":[{{"type":"focusRelation","relationKey":"rel:..."}}]}}
4. 若无法稳定输出 JSON，直接用中文段落作答，不要只回复无法解析或服务不可用，也不要把工具调用原文写进回答。
5. claims 的 evidenceRefs 只能引用工具返回中真实出现的 rel:/src:/limit: id；
6. 提到的代码标识用反引号包裹；
7. 工具返回的字节数有限，先搜索定位再取上下文，不要一次读全部。
"#
        )
    }

    /// 模型删除后遗留：供测试检查 cache 已清理。
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    pub fn _adapter_hint(&self) -> Option<&dyn ModelAdapter> {
        // 真实 adapter 在 B1 接入；此方法仅为保持 trait 可见
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderKind;
    use crate::secret::MemorySecretStore;
    use serde_json::json;

    fn service() -> UnderstandingService {
        UnderstandingService::new(Box::new(MemorySecretStore::new()))
    }

    #[test]
    fn cache_key_components_are_frozen() {
        let s = service();
        let model = ModelConfig {
            id: "qwen-local".into(),
            provider: ProviderKind::Ollama,
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "qwen3:14b".into(),
            api_key_ref: None,
        };
        let k1 = s.cache_key_for("eh:1", &model, "v1", "zh-CN", "brief");
        let k2 = s.cache_key_for("eh:1", &model, "v1", "zh-CN", "brief");
        assert_eq!(k1.key_string(), k2.key_string());
        let k3 = s.cache_key_for("eh:2", &model, "v1", "zh-CN", "brief");
        assert_ne!(k1.key_string(), k3.key_string());
    }

    #[test]
    fn validate_answer_drops_bad_navigation_and_downgrades_claims() {
        let s = service();
        let mut answer = UnderstandingAnswer {
            schema_version: "codelattice.understandingAnswer.v1".into(),
            scope: crate::dto::AnswerScope {
                scope_type: crate::dto::ConversationScopeType::Node,
                id: "n:a".into(),
            },
            answer_summary: "x".into(),
            claims: vec![Claim {
                id: "claim:1".into(),
                text: "调用了 `GhostFn`".into(),
                classification: ClaimClassification::GroundedInterpretation,
                evidence_refs: vec!["rel:abc".into()],
                coverage_caveat_refs: vec![],
            }],
            navigation_actions: vec![crate::dto::NavigationAction::FocusNode {
                node_id: "n:ghost".into(),
                snapshot_id: "snap:1".into(),
            }],
        };
        let report = s.validate_answer(
            &mut answer,
            &["rel:abc".to_string()],
            &["Calculator".to_string()],
            &["n:a".to_string()],
            &[],
        );
        assert_eq!(
            answer.claims[0].classification,
            ClaimClassification::Hypothesis
        );
        assert!(answer.navigation_actions.is_empty(), "越权导航必须被丢弃");
        assert!(!report.downgraded_claims.is_empty());
    }

    #[test]
    fn unregister_model_purges_its_cache_entries() {
        let mut s = service();
        let model = ModelConfig {
            id: "remote-x".into(),
            provider: ProviderKind::OpenaiCompatible,
            base_url: "https://x/v1".into(),
            model: "m1".into(),
            api_key_ref: Some("keychain:codelattice/remote-x".into()),
        };
        assert!(s.register_model(model.clone()));
        let k = s.cache_key_for("eh:1", &model, "v1", "zh-CN", "brief");
        s.cache.put(k.clone(), json!({"a": 1}), 1);
        s.model_cache_keys
            .entry(model.id.clone())
            .or_default()
            .push(k.key_string());
        assert_eq!(s.cache_len(), 1);
        let (removed, purged) = s.unregister_model("remote-x");
        assert!(removed);
        assert!(purged >= 1, "删除模型必须清理关联缓存");
        assert_eq!(s.cache_len(), 0);
    }

    #[test]
    fn evidence_hash_is_deterministic() {
        assert_eq!(
            UnderstandingService::evidence_hash("abc"),
            UnderstandingService::evidence_hash("abc")
        );
        assert_ne!(
            UnderstandingService::evidence_hash("abc"),
            UnderstandingService::evidence_hash("abd")
        );
    }

    #[test]
    fn explain_prompt_embeds_evidence_and_asks_for_json() {
        let s = service();
        let evidence = json!({
            "selection": {"relationKey": "rel:abc", "sourceId": "n:a", "targetId": "n:b", "kind": "calls"}
        });
        let prompt = s.explain_prompt(&evidence, "brief");
        assert!(prompt.contains("rel:abc"));
        assert!(prompt.contains("grounded_interpretation"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn parse_model_answer_handles_markdown_fenced_json() {
        let text = r#"好的，以下是回答：
```json
{"schemaVersion":"codelattice.understandingAnswer.v1","scope":{"type":"node","id":"n:a"},"answerSummary":"职责说明","claims":[{"id":"claim:1","text":"调用了 `Calculator::new`","classification":"grounded_interpretation","evidenceRefs":["rel:abc"]}],"navigationActions":[{"type":"focusRelation","relationKey":"rel:abc","snapshotId":"snap:1"}]}
```
"#;
        let answer = UnderstandingService::parse_model_answer(text).unwrap();
        assert_eq!(answer.answer_summary, "职责说明");
        assert_eq!(answer.claims[0].text, "调用了 `Calculator::new`");
        assert_eq!(answer.navigation_actions.len(), 1);
    }

    #[test]
    fn parse_model_answer_accepts_prompt_shaped_json_without_snapshot_id() {
        let text = r#"{"answerSummary":"scripts 调用了未知命令","claims":[{"id":"claim:1","text":"external-command-invocation","classification":"hypothesis","evidenceRefs":[]}],"navigationActions":[{"type":"focusRelation","relationKey":"rel:abc"}]}"#;
        let mut answer = UnderstandingService::parse_model_answer(text).unwrap();
        assert_eq!(answer.answer_summary, "scripts 调用了未知命令");
        UnderstandingService::bind_answer_snapshot(&mut answer, "snap:1");
        match &answer.navigation_actions[0] {
            crate::dto::NavigationAction::FocusRelation { snapshot_id, .. } => {
                assert_eq!(snapshot_id, "snap:1");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parse_model_answer_fills_missing_claim_fields() {
        let text = r#"{"answerSummary":"ok","claims":{"text":"hello"},"navigationActions":{"type":"focusRelation","relationKey":"rel:abc"}}"#;
        let answer = UnderstandingService::parse_model_answer(text).unwrap();
        assert_eq!(answer.claims[0].id, "claim:1");
        assert_eq!(answer.claims[0].text, "hello");
        assert_eq!(answer.navigation_actions.len(), 1);
    }

    #[test]
    fn parse_model_answer_rejects_non_json() {
        assert!(UnderstandingService::parse_model_answer("抱歉我无法回答").is_err());
        assert!(UnderstandingService::parse_model_answer("").is_err());
    }

    #[test]
    fn answer_from_model_text_keeps_plain_prose() {
        let text =
            "这条聚合边是 scripts 里已有的外部命令调用向上归并到 (unknown)，不是新推断的依赖。";
        let answer = UnderstandingService::answer_from_model_text(text).unwrap();
        assert_eq!(answer.answer_summary, text);
        assert!(answer.claims.is_empty());
        assert!(UnderstandingService::answer_from_model_text("").is_err());
        assert!(UnderstandingService::answer_from_model_text("   ").is_err());
    }

    #[test]
    fn answer_from_model_text_still_prefers_valid_json() {
        let text = r#"{"answerSummary":"json-ok","claims":[]}"#;
        let answer = UnderstandingService::answer_from_model_text(text).unwrap();
        assert_eq!(answer.answer_summary, "json-ok");
    }

    #[test]
    fn extract_json_object_handles_braces_in_strings() {
        let text = r#"prefix {"a":"{not a brace}","b":1} suffix"#;
        let obj = extract_json_object(text).unwrap();
        assert_eq!(obj, r#"{"a":"{not a brace}","b":1}"#);
    }

    #[test]
    fn parse_tool_calls_supports_both_shapes() {
        // 形态 1：{"toolCalls": [...]}
        let t1 = r#"{"toolCalls":[{"name":"search_nodes","arguments":{"q":"main"}}]}"#;
        let calls = UnderstandingService::parse_tool_calls(t1);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["name"], "search_nodes");
        assert_eq!(calls[0]["arguments"]["q"], "main");

        // 形态 2：原生 OpenAI tool_calls
        let t2 = r#"{"choices":[{"message":{"tool_calls":[{"function":{"name":"get_node_context","arguments":"{\"nodeId\":\"n:a\"}"}}]}}]}"#;
        let calls = UnderstandingService::parse_tool_calls(t2);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["name"], "get_node_context");
        assert_eq!(calls[0]["arguments"]["nodeId"], "n:a");

        // 无工具调用 → 空
        assert!(UnderstandingService::parse_tool_calls("没有调用").is_empty());
    }

    #[test]
    fn parse_tool_calls_reads_longcat_xml() {
        let text = "\
这条边是外部命令调用。
<longcat_tool_call>search_nodes
<longcat_arg_key>query</longcat_arg_key><longcat_arg_value>scripts</longcat_arg_value>
<longcat_arg_key>limit</longcat_arg_key><longcat_arg_value>10</longcat_arg_value>
</longcat_tool_call>";
        let calls = UnderstandingService::parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["name"], "search_nodes");
        assert_eq!(calls[0]["arguments"]["query"], "scripts");
        assert_eq!(calls[0]["arguments"]["limit"], 10);
    }

    #[test]
    fn parse_tool_calls_reads_longcat_get_node_context() {
        let text = "<longcat_tool_call>get_node_context\n<longcat_arg_key>nodeId</longcat_arg_key>\n<longcat_arg_value>shell:file:build.sh</longcat_arg_value>\n</longcat_tool_call>";
        let calls = UnderstandingService::parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["name"], "get_node_context");
        assert_eq!(calls[0]["arguments"]["nodeId"], "shell:file:build.sh");
    }

    #[test]
    fn parse_tool_calls_reads_unclosed_longcat_xml() {
        let text = "<longcat_tool_call>get_node_context\n<longcat_arg_key>nodeId</longcat_arg_key>\n<longcat_arg_value>shell:file:build.sh</longcat_arg_value>";
        let calls = UnderstandingService::parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["name"], "get_node_context");
        assert_eq!(calls[0]["arguments"]["nodeId"], "shell:file:build.sh");
    }

    #[test]
    fn strip_tool_call_markup_hides_vendor_xml_from_the_user() {
        let text = "先给结论。\n<longcat_tool_call>search_nodes\n<longcat_arg_key>query</longcat_arg_key><longcat_arg_value>scripts</longcat_arg_value>\n</longcat_tool_call>\n";
        let cleaned = strip_tool_call_markup(text);
        assert!(cleaned.contains("先给结论"));
        assert!(!cleaned.contains("longcat_tool_call"));
        assert!(!cleaned.contains("search_nodes"));
        let answer = UnderstandingService::answer_from_model_text(text).unwrap();
        assert_eq!(answer.answer_summary, "先给结论。");
    }

    #[test]
    fn chat_system_prompt_embeds_tool_schema() {
        let tools = json!([{"name": "project_summary", "description": "项目概览"}]);
        let prompt = UnderstandingService::chat_system_prompt(&tools);
        assert!(prompt.contains("project_summary"));
        assert!(prompt.contains("不要输出 <longcat_tool_call>"));
        assert!(prompt.contains("toolCalls"));
        assert!(prompt.contains("grounded_interpretation"));
    }
}
