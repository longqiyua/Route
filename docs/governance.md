# Governance — Responsibility, Canonical Semantics, Compliance

> **V0.8 governance spec (protocol + notice).** Defines who is responsible
> for what, what "official Route" means versus a fork, and the
> disclaimer/compliance notice for Route protocols and AI outputs. This file
> is the single home for these semantics — they are intentionally **not**
> repeated in every runtime output.

## 1. Responsibility Boundary

| Side | Responsibility |
|------|----------------|
| **Route** | uphold protocol semantics; report uncertainty honestly; preserve provenance; prohibit fabricated Evidence; maintain save/recovery; respect permissions and constraints; use reasonable capability to reduce destruction risk |
| **Operator / User** | review and authorize important operations; production deployment; credential handling; legal / commercial compliance; irreversible external actions; final acceptance of real-world operational risk |

Rules:

- "User responsibility" is **never** a license for Route to silently damage
  a project, fabricate verification, or skip required safety boundaries.
- Consequence-level gating (L0–L4) is defined in
  [maintenance.md §16](maintenance.md#16-consequence-levels): the higher the
  level, the stronger the permission / checkpoint / evidence / recovery /
  confirmation requirements.

## 2. Canonical Semantics

Three things are **separate** and must not be conflated:

1. **LICENSE** — rights to copy / modify / redistribute the code are decided
   by the actual repository license ([LICENSE](../LICENSE)).
2. **Canonical Specification** — official Route terminology, protocol
   invariants, and compatibility semantics are maintained by the canonical
   Route specification / official maintainer.
3. **Brand / Official Status** — whether an implementation may call itself
   official Route.

Compatibility model:

| Tier | Meaning |
|------|---------|
| **ROUTE OFFICIAL** | the canonical official specification |
| **ROUTE COMPATIBLE** | satisfies the defined compatibility contract; may add capabilities |
| **ROUTE DERIVATIVE / FORK** | modifies core semantics; does **not** automatically represent official Route |
| **ROUTE-INSPIRED** | inspired by Route; does not claim compatibility |

Rules:

- A fork/extension may modify — but its modified semantics must not
  automatically claim to represent official Route.
- Vague "final interpretation rights" language must **not** replace the
  LICENSE; however, the official maintainer retains design logic,
  terminology, and version-statement rights.

## 3. Disclaimer / Compliance / Version Notice

> This notice covers the Route protocol specifications, prompt material, and
> AI outputs produced under them. It is placed here once, deliberately, and
> is not duplicated into runtime outputs.

**【免责声明 / Disclaimer】**
本提示词 / Route 相关协议仅供学习、研究与交流使用。鉴于人工智能输出具有
概率性与不可预测性，作者不对由提示词、协议或相关 AI 输出产生内容的准确性、
完整性、可靠性、适用性或特定目的适用性作明示或默示担保；使用者应自行审核。
因使用、部署、发布、修改或依赖本提示词 / 协议及相关输出而产生的直接或间接
后果、损失、争议或法律责任，由使用者依法自行承担，但以下表述不得与实际
LICENSE 或适用法律相冲突。

**【合规要求 / Compliance】**
使用者不得将本提示词 / 协议及相关输出用于违反适用法律法规、侵犯第三方合法
权益、绕过应有权限、安全边界或违背适用人工智能伦理 / 治理要求的用途。

**【版本与官方说明 / Version & Official Status】**
作者 / 官方维护者保留对本提示词、协议规范、设计逻辑、官方术语和版本进行
更新、修改、补充与官方说明的权利；第三方 Fork / 修改版应明确标注其偏离，
不能将自身解释自动视为官方 Route 解释。实际开源权利义务以仓库 LICENSE
为准。

### What the disclaimer is NOT

The disclaimer is **not** a license for Route to skip verification. The
protocol still requires, to the extent possible: real Evidence, risk
warnings, recovery capability, and correct implementation. Fabricated
verification remains a protocol violation even where user responsibility
applies.

## 4. Reference Engine Surface

None required. Governance is documentation + protocol semantics; the engine
enforces pieces it already owns (evidence trust, promotion gates,
confirmation prompts for destructive operations) and adds nothing new for
this section.
