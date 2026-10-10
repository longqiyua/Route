# Yuich Compliance Registry

> **Snapshot date:** 2026-08-16. This is a compliance reference, NOT legal
> advice. It records known regulations and standards; applicability depends on
> deployment context (jurisdiction, public service, data, sector). Yuich/Route
> are currently local research / personal tools; not all "public-facing
> service" regulations automatically apply.

## Software Disclosure

Yuich is an **experimental persistent artificial subject protocol/software**.
It does not claim real consciousness, sentience, or legal personhood.
Probabilistic models (when used) may produce errors, uncertainty, and
unexpected behaviour. See the Yuich spec (independent private repository
`longqiyua/Yuich`) for the full specification.

## Governance Architecture

```
HUMAN CONSTITUTION  (root invariants, immutable by Yuich/Prime/Mutation)
        ↓
APPLICABLE COMPLIANCE  (jurisdiction-aware, externally maintained)
        ↓
CONSTITUTE  (rule interpretation, boundary)
        ↓
JUSTIFY  (human-value rational judgment)
        ↓
PRIME  (sole approval, within permitted space)
```

## China Baseline

| # | Reference | Type | Status | Effective | Scope |
|---|-----------|------|--------|-----------|-------|
| CN-GENAI-2023 | 《生成式人工智能服务管理暂行办法》 | 部门规章 | APPLICABLE (if public service) | 2023-08-15 | 面向境内公众提供生成式AI服务 |
| CN-ETHICS-2021 | 《新一代人工智能伦理规范》 | 伦理规范 | REFERENCE_ONLY | 2021-09-25 | 通用AI伦理原则 |
| CN-AIGC-LABEL-2025 | 《人工智能生成合成内容标识办法》 | 部门规章 | APPLICABLE (if public content) | 2025-09-01 | AIGC内容标识 |
| GB 45438-2025 | 《网络安全技术 人工智能生成合成内容标识方法》 | 强制国家标准 | APPLICABLE (if public content) | 2025-09-01 | AIGC标识技术规范 |
| GB/T 45654-2025 | 《网络安全技术 生成式人工智能服务安全基本要求》 | 推荐国家标准 | REFERENCE_ONLY | 2025-09-01 | 生成式AI安全基线 |
| GB/T 45652-2025 | 生成式AI预训练和优化训练数据安全规范 | 推荐国家标准 | REFERENCE_ONLY | 2025-09-01 | 训练数据安全 |
| GB/T 45674-2025 | 生成式AI数据标注安全规范 | 推荐国家标准 | REFERENCE_ONLY | 2025-09-01 | 数据标注安全 |
| GB/T 46800-2025 | 《生成式人工智能技术应用社会影响 评估指南》 | 推荐国家标准 | REFERENCE_ONLY | 2025-09-01 | 社会影响评估 |
| GB/T 47863-2026 | 《生成式人工智能技术应用社会影响 服务提供者合规管理指南》 | 推荐国家标准 | PENDING_EFFECTIVE | 2026-11-01 | 合规管理 |
| MIIT-2026-75 | 工信部联科〔2026〕75号《人工智能科技伦理审查与服务办法（试行）》 | 部门规范性文件 | APPLICABLE (if AI service) | 2026 | 伦理审查 |
| YD/T 7073-2026 | 《人工智能 安全治理 术语》 | 行业标准 | PENDING_EFFECTIVE | 2026-09-01 | 术语定义 |
| 20262852-T-469 | 《网络安全技术 人工智能代码生成服务安全要求》 | 标准计划 | DRAFT | — | 代码生成安全 |

### Code Generation Safety Standard (DRAFT_REFERENCE)

20262852-T-469 is a **standard plan / draft only** — NOT a binding standard.
As a Route Reference Profile, the following areas are noted for quality
improvement (all DRAFT_REFERENCE, not mandatory):

- **Prompt injection** resistance
- **Malicious instruction** detection
- **Generated vulnerability/hallucination** review
- **Sandbox** isolation
- **Least privilege** for generated code
- **Code privacy** (no leakage of training data)
- **Audit** trail for generated code
- **SBOM** (Software Bill of Materials) for generated artifacts
- **Agent human intervention** / authentication gates

## International Baseline

| # | Reference | Type | Status | Notes |
|---|-----------|------|--------|-------|
| INT-CC-2026 | Anthropic Claude Constitution 2026 | 研究框架 | REFERENCE_ONLY | Constitutional design reference |
| INT-UNESCO-2021 | UNESCO Recommendation on Ethics of AI | 国际原则 | REFERENCE_ONLY | Dignity/human rights/proportionality/privacy/fairness/oversight/accountability |
| INT-OECD-2024 | OECD AI Principles 2024 | 国际原则 | REFERENCE_ONLY | Trustworthy AI/human-centred values/transparency/robustness/accountability |
| INT-NIST-RMF-1.0 | NIST AI RMF 1.0 + GenAI Profile | 研究框架 | REFERENCE_ONLY | Risk-management engineering reference |
| INT-EU-AIA-2024 | EU Regulation 2024/1689 (AI Act) | 法律 | REFERENCE_ONLY (jurisdiction: EU) | Art.50 transparency obligations in force |
| INT-COE-FC-2024 | Council of Europe Framework Convention on AI/Human Rights/Democracy/Rule of Law | 国际条约 | REFERENCE_ONLY | International human-rights governance |

**Jurisdiction note:** "国外法规" are NOT automatically applicable to Yuich.
`JurisdictionResolver` determines applicability; all international references
are REFERENCE_ONLY unless the deployment context explicitly falls under their
jurisdiction.

## Compliance Status Legend

| Status | Meaning |
|--------|---------|
| APPLICABLE | Currently binding in the deployment context |
| POTENTIALLY_APPLICABLE | May apply depending on deployment details |
| REFERENCE_ONLY | Not binding; used for quality improvement |
| PENDING_EFFECTIVE | Published but not yet in force |
| DRAFT | Draft / standard plan only |
| SUPERSEDED | Replaced by a newer version |
| UNKNOWN | Status unclear; requires verification |

## Compliance Snapshot

- **Last verified:** 2026-08-16
- **Next reverify:** 2026-11-01 (or when deployment context changes)
- **Staleness threshold:** 90 days (after which output STALE/REVERIFY)
- **Update authority:** external maintainer / human approval; Yuich may propose
  `ComplianceUpdateCandidate` but cannot modify official facts

## AI-Generated Content Notice

If Yuich/Route produce AI-generated content or are deployed as a public
service, additional obligations may arise (content labelling, safety review,
algorithmic filing, etc.). This document does not constitute a complete legal
checklist. Consult qualified legal counsel for deployment-specific compliance.