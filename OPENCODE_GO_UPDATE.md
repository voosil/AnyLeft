# OpenCode Go API 更新说明

## 变更概述

OpenCode Go 的 API 接口在 2026 年 9-10 月期间发生了重要变更，AnyLeft 已相应更新以支持新的 API 规范。

## 主要变更

### 1. Base URL 迁移

**旧端点**: `https://opencode.ai/zen/go/v1/usage`  
**新端点**: `https://opencode.ai/inference/go/v1/usage`

- 旧的 `/zen/go/v1/` 路径已返回 **404 Not Found**
- 新的 `/inference/go/v1/` 路径已验证可用（返回 401，需要认证）
- 模型列表端点：`https://opencode.ai/inference/go/v1/models`

### 2. 强制会话头

自 **2026-09-05** 起，所有 OpenCode Go API 请求必须携带 `x-opencode-session` 请求头。

- **缺失后果**: 返回 `400 MissingSessionID`
- **要求**: 值为非空字符串，同一对话的多轮请求应使用相同值
- **AnyLeft 实现**: 使用固定值 `"anyleft"` 作为会话标识

### 3. 支持的协议端点

OpenCode Go 现在提供三种协议端点（同一批模型，不同 wire protocol）：

| 协议 | 端点 | 认证头 |
|------|------|--------|
| OpenAI Chat Completions | `/inference/go/openai/v1/chat/completions` | `Authorization: Bearer` |
| Anthropic Messages | `/inference/go/anthropic/v1/messages` | `x-api-key` |
| OpenAI Responses | `/inference/go/openai/v1/responses` | `Authorization: Bearer` |

**所有端点都必须携带 `x-opencode-session` 头。**

### 4. API Key 格式

- 新格式：`oc_sk_` 前缀
- 旧格式：`sk-go-` 前缀（仍可使用）
- 获取位置：OpenCode 控制台 → Settings → Service Accounts → Add Service Account

## AnyLeft 代码变更

### 文件：`src-tauri/src/providers/opencode_go.rs`

1. **更新 USAGE_URL 常量**
   ```rust
   const USAGE_URL: &str = "https://opencode.ai/inference/go/v1/usage";
   ```

2. **添加 SESSION_ID 常量**
   ```rust
   const SESSION_ID: &str = "anyleft";
   ```

3. **在 HTTP 请求中添加 x-opencode-session 头**
   ```rust
   let response = ctx
       .http
       .get(USAGE_URL)
       .bearer_auth(api_key.trim())
       .header("Accept", "application/json")
       .header("User-Agent", "AnyLeft")
       .header("x-opencode-session", SESSION_ID)  // 新增
       .send()
       .await?;
   ```

### 文件：`README.md`

更新文档中的 API 端点说明，从 `/zen/go/v1/usage` 改为 `/inference/go/v1/usage`。

## 验证结果

- ✅ 所有 8 个单元测试通过
- ✅ Release 构建成功
- ✅ 新端点已验证可用（HTTP 401 响应，需要有效 API Key）
- ✅ 旧端点已确认不可用（HTTP 404）

## 模型列表

新端点返回的可用模型（截至 2026-10-22）：

- deepseek-v4.1-flash, deepseek-v4-pro
- glm-5.1, glm-5.2, glm-5.3, glm-5.3-flash
- grok-4.6, grok-4.7
- gpt-5.6-luna, gpt-6-luna
- claude-haiku-5-5
- hy3, hy4-preview
- kimi-k2.6, kimi-k2.7-code, kimi-k3
- mimo-v2.5, mimo-v2.5-pro, mimo-v2.6-flash, mimo-v2.6-pro
- minimax-m2.5, minimax-m2.7, minimax-m3, minimax-m3.1-flash-free
- qwen3.6-plus, qwen3.7-max, qwen3.7-plus, qwen3.8-max, qwen3.8-flash
- longcat-2.0
- step-5-preview
- omen-alpha
- beam-free, muse-spark-1.2-contributor, muse-spark-1.3-contributor
- gemini-4-argon-contributor

## 参考资料

- [OpenCode Go 官方文档](https://opencode.ai/v2/docs/console/go/)
- [OpenCode Go API 变更讨论](https://github.com/anomalyco/opencode/issues/31084)
- [x-opencode-session 头要求](https://blog.csdn.net/Alpha_0630/article/details/167703051)

## 更新日期

2026-10-22
