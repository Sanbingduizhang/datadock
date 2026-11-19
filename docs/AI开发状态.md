# DataDock AI 开发状态

> 本文件用于跨会话交接开发上下文。任何新的 AI 会话在开始编码前必须先阅读本文件与相关设计文档。
> 更新时间：2026-11-18

## 一、当前阶段

当前阶段：**阶段 E/F/G/H 的 MySQL 部分完成**：连接、资源树、Monaco 编辑、执行/取消/历史/收藏、手动事务、表浏览（分页/排序/筛选/列显示/列宽/导出）、行增删改（含内联编辑与批量事务）、表结构、设置、SQL 文件、多工作区、窗口记忆。**Redis 适配器已完整：命令控制台 + 键浏览（SCAN + 类型分组 + 值查看器 + TTL）；PostgreSQL MVP 已完整：连接 + SQL 执行 + 资源树 + 表结构 + 表数据浏览 + 导出 + 手动事务（BEGIN/COMMIT/ROLLBACK）**

阶段状态：已完成（编译、Clippy 0 警告、97 项 Rust 单元测试、88 项前端测试、MySQL / Redis / PostgreSQL 端到端集成测试、应用构建通过）；Keychain 往返测试在 AI 沙箱因 EPERM 失败（`--skip=keyring_round_trip` 可过滤），界面与真实 Keychain 需用户在真实环境确认

阶段对应关系：

| 参考文档 | 阶段划分 | 状态 |
| --- | --- | --- |
| `docs/13` 阶段 A | 项目初始化与基础骨架 | 已完成 |
| `docs/13` 阶段 B / `docs/08` 阶段 1 | 主界面、多标签工作区、主题、基础组件 | 已完成 |
| `docs/13` 阶段 C / `docs/08` 阶段 2 | Rust 基础层、SQLite、凭据、设置与工作区持久化 | 已完成 |
| `docs/08` 阶段 0 | MySQL 驱动验证（PoC） | 已完成，见 `docs/技术决策记录.md` TDR-001 |
| `docs/13` 阶段 D | MySQL 连接管理与资源树 | 已完成 |
| `docs/13` 阶段 E | SQL 编辑器与执行引擎 | 基本完成：多语句、多结果集、取消、历史已可用；Monaco 与收藏/SQL 文件待做 |
| `docs/13` 阶段 F | 表数据浏览 | 读取部分完成：分页 + 排序 + 筛选 + 行详情 + 导出 + 表结构 |
| `docs/13` 阶段 G | 表数据编辑 | 已完成：新增行、内联编辑、单行编辑/删除、待保存队列与批量事务执行 |
| `docs/13` 阶段 H | 设置与体验完善 | 完成：设置、SQL 文件、收藏、Monaco、窗口记忆、多工作区；Redis/PG 属后续阶段 |

## 二、已完成内容

### 2.1 工程与工具链

- Git 仓库（`main` 分支），提交信息遵循 `docs/10` 约定。
- `.editorconfig` / `.prettierrc.json` / ESLint 9 Flat Config（TS + Vue）。
- 依赖锁定：`package-lock.json`、`src-tauri/Cargo.lock`。

### 2.2 前端基础（阶段 A）

- 入口 `src/main.ts`、`src/App.vue`；Hash 路由。
- 统一 IPC 封装 `src/api/index.ts`（失败统一转 `AppError`）。
- 类型 `src/types/api.ts`（与 Rust Serde camelCase 严格一致）。
- 错误归一化 `src/utils/errors.ts`（含单测）。
- 设计令牌：`src/styles/{reset,variables,themes,global}.css`。

### 2.3 主界面（阶段 B）

- 菜单栏、工具栏、资源树（递归 + 搜索 + 右键菜单）、多标签工作区、结果面板、状态栏。
- 浅色 / 深色 / 跟随系统主题；弹窗、Toast、空状态、拖拽调整面板尺寸。
- 公共组件：`AppButton`、`AppIconButton`、`AppInput`、`AppMenu`、`AppDropdown`、`AppModal`、`AppTooltip`、`AppEmpty`、`AppToastHost`。

### 2.4 本地存储（阶段 C）

**数据库**

- 位置：`{appDataDir}/datadock.db`（macOS：`~/Library/Application Support/com.datadock.app/`）。
- 配置：WAL、`foreign_keys=ON`、`busy_timeout=5s`、连接池 5、`acquire_timeout=5s`。
- 迁移：`src-tauri/migrations/0001_init.sql`，通过 `sqlx::migrate!` 编译期嵌入，SQLx 自行维护迁移记录表。
- 表：`data_sources`、`query_history`、`sql_favorites`、`app_settings`、`workspace_state` 及文档要求的全部索引。
- 启动即执行迁移 + 写入默认设置（`INSERT OR IGNORE`，不会覆盖用户已修改的值）。
- 退出时（`RunEvent::Exit`）关闭连接池，避免 WAL 未刷盘。

**Repository 层**（`src-tauri/src/repositories/`）

- `settings_repository`：`load_map` / `ensure_defaults` / `upsert_many`（事务）。
- `workspace_repository`：`load` / `save`（`state_key` 唯一，JSON 损坏时按空状态处理并告警）。
- `data_source_repository`：CRUD（不含密码，只存 `credential_key`）。
- `history_repository`：写入 running 记录、结束更新、分页/按连接查询、删除、清空、按上限清理。
- `favorite_repository`：CRUD。

**Service 层**

- `SettingsService`：读取 → 默认值回退；部分更新合并 → 范围校验 → 仅写变化的键。
- `WorkspaceService`：保存前规范化 `version`，限制标签数量（≤100）与标题长度（≤200）、工作区标识长度。

**系统凭据**

- `CredentialStore` 接口 + `KeyringCredentialStore`（`keyring` 3.6.3，macOS 用 `apple-native`，Windows/Linux 各自原生实现）。
- 服务名固定 `com.datadock.app`；数据库只保存 `credential_key`。
- 凭据不存在（`NoEntry`）与访问失败（其他错误）分别处理，绝不退化为明文。

**Command**：`get_all_settings`、`update_settings`、`get_workspace`、`save_workspace`。

### 2.5 前端持久化接入（阶段 C）

- `src/api/settings.ts`、`src/api/workspace.ts`、`src/types/settings.ts`。
- 启动顺序：按系统主题先渲染 → 读取设置应用主题 → 恢复工作区 → 开启自动保存。
- `useWorkspacePersistence`：布局（导航宽度/可见性、结果面板高度/可见性、状态栏）与标签（含 SQL 草稿）变更后 400ms 防抖保存，`pagehide` 时补写一次。
- 恢复的 SQL 草稿标记为「未保存」，避免误以为已写入文件。
- 主题切换立即生效并异步持久化；`general.restoreWorkspace = false` 时不恢复上次工作区。

### 2.6 SQL 执行与表浏览（阶段 E / F）

**执行引擎**

- 多语句：`utils/sql.rs` 状态机切分（引号/反引号/行注释/块注释），同一会话顺序执行，首错停止并保留已完成结果，单次上限 200 条。
- 结果：`StatementResult[]` + `error{index, sqlPreview, message}`；前端按语句分标签展示。
- 取消：前端生成 `taskId` → `cancel_task`；执行会话为每请求独立单连接池，`KILL QUERY` 由主池发出（TDR-003）。
- 行数上限：`SET SESSION sql_select_limit`，不改写用户 SQL。
- 类型映射：BIGINT/UBIGINT、DECIMAL(BigDecimal)、JSON、DATETIME/DATE/TIME、BLOB/BINARY、NULL 分类型返回。
- 历史：`query.history_enabled` / `query.history_limit` 生效，每 20 次执行清理一次。

**表数据与结构**

- `get_table_data`：`COUNT(*)` + `LIMIT/OFFSET`，无排序列时按主键升序，保证翻页稳定；列元数据取自 `information_schema`（空表也能显示表头）。
- 排序：`orderBy` 必须与列元数据匹配（不存在的列直接报错）；筛选：`filter` 为 `WHERE` 之后的表达式，拒绝分号与超长内容，沿用同一 `COUNT(*)` 条件。
- `get_table_schema`：字段、索引、建表 SQL（`SHOW CREATE TABLE`）。
- 前端：`TableTab`（分页、每页 50/100/200/500、刷新、行号连续、列头点击排序、筛选输入框、点击行号看整行详情、导出）、`StructureTab`（字段/索引/DDL，复制带降级方案）。
- 导出：`export_table_data` 由 Rust 分批读取并流式写盘（CSV 带 UTF-8 BOM，JSON 数组），默认上限 100 万行；前端只用 `plugin-dialog` 选择路径，不授予文件系统权限。
- 行编辑：`utils/table-edit.ts` 生成 `UPDATE/DELETE ... WHERE <主键> LIMIT 1`，无主键时禁用编辑；执行前在弹窗展示待执行 SQL，确认后走 `execute_sql`（`sourceType = table`），成功后自动刷新当前页。
- 查询结果导出：`export_query_result` 只接受**单条且返回结果集**的语句（拒绝 UPDATE/INSERT 等），导出时会重新执行该 SQL；上限 10 万行（结果无法分页），超出标记截断。

### 2.7 编辑器、收藏与设置（阶段 H）

- **Monaco 编辑器**：`SqlEditor.vue` 封装，SQL 高亮、行号、查找、括号匹配；⌘/Ctrl+↵ 执行、Esc 取消；主题随应用切换；字号/缩进/换行读设置。Monaco 通过 `defineAsyncComponent` 按需加载（独立 chunk，首屏不受影响）。
- **SQL 收藏**：`FavoriteService` + 4 个命令；查询标签「收藏」按钮命名保存；侧栏「收藏」面板点击回填、可删除。
- **SQL 文件**：`read_sql_file` / `write_sql_file`（UTF-8、5 MB 上限）；工具栏与菜单（⌘O / ⌘S）接入，保存后按文件名更新标签标题并清除未保存标记。
- **侧栏面板**：资源 / 历史 / 收藏三面板共用一列；历史 200 条、搜索过滤、状态点、清空（带确认）。
- **设置界面**：外观（主题即时预览，取消回滚）、编辑器（字号/缩进/换行）、查询（默认返回行数）、数据（默认每页行数）、历史开关、恢复工作区；保存后重新加载设置使编辑器与分页立即生效。

### 2.8 表数据能力补充（阶段 F/G）

- **条件筛选**：`FilterPanel` 字段 + 运算符 + 值，支持 AND/OR、LIKE/IN/IS NULL；可切换「直接输入」表达式；条件经 `buildWhereClause` 生成并用于数据与计数。
- **新增行**：按表结构区分「默认值 / NULL / 指定值」，只把需要写入的列放进 INSERT；执行前展示语句。
- **列宽拖拽**：结果表格固定布局 + 拖拽手柄（最小 60px），切换结果集自动重置。

### 2.9 事务、批量变更与多工作区（阶段 G/H 收尾）

- **手动事务**：查询标签或工具栏开启事务；独立会话执行 `START TRANSACTION`，提交/回滚后关闭。关闭标签、切换工作区、断开连接都会回滚未提交内容。
- **批量变更**：行编辑与删除先进入「待保存」队列（表格行黄色高亮），统一生成多条 `UPDATE/DELETE`，确认后一次执行；失败停在出错语句，已执行部分不会自动回滚。
- **列显示与顺序**：表头「列」菜单可勾选显隐、上下移动列；仅影响当前标签，切换结果集重置。
- **空结果集列头**：后端用 `describe` 补列元数据，`WHERE 1=0` 之类的查询也能显示表头。
- **窗口记忆**：官方 `tauri-plugin-window-state` 保存尺寸/位置/最大化，启动自动恢复。
- **多工作区**：工具栏切换工作区，支持新建、重命名、删除（默认工作区不可删）；标签与布局按工作区保存，切换时先保存当前再恢复目标。

### 2.10 编辑体验收尾（阶段 G）

- **批量事务化**：`execute_batch_sql` 将多条表数据变更包在一个事务中，失败整体回滚；批量弹窗文案同步更新。
- **单元格内联编辑**：双击单元格编辑，回车/失焦提交、Esc 取消；提交前按列类型校验（整数/decimal/日期时间/JSON/enum/varchar 长度/布尔），失配只提示不改值。
- **列偏好持久化**：列顺序、显隐、列宽按「连接|库|表」记入 localStorage，重开表时恢复。

### 2.11 Redis 适配器（阶段 I 起步）

- 连接对话框新增类型选择（MySQL / Redis），Redis 默认端口 6379、用户名可留空、默认库填索引（0-15）。
- 独立的 `RedisManager` + `RedisService`：`redis_execute`（任意命令）、`redis_list_databases`（仅当前库）。
- 查询标签在 Redis 连接下自动切换为命令控制台：`isRedis` 时调用 `redis_execute`，结果复用结果网格与导出。
- 命令行解析支持引号与转义；`HGETALL`/`CONFIG GET` 按 `field`/`value` 两列展示。
- 状态栏引擎名与侧栏文案已按引擎切换；Redis 标签不显示事务按钮。

### 2.12 PostgreSQL 资源树（第 1 项）

- 分发方式：`ExplorerService` 注入 `PgManager`，按 `connection_id` 命中哪个运行时管理器判断引擎（与 `QueryService::execute()` 一致，等价于按 sourceType，无需读取连接配置表）。
- `list_databases` 对 PG 调用 `pg_explorer::list_schemas`，schema 名映射成 `DatabaseInfo`；`list_tables` 对 PG 调用 `pg_explorer::list_tables(pool, schema)`（`databaseName` 即 schema 名）。**命令层与 IPC 契约不变**，MySQL / Redis 路径不受影响。
- 资源树第二层在 PG 下显示为 schema（文案、`Layers` 图标、右键去掉「查看建库语句」、复制名称改为「复制 schema 名称」），其下仍是表 / 视图分组。
- 状态栏引擎名补 PostgreSQL；连接成功提示按 `sourceType` 显示引擎名。
- 配套修复：`ConnectionService::list()` / `to_info()` 补上 PG 运行时连接状态，避免刷新列表后 PG 被判「未连接」导致资源树无法展开。
- 集成测试：`src-tauri/tests/pg_integration.rs`（`--ignored`），真实连本地 PG 18.6，自建临时 schema + 表 + 视图，验证 schema 列表、表/视图分类、空 schema 校验、连接状态与断开回落，结束清理。

### 2.12 PostgreSQL 表数据浏览与导出（第 2/3 项）

- 表数据浏览：`adapters/pg/table.rs` 新增 `fetch_data` + `column_metadata`。复用 `schema::column_info`（`database` 即 schema 名）取列元数据与主键；`COUNT(*)` + `LIMIT/OFFSET` 分页，无排序列时按主键升序保证翻页稳定；筛选 `filter` 为 `WHERE` 后表达式，服务层拒绝分号与超长内容；类型转换复用 `pg_query::convert_row`（integer/decimal/NULL 等齐全）。
- 表数据导出：`adapters/pg/export.rs` 新增 `export_table`，分批 `LIMIT/OFFSET`（1000/批）流式写盘，CSV 带 UTF-8 BOM、JSON 为对象数组，上限 100 万行并返回 truncated。
- 写盘工具抽取：新增 `adapters/export_writer.rs`，把 CSV 表头/记录、JSON 记录写入从 MySQL `export.rs` 抽为共享（RFC 4180 转义、BOM、NULL 语义一致），MySQL 与 PG 导出复用同一套。
- TableService 引擎分发：注入 `PgManager`，`data`/`export` 按 `connection_id` 命中哪个运行时管理器判断引擎（与 `QueryService`/`ExplorerService`/`SchemaService` 一致），命令层与 IPC 契约不变，MySQL 路径未受影响。
- 集成测试：`pg_integration.rs` 扩展覆盖表数据分页（total/pageSize/主键）、非法排序列拒绝、筛选（命中/分号拒绝）、CSV/JSON 导出（BOM/表头/numeric 精度/数组长度）。
- 坑：PG 对 numeric 绑定字符串严格（`column "balance" is of type numeric but expression is of type text`），测试插入改用 `$2::numeric` 显式转换；`convert_row` 中 BigDecimal 输出 `10.5000`（保留 scale）为预期行为。

### 2.13 PostgreSQL 手动事务（第 5 项）

- 会话存储：`ManagedPg` 增加 `transactions` 表（`RwLock<HashMap<transaction_id, PgPool>>`）与 `ssl_mode` 字段，`register_transaction` / `transaction_pool` / `take_transaction` / `close_all`；`PgManager::remove` 断开时调用 `close_all` 关闭主池与全部事务池。
- 执行复用：`execute_pg` 在请求带 `transaction_id` 时改用事务会话执行（`execute_one(pool,...)`），否则仍用长连接池；无 `KILL QUERY` 等价物，不支持取消。
- 事务命令：`begin_transaction` 对 PG 建独立单连接池并执行 `BEGIN`（MySQL 仍 `START TRANSACTION`）；`commit_transaction` / `rollback_transaction` 取出会话执行 `COMMIT` / `ROLLBACK` 后关闭。前端 `!isRedis` 已显示事务按钮，PG 现在真正可用。
- 集成测试：`pg_integration.rs` 新增事务段（引入 `QueryService`）：事务内 INSERT 对主池不可见 → 回滚后消失 → 二次事务 INSERT 提交后保留 → 已提交会话复用报「事务会话不存在」。
- 坑：`PgPool` 不能 `&**` 解引用，需 `&sqlx::PgPool` 显式标注；`ManagedPg::new` 8 参数触发 clippy `too_many_arguments(8/7)`，加 `#[allow]`。

### 2.14 Redis 键浏览

- 后端模型：新增 `models/redis.rs`（`RedisKeyInfo`/`RedisScanRequest`/`RedisScanResponse`/`RedisKeyValueRequest`/`RedisKeyValueResponse`/`RedisKeyValue` 枚举覆盖 string/hash/list/set/zset/stream/none）。
- 后端适配器：新增 `adapters/redis/keys.rs`。`scan_keys`（SCAN 游标 + TYPE + TTL 逐键探测，跳过已删除键）+ `get_key_value`（TYPE 后按类型分发到 GET/HGETALL/LRANGE/SMEMBERS/ZRANGE/XRANGE，`max_items` 限制单键返回行数防止大键卡顿）。`RedisKeyType` 支持 lowercase serde tag。
- Service 扩展：`RedisService` 新增 `scan_keys`（游标从 0 开始，pattern 默认 `*`，count 默认 100）与 `get_key_value`（`max_items` 默认 100）；`commands/redis.rs` 新增 `redis_scan_keys`、`redis_get_key_value` Command 并注册到 `lib.rs`。
- 前端：新增 `types/redis.ts`、`api/redis.ts`；`stores/explorer.ts` 增加 `redisKeys` / `loadRedisKeys`；`stores/workspace.ts` 新增 `openRedisKeyTab`，`WorkspaceTabType` 增加 `redis-key`。
- 资源树：`ConnectionTree.vue` Redis 分支按键类型分组展示（string/hash/list/set/zset/stream 各自图标），显示 TTL（`-1` → 永不过期，`>0` → 人性化时长），双击键名打开 `RedisKeyTab`。
- 键值查看器：新增 `views/workspace/tabs/RedisKeyTab.vue`，按类型渲染值（string → pre 块；hash → 字段/值两列表格；list → 索引/值两列表格；set → 成员列表；zset → 成员/分数表格；stream → id + 字段-值嵌套）。
- 后端 workspace 模型：`WorkspaceTabKind` 新增 `RedisKey`，`TabContextState` 增加 `key_name` 字段，保证 redis-key 标签可持久化与恢复。
- 坑：前端 `ConnectionTree.vue` 双 `<template>` 嵌套缺少闭合标签，`vite build` 报 `Element is missing end tag`，已补 `<template>` 闭合。

## 三、未完成内容

| 内容 | 状态 | 说明 |
| --- | --- | --- |



| 空结果集的列头 | 未开始 | 见 TDR-002 |
| Redis 键空间浏览 | 已完成 | SCAN 匹配扫描 + TYPE 分组 + 值查看器 + TTL；资源树 Redis 下展示键列表，点击开键值查看器 |
| PostgreSQL 适配器 | **MVP 完成** | 连接 + SQL 执行 + 资源树 + 表结构 + 表数据浏览/导出 + 手动事务均已接入；PG 无 `KILL QUERY` 等价物，不支持取消 |
| 编辑前的类型校验（长度/枚举/日期） | 未开始 | 依赖字段元数据，暂由服务端报错兜底 |
| 列偏好跨设备同步 | 未开始 | 目前存 localStorage |
| SQL 文件打开 / 保存 | 未开始 | 已有 dialog 插件，尚缺 Rust 侧读写与菜单接线 |
| 查询结果导出 | 已完成 | 重新执行只读语句后写盘，上限 10 万行 |

| 导出 CSV / JSON | 未开始 | 阶段 H |

## 四、当前项目结构

```
datadock/
├── docs/                                  # 00~13 + AI开发状态.md
├── scripts/gen-icon.py
├── src/
│   ├── api/                               # index / app / settings / workspace / connection / explorer / schema / query / history
│   ├── components/{common,database,layout,query}
│   ├── composables/                       # useToast, useWorkspacePersistence
│   ├── constants/                         # layout, sample-explorer
│   ├── router/, stores/{app,ui,workspace}
│   ├── styles/, types/{api,ui,settings,workspace,connection,database,query,history,table}
│   ├── utils/{errors,workspace-persist}
│   ├── views/workspace/{WorkspaceView.vue,tabs/{Welcome,Query,Table,Structure}Tab.vue}
│   ├── App.vue, env.d.ts, main.ts
├── src-tauri/
│   ├── capabilities/default.json
│   ├── migrations/0001_init.sql
│   ├── icons/
│   ├── src/
│   │   ├── adapters/mysql/{connection,explorer,schema,query,table}.rs
│   │   ├── commands/{app,settings,workspace,connection,explorer,schema,query,history,table}.rs
│   │   ├── core/connection_manager.rs
│   │   ├── credentials/credential_store.rs
│   │   ├── database/sqlite.rs
│   │   ├── errors/{app_error,error_code}.rs
│   │   ├── models/{app,data_source,favorite,history,settings,workspace,connection,database,query,table}.rs
│   │   ├── repositories/{settings,workspace,data_source,history,favorite}_repository.rs
│   │   ├── services/{app,settings,workspace,connection,explorer,schema,query,history,table}_service.rs
│   │   ├── utils/{time,sql}.rs
│   │   ├── lib.rs, logging.rs, main.rs, state.rs
│   ├── build.rs, Cargo.toml, Cargo.lock, tauri.conf.json
├── src-tauri/tests/                       # mysql_poc.rs / mysql_integration.rs / redis_integration.rs / pg_integration.rs
├── tests/frontend/                        # errors / workspace-store / ui-store / workspace-persist
├── eslint.config.js, index.html, package.json
├── tsconfig.json, tsconfig.node.json, vite.config.ts, vitest.config.ts
```

## 五、重要技术决策

| 决策 | 结论 | 原因 |
| --- | --- | --- |
| Tauri | 2.x（2.12.6） | 设计文档指定；3.x 为 alpha |
| 前端构建 | Vite 7.3.8 + TS 5.9 | 稳定线 |
| 本地存储 | sqlx 0.8（`runtime-tokio` + `sqlite` + `migrate` + `macros`） | 与 Tokio 架构一致，自带迁移管理 |
| 迁移方式 | `sqlx::migrate!` 内嵌 SQL | 打包后无需附带脚本；不重复自建 schema_migrations |
| 系统凭据 | keyring 3.6.3（按平台启用原生 feature） | macOS Keychain / Windows Credential Manager / Linux Secret Service |
| 模块可见性 | `lib.rs` 中分层模块为 `pub mod` | 后续阶段复用同一套服务，避免为尚未接线的仓储加 `dead_code` 抑制 |
| 设置存储契约 | 采用 `docs/04` 的扁平键，映射到 `docs/06` 的分组对象 | 两文档键名不一致，见下方「文档差异处理」 |
| 工作区 JSON | 采用前端实际的 `type + context` 形状，`version = 1` | `docs/06` 15.1 的形状缺少 welcome/structure，实际 UI 为超集 |
| 保存策略 | 400ms 防抖 + `pagehide` 补写 | 拖拽面板/输入时不频繁写库 |
| UI 组件库 | 自研（不引入 Naive UI） | 阶段 B 已落地并贴合设计令牌；必要表单控件增量自研 |
| 图标 | Lucide | `docs/02` 14.5 建议 |
| 导出写盘 | Rust 侧写文件，前端只用 `plugin-dialog` 选路径 | 避免授予前端 fs 权限；数据不必经 IPC 往返 |
| Monaco | 0.52.2 + `?worker` + 按需加载 | 0.57 改动语言注册结构且体积更大；按需加载保证首屏 |
| 筛选 | 条件行构建 WHERE，也可直接输入表达式 | Navicat 式交互；两种模式共用同一最终 SQL |
| 执行模型 | 同步 `execute_sql` + 客户端 `taskId` + `cancel_task` | 见 TDR-003，取消不依赖任务队列 |
| 语句会话 | 每请求独立单连接池（max=1） | `SET SESSION` 与 `CONNECTION_ID()` 才可靠 |
| 版本号 | `0.1.0`（三处一致） | V1 发布统一升为 `1.0.0` |

### 文档差异处理（需知悉）

1. **设置键名**：`docs/06` 14.1 使用 `appearance.theme` / `editor.fontSize` / `query.defaultMaxRows` / `general.restoreWorkspace`；`docs/04` 7.4 使用 `editor.font_size` / `data.max_result_rows` 等扁平键。
   → 存储按 `docs/04`，IPC 按 `docs/06`。为对齐新增两个键：`editor.tab_size`、`general.restore_workspace`。映射集中在 `models/settings.rs`。
2. **未暴露的预留键**：`docs/04` 中的 `editor.line_numbers`、`query.history_limit`、`query.default_timeout`、`window.remember_size/position` 已按默认值初始化，但 `docs/06` 的 `AppSettings` 未包含，暂不通过 IPC 暴露。
3. **工作区状态形状**：见上表决策，`docs/06` 15.1 与 `docs/04` 8.4 不一致，采用两者并集。

## 六、已完成测试

| 测试项 | 执行命令 | 结果 |
| --- | --- | --- |
| 前端类型检查 | `npm run type-check` | 通过 |
| 前端 ESLint | `npm run lint` | 通过，0 警告 |
| 前端单元测试 | `npm run test` | 通过，6 文件 / 65 用例 |
| 前端生产构建 | `npm run build` | 通过，主包 278 kB（gzip 94 kB）+ Monaco 按需块 3.24 MB（gzip 828 kB） |
| Rust 格式 / 编译 | `cargo fmt --check`、`cargo check --all-targets` | 通过，0 警告 |
| Rust Clippy | `cargo clippy --all-targets --all-features` | 通过，0 警告 |
| Rust 单元测试 | `cargo test` | 通过，98 用例 |
| MySQL PoC | `cargo test --test mysql_poc -- --ignored` | 7/7 通过（认证、多语句、类型、流式、事务、KILL QUERY、元数据） |
| MySQL 端到端集成 | `cargo test --test mysql_integration -- --ignored` | 通过：测试连接→建配置→连接→16 库→4 表（含视图）→表结构 7 字段/3 索引→查询（DECIMAL 精度/NULL/截断）→INSERT→**多语句 2 结果集**→**中途失败保留已完成结果**→**KILL QUERY 取消**→历史落库→**表数据分页（主键排序/总数/DECIMAL）**→**筛选/排序/非法排序列拒绝**→**导出 CSV/JSON（BOM/表头/null/精度）**→**行 UPDATE/DELETE 语句形态**→**查询结果导出（含拒绝写语句）**→**事务（回滚/提交/复用失效）**→**空结果集列头**→**批量事务回滚**→改配置→重连→断开→删除 |
| Tauri 构建 | `npm run tauri build -- --bundles app` | 通过，`DataDock.app` 19.91 MiB |
| Redis 集成 | `cargo test --test redis_integration -- --ignored` | 通过：测试连接→建配置→连接→PING→SET/GET→HSET/HGETALL（两列）→库列表→断开→删除 |
| PostgreSQL 资源树集成 | `cargo test --test pg_integration -- --ignored` | 通过（本机 PG 18.6）：建临时 schema + 表 + 视图→PG 连接→连接状态统计→schema 列表（排除系统 schema）→表/视图分类→表结构（字段/主键/索引/DDL/视图定义）→**表数据分页（total/主键/非法排序列拒绝）**→**筛选（命中/分号拒绝）**→**CSV/JSON 导出（BOM/表头/numeric 精度/数组长度）**→**手动事务（未提交不可见/回滚消失/提交保留/失效会话报错）**→空 schema 校验→断开回落→删除配置→清理 |

未执行 / 待验证：

- **应用级持久化**：需用户启动应用确认「改主题/拖面板/写 SQL → 重启后恢复」（沙箱内不代启 GUI）。
- DMG 安装包（沙箱 `hdiutil` 受限）。
- 真实 MySQL 连接已由集成测试覆盖；界面点击流程待人工确认。
- Windows / Linux 构建与凭据验证。

## 七、已知问题

1. `npm audit` 13 项 high 均来自开发期传递依赖 `picomatch`，无修复版本，不进运行时产物。
2. 资源树、连接与查询均已接真实 MySQL；示例数据已移除。
3. 编辑器字号/缩进设置已可存储，但暂无设置界面，也未接入编辑器（阶段 E/H）。
3.1 单次最多 200 条语句；结果默认上限 1000 行（可调至 10000），超出标记截断。每次执行会建立一次独立会话（代价见 TDR-003）。
3.2 空结果集列头已支持（`describe`）；取消中被中断的语句结果会被丢弃。
3.4 批量变更已事务化；但若语句里混入 DDL（隐式提交）则不再保证原子性。
3.5 内联编辑不能直接写 NULL（请用行编辑弹窗的「置 NULL」）；列偏好存 localStorage，清缓存会丢失。
3.6 Redis 仅支持命令控制台：不展示键、不写查询历史、不支持取消与手动事务。
3.3 **Keychain 在 AI 沙箱环境中写入被拒（EPERM）**：集成测试改用内存凭据实现验证链路，真实 Keychain 需用户运行应用验证。
4. 未实现多工作区（`workspace_id` 目前固定 `main`）。
4.1 表数据 `COUNT(*)` 在大表上可能较慢（无缓存）；翻页使用 `OFFSET`，深分页会变慢。
5. 窗口尺寸/位置记忆未实现（设置键已预留）。
6. `cargo test` 会真实读写 Keychain（临时条目，测试结束即删除），用于证明凭据能力可用；若系统拒绝访问，该测试会失败而非被跳过。
7. macOS 签名 / 公证未配置。
8. 开发约定：AI 只编写代码并执行检查/构建，不代为启动 GUI 应用。

## 八、待确认事项

1. **MySQL 测试库**：将创建 `tbdock` 库用于 PoC 与联调，且仅操作该库；如不希望创建请提前说明。
2. **Windows / Linux 凭据验证**：当前仅 macOS 验证通过，其他平台需对应环境。
3. **版本号策略**：骨架期 `0.1.0`，V1 发布改 `1.0.0`。

## 九、下一步任务

PostgreSQL 适配器已完成（连接 / SQL 执行 / 资源树 / 表结构 / 表数据浏览 / 导出 / 手动事务），按优先级：

1. **Redis 键浏览**：SCAN + 类型分组、值查看器（Hash/List/Set/ZSet）、TTL 展示。
2. **PostgreSQL 取消**（可选）：PG 无 `KILL QUERY`，可用 `pg_cancel_backend(pid)` 实现，需为事务会话记录 `pg_backend_pid()`。
3. 导出增加 Excel 选项、结果集复制为 INSERT。
4. 连接分组、最近使用、SSH 隧道（`docs/04` 预留）。
