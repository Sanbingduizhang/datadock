# MySQL 数据源适配器详细设计

> 文档路径：`docs/11-MySQL数据源适配器详细设计.md`
> 所属项目：DataDock
> 适用版本：V1.0
> 文档状态：技术设计方案，部分技术选型待验证

## 1. 文档目的

本文档用于定义 DataDock V1.0 中 MySQL 数据源适配器的详细实现方案。

MySQL 是 DataDock V1 的第一个数据库类型。MySQL Adapter 负责连接远程 MySQL 数据库，并向上层业务模块提供统一的数据库操作能力。

主要包括：

1. MySQL 数据库连接管理。
2. 数据库、数据表、视图等元数据查询。
3. SQL 编辑器中的任意 SQL 执行。
4. SQL 执行结果解析与返回。
5. 表数据分页查询。
6. 表数据筛选、排序。
7. 表数据新增、修改、删除。
8. MySQL 数据类型映射。
9. 事务与错误处理。
10. 查询任务取消与资源管理。

本文档是对 `docs/03-技术架构设计.md` 和 `docs/05-核心模块设计.md` 中 Adapter 相关设计的进一步细化。

本文档重点关注 MySQL 适配器本身的技术实现，不重新定义整个应用的产品需求和 UI 规范。

## 2. 设计原则

### 2.1 适配器独立

MySQL 的数据库连接、SQL 执行、元数据查询和数据类型处理全部由 MySQL Adapter 负责。

上层业务模块不直接依赖 MySQL 驱动。

建议调用关系：

```
Vue 前端
   |
   v
Tauri Commands
   |
   v
Application Services
   |
   v
Database Adapter Interface
   |
   v
MySQL Adapter
   |
   v
MySQL Driver
   |
   v
Remote MySQL Server
```

后续增加 PostgreSQL、Redis 时，应分别实现对应适配器。

不能因为 MySQL 使用表结构，就要求 Redis 也必须实现相同的数据库、表、字段操作模型。

### 2.2 安全优先

必须遵循：

- 数据库密码不能明文存储在 SQLite。
- 数据库密码不能保存在前端 localStorage。
- 数据库密码不能出现在普通日志中。
- SQL 参数必须使用参数化查询。
- 数据库标识符必须进行安全引用。
- 不允许直接拼接未经校验的用户输入生成 SQL。
- 不允许将数据库错误信息中的敏感连接信息直接展示给用户。

### 2.3 异步执行

数据库连接和 SQL 执行不能阻塞 UI 线程。

耗时操作必须在 Rust 异步任务中执行。

前端通过 Tauri Command 发起操作，通过事件或任务查询接口获取执行状态和结果。

### 2.4 保留原始数据精度

数据库中的数据可能超过 JavaScript Number 的安全整数范围。

例如：

```
BIGINT
DECIMAL(30, 10)
```

不能直接将这些数据转换成 JavaScript Number。

必须设计统一的数据类型传输结构，避免数据精度丢失。

## 3. MySQL 版本兼容范围

### 3.1 初始兼容建议

V1 建议优先以 MySQL 8.0+ 作为主要开发和测试环境。

其他版本的兼容情况需要通过实际测试确认。

| 数据库类型            | 初始定位                     |
| --------------------- | ---------------------------- |
| MySQL 8.0+            | 优先支持                     |
| MySQL 5.7             | 兼容性待验证                 |
| MariaDB               | 单独验证，不默认视为完全兼容 |
| 其他 MySQL 兼容数据库 | 后续评估                     |

以上属于开发建议，并不代表当前已经实现或验证了这些版本。

### 3.2 版本兼容原则

不能仅根据数据库连接成功，就认定所有功能都兼容。

需要重点验证：

- 数据库元数据查询。
- JSON 类型读取。
- 时间类型读取。
- 字段默认值读取。
- 索引信息读取。
- SHOW CREATE TABLE。
- 不同版本的系统变量和系统表结构。

如果某个数据库版本不支持特定功能，应在对应功能层进行兼容处理。

## 4. 技术选型与驱动验证

### 4.1 驱动选型

Rust MySQL 驱动暂时建议优先验证 SQLx。

SQLx 可以提供异步数据库访问能力，与 Tauri 的异步 Rust 后端架构比较契合。

但不能在未验证功能的情况下，直接认定 SQLx 一定满足 DataDock 的全部需求。

需要建立独立的技术验证程序。

### 4.2 必须验证的能力

| 验证项目 | 验证内容                           |
| -------- | ---------------------------------- |
| 普通查询 | SELECT 查询和结果读取              |
| DML      | INSERT、UPDATE、DELETE             |
| DDL      | CREATE、ALTER、DROP                |
| 任意 SQL | 是否能够执行用户输入的不同类型 SQL |
| 多语句   | 一个请求执行多条 SQL               |
| 多结果集 | 能否读取多个结果集                 |
| 类型映射 | MySQL 各种数据类型的读取           |
| 流式读取 | 大结果集是否能够逐批读取           |
| 连接池   | 多个数据源是否可以独立管理         |
| 事务     | 事务提交、回滚和连接绑定           |
| 查询取消 | 能否真正终止服务端正在执行的查询   |
| 错误处理 | 能否获取 MySQL 错误码和错误信息    |

### 4.3 多语句与多结果集

SQL 编辑器可能执行以下内容：

```
SELECT * FROM users;

SELECT * FROM orders;
```

用户还可能执行：

```
CALL procedure_name();
```

这类场景可能涉及多个结果集。

不能简单地认为一次 SQL 请求只会返回一个 SELECT 结果。

驱动如果不支持完整的多语句或多结果集处理，需要考虑：

1. 将用户执行的 SQL 按语句拆分。
2. 按顺序执行。
3. 分别收集每条语句的执行结果。
4. 将多个结果集返回前端。

但 SQL 拆分必须正确处理字符串、注释、存储过程定义和分号。

不能简单使用字符串分割：

```
sql.split(';')
```

因为以下 SQL 中的分号不代表语句结束：

```
SELECT 'hello;world';

-- 这是一条注释，里面包含 ;
SELECT * FROM users;
```

另外，存储过程、触发器等 SQL 可能涉及更复杂的分隔符规则。

如果 SQLx 或所选驱动无法满足完整的客户端需求，应对比 `mysql_async`、`mysql` 等方案，必要时评估更底层的协议实现。

最终驱动选型需要通过 PoC 验证，而不是仅凭 API 是否方便决定。

### 4.4 技术验证要求

在正式开发 SQL 编辑器之前，先实现独立的 MySQL 驱动验证程序。

验证程序至少完成：

- 创建连接。
- 执行 SELECT。
- 执行 INSERT。
- 执行 UPDATE。
- 执行 DELETE。
- 执行 DDL。
- 读取不同字段类型。
- 读取多结果集。
- 执行事务。
- 模拟网络中断。
- 测试查询取消。
- 测试大结果集读取。

验证结果记录在项目技术决策文档中。

## 5. MySQL 连接管理

### 5.1 连接管理目标

DataDock 允许用户配置多个 MySQL 数据源。

例如：

```
本地开发数据库
测试服务器
预发布数据库
生产数据库
```

不同数据源之间必须相互隔离。

不能因为一个数据库连接断开，导致其他数据源也受到影响。

### 5.2 连接池策略

建议每个数据源拥有独立的连接池或独立连接管理实例。

```
ConnectionManager
    |
    +---- DataSource A
    |        |
    |        +---- MySQL Pool A
    |
    +---- DataSource B
    |        |
    |        +---- MySQL Pool B
    |
    +---- DataSource C
             |
             +---- MySQL Pool C
```

连接池配置建议：

| 配置项           | 建议初始值 | 说明               |
| ---------------- | ---------- | ------------------ |
| 最大连接数       | 5          | 每个数据源独立配置 |
| 最小连接数       | 0          | 按需建立连接       |
| 连接超时         | 10 秒      | 建立连接的等待时间 |
| 空闲超时         | 300 秒     | 空闲连接回收       |
| 连接最大生命周期 | 1800 秒    | 防止连接长期不更新 |

这些数值属于初始建议，应根据实际驱动能力和用户使用场景调整。

不要为了提高并发能力，默认给每个数据源创建大量连接。

桌面数据库管理工具的连接数量通常不需要特别大。

### 5.3 连接配置结构

建议 Rust 内部使用类似以下结构：

```
pub struct MySqlConnectionConfig {
    pub data_source_id: String,

    pub host: String,
    pub port: u16,

    pub username: String,
    pub database: Option<String>,

    pub charset: Option<String>,
    pub ssl_mode: Option<String>,

    pub connect_timeout_secs: u64,

    pub max_connections: u32,

    pub credential_key: String,
}
```

注意：

`credential_key` 用于从系统凭据管理服务中获取密码。

密码不应该出现在该结构的持久化版本中。

如果驱动需要密码，应在连接创建过程中临时获取，并避免将其写入日志或调试输出。

### 5.4 连接密码管理

推荐流程：

```
用户填写连接信息
       |
       v
前端提交连接配置
       |
       v
Rust 接收配置
       |
       v
调用系统凭据管理服务
       |
       v
安全保存数据库密码
       |
       v
SQLite 保存 credential_key
       |
       v
建立 MySQL 连接
```

连接时：

```
读取 data_sources
       |
       v
根据 credential_key 读取密码
       |
       v
构建临时连接参数
       |
       v
创建连接池
```

系统凭据存储建议：

- macOS：Keychain。
- Windows：Credential Manager。
- Linux：Secret Service 等系统凭据服务。

如果当前系统没有可用的凭据服务，应明确提示用户，而不是静默退化为明文存储。

### 5.5 连接状态

建议定义：

```
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
    Closing,
}
```

前端可以显示：

| 状态         | 显示含义     |
| ------------ | ------------ |
| Disconnected | 未连接       |
| Connecting   | 正在连接     |
| Connected    | 已连接       |
| Reconnecting | 正在重新连接 |
| Failed       | 连接失败     |
| Closing      | 正在关闭     |

连接状态必须由 Rust 后端维护，前端不能自行推测数据库连接状态。

### 5.6 连接生命周期

连接生命周期：

```
创建数据源
    |
    v
保存连接配置
    |
    v
测试连接
    |
    v
打开数据源
    |
    v
创建连接池
    |
    v
查询数据库
    |
    v
关闭数据源
    |
    v
释放连接池
```

主要操作：

| 操作      | 说明           |
| --------- | -------------- |
| create    | 创建连接配置   |
| test      | 测试数据库连接 |
| open      | 打开数据源     |
| close     | 关闭数据源     |
| reconnect | 重新建立连接   |
| delete    | 删除数据源配置 |

### 5.7 断线与重连

网络断开可能导致：

- 连接池中的连接失效。
- 当前 SQL 执行失败。
- 正在进行的表数据读取中断。
- 数据库资源树无法刷新。

处理原则：

1. 识别连接异常。
2. 更新数据源状态。
3. 将错误返回给前端。
4. 根据驱动连接池机制清理失效连接。
5. 用户主动重连，或按照明确的重连策略尝试恢复。

需要特别注意：

不能自动重试所有 SQL。

例如：

```
INSERT INTO orders (name) VALUES ('test');
```

如果数据库已经执行成功，但客户端在接收结果之前断线，那么客户端可能无法判断是否已经提交。

此时自动重试可能产生重复数据。

因此：

- 普通只读查询可以根据明确的重试策略进行有限重试。
- 写操作默认不自动重试。
- 事务执行失败后不能直接假设事务未提交。
- 无法确认执行结果时，应提示用户检查数据库状态。

## 6. MySQL Adapter 接口设计

### 6.1 接口划分

不建议将所有数据库操作设计成一个庞大的接口。

可以按照能力划分：

```
MySqlAdapter
    |
    +---- ConnectionCapability
    |
    +---- MetadataCapability
    |
    +---- QueryCapability
    |
    +---- TableDataCapability
    |
    +---- TransactionCapability
```

### 6.2 连接能力

```
pub trait ConnectionCapability {
    async fn connect(
        &self,
        config: MySqlConnectionConfig,
    ) -> Result<(), AdapterError>;

    async fn test_connection(
        &self,
        config: MySqlConnectionConfig,
    ) -> Result<ConnectionTestResult, AdapterError>;

    async fn disconnect(
        &self,
        data_source_id: &str,
    ) -> Result<(), AdapterError>;

    async fn get_connection_status(
        &self,
        data_source_id: &str,
    ) -> Result<ConnectionStatus, AdapterError>;
}
```

### 6.3 元数据能力

```
pub trait MetadataCapability {
    async fn list_databases(
        &self,
        data_source_id: &str,
    ) -> Result<Vec<DatabaseInfo>, AdapterError>;

    async fn list_tables(
        &self,
        data_source_id: &str,
        database: &str,
    ) -> Result<Vec<TableInfo>, AdapterError>;

    async fn list_views(
        &self,
        data_source_id: &str,
        database: &str,
    ) -> Result<Vec<ViewInfo>, AdapterError>;

    async fn get_table_schema(
        &self,
        data_source_id: &str,
        database: &str,
        table: &str,
    ) -> Result<TableSchema, AdapterError>;

    async fn get_create_table_sql(
        &self,
        data_source_id: &str,
        database: &str,
        table: &str,
    ) -> Result<String, AdapterError>;
}
```

### 6.4 SQL 查询能力

```
pub trait QueryCapability {
    async fn execute_query(
        &self,
        request: ExecuteQueryRequest,
    ) -> Result<QueryExecutionResult, AdapterError>;
}
```

SQL 执行结果不应只有一个简单的字符串。

需要区分：

- 查询结果集。
- 影响行数。
- DDL 执行状态。
- 多语句执行结果。
- 错误信息。
- 执行耗时。

### 6.5 表数据能力

```
pub trait TableDataCapability {
    async fn fetch_table_data(
        &self,
        request: FetchTableDataRequest,
    ) -> Result<TableDataResult, AdapterError>;

    async fn insert_row(
        &self,
        request: InsertRowRequest,
    ) -> Result<MutationResult, AdapterError>;

    async fn update_row(
        &self,
        request: UpdateRowRequest,
    ) -> Result<MutationResult, AdapterError>;

    async fn delete_row(
        &self,
        request: DeleteRowRequest,
    ) -> Result<MutationResult, AdapterError>;
}
```

以上代码是接口示意，实际实现时需要根据 Rust async trait 的具体实现方式调整。

## 7. 元数据查询设计

### 7.1 数据库列表

使用：

```
SHOW DATABASES;
```

也可以使用：

```
SELECT
    SCHEMA_NAME
FROM information_schema.SCHEMATA
ORDER BY SCHEMA_NAME;
```

建议优先通过 `information_schema` 统一处理需要的元数据。

但是部分场景使用 SHOW 命令更直接。

### 7.2 数据表列表

```
SELECT
    TABLE_NAME,
    TABLE_TYPE,
    ENGINE,
    TABLE_ROWS,
    TABLE_COMMENT
FROM information_schema.TABLES
WHERE TABLE_SCHEMA = ?
ORDER BY TABLE_NAME;
```

字段说明：

| 字段          | 说明               |
| ------------- | ------------------ |
| TABLE_NAME    | 表名               |
| TABLE_TYPE    | BASE TABLE 或 VIEW |
| ENGINE        | 存储引擎           |
| TABLE_ROWS    | 估算行数           |
| TABLE_COMMENT | 表注释             |

注意：

InnoDB 的 TABLE_ROWS 通常是估算值，不能将其直接作为精确数据总量。

### 7.3 字段结构查询

```
SELECT
    COLUMN_NAME,
    ORDINAL_POSITION,
    COLUMN_DEFAULT,
    IS_NULLABLE,
    DATA_TYPE,
    COLUMN_TYPE,
    CHARACTER_MAXIMUM_LENGTH,
    NUMERIC_PRECISION,
    NUMERIC_SCALE,
    COLUMN_KEY,
    EXTRA,
    COLUMN_COMMENT
FROM information_schema.COLUMNS
WHERE TABLE_SCHEMA = ?
  AND TABLE_NAME = ?
ORDER BY ORDINAL_POSITION;
```

需要解析：

- 字段名称。
- 字段顺序。
- 字段类型。
- 完整字段类型。
- 是否允许 NULL。
- 默认值。
- 是否主键。
- 是否自增。
- 字段注释。
- 精度和长度。

### 7.4 索引查询

```
SELECT
    INDEX_NAME,
    NON_UNIQUE,
    SEQ_IN_INDEX,
    COLUMN_NAME,
    INDEX_TYPE,
    CARDINALITY
FROM information_schema.STATISTICS
WHERE TABLE_SCHEMA = ?
  AND TABLE_NAME = ?
ORDER BY INDEX_NAME, SEQ_IN_INDEX;
```

需要支持：

- 普通索引。
- 唯一索引。
- 主键索引。
- 复合索引。

复合索引不能只保存一个字段。

例如：

```
UNIQUE KEY uk_user_tenant (user_id, tenant_id)
```

需要保存完整的字段组合和顺序。

### 7.5 建表 SQL

建议使用：

```
SHOW CREATE TABLE `database_name`.`table_name`;
```

数据库名和表名必须经过安全引用。

不能将用户输入直接拼接到 SQL 中。

`SHOW CREATE TABLE` 的返回结构需要根据驱动结果进行解析。

## 8. MySQL 数据类型映射

### 8.1 设计目标

MySQL 数据类型和前端 JavaScript 类型并不是一一对应关系。

例如：

MySQL：

```
BIGINT
DECIMAL(30, 10)
DATETIME
JSON
BLOB
```

不能全部转换为 JavaScript Number 或 String 后就不再区分类型。

DataDock 需要保留足够的类型信息，以支持正确展示、编辑和数据回写。

### 8.2 统一字段定义

建议：

```
interface ColumnDefinition {
    name: string;

    dataType: string;

    columnType: string;

    nullable: boolean;

    defaultValue: string | null;

    primaryKey: boolean;

    autoIncrement: boolean;

    unsigned: boolean;

    comment?: string;

    precision?: number;

    scale?: number;

    maxLength?: number;
}
```

其中：

- `dataType` 表示基础类型。
- `columnType` 表示 MySQL 完整类型定义。
- `unsigned` 表示是否为无符号类型。
- `precision` 和 `scale` 用于数值精度处理。

### 8.3 统一单元格数据结构

建议：

```
interface CellValue {
    type: string;

    value: string | number | boolean | null;

    isNull: boolean;

    displayValue?: string;

    rawValue?: string;
}
```

具体结构可以根据 Rust 序列化实现调整。

对于需要保留精度的数据，`value` 必须使用字符串。

### 8.4 类型映射参考

| MySQL 类型 | 建议传输类型  | 说明                     |
| ---------- | ------------- | ------------------------ |
| TINYINT    | number/string | 根据范围和语义处理       |
| SMALLINT   | number/string | 根据范围处理             |
| MEDIUMINT  | number/string | 根据范围处理             |
| INT        | number/string | 根据是否 unsigned 处理   |
| BIGINT     | string        | 避免 JavaScript 精度丢失 |
| FLOAT      | number/string | 注意浮点精度             |
| DOUBLE     | number/string | 注意浮点精度             |
| DECIMAL    | string        | 必须保留精度             |
| CHAR       | string        | 普通文本                 |
| VARCHAR    | string        | 普通文本                 |
| TEXT       | string        | 长文本                   |
| TINYTEXT   | string        | 长文本                   |
| MEDIUMTEXT | string        | 长文本                   |
| LONGTEXT   | string        | 长文本                   |
| BINARY     | binary        | 二进制                   |
| VARBINARY  | binary        | 二进制                   |
| BLOB       | binary        | 二进制                   |
| TINYBLOB   | binary        | 二进制                   |
| MEDIUMBLOB | binary        | 二进制                   |
| LONGBLOB   | binary        | 二进制                   |
| DATE       | string        | 日期                     |
| TIME       | string        | 时间                     |
| DATETIME   | string        | 日期时间                 |
| TIMESTAMP  | string        | 时间戳类型               |
| YEAR       | number/string | 年份                     |
| JSON       | JSON/string   | 保留原始文本或结构       |
| ENUM       | string        | 枚举                     |
| SET        | string        | 集合                     |
| BIT        | binary/string | 位数据                   |
| GEOMETRY   | binary        | 空间数据                 |

这里的 `binary` 是逻辑类型，具体 IPC 编码可使用 Base64 或其他经过验证的二进制传输方式。

### 8.5 BIGINT 精度

JavaScript Number 的安全整数范围有限。

如果将超出安全范围的 BIGINT 转成 Number，就可能丢失精度。

例如：

```
SELECT 9007199254740993;
```

不能简单返回：

```
{
  "value": 9007199254740992
}
```

建议返回：

```
{
  "type": "bigint",
  "value": "9007199254740993",
  "isNull": false
}
```

### 8.6 DECIMAL 精度

DECIMAL 应始终保留字符串表示。

例如：

```
DECIMAL(30, 10)
```

原始值：

```
12345678901234567890.1234567890
```

不能转换为普通浮点数。

否则会出现精度丢失。

### 8.7 NULL 处理

必须区分：

```
NULL
```

和：

```
空字符串 ""
```

建议：

```
{
  "type": "varchar",
  "value": null,
  "isNull": true
}
```

空字符串：

```
{
  "type": "varchar",
  "value": "",
  "isNull": false
}
```

### 8.8 日期时间处理

MySQL 日期时间类型需要特别注意时区。

- DATE：只包含日期。
- TIME：时间或时间间隔。
- DATETIME：不携带时区语义。
- TIMESTAMP：涉及数据库会话时区转换。

建议保留驱动读取到的原始日期时间信息，并明确数据库会话时区处理策略。

不要默认将所有 DATETIME 转换为 UTC 时间。

对于用户输入和展示，应避免因本地时区转换导致时间偏移。

### 8.9 JSON 类型

MySQL JSON 字段建议：

- 保留原始 JSON 内容。
- 前端可以提供格式化展示。
- 允许用户编辑 JSON。
- 提交前进行 JSON 格式校验。
- 不要因为前端解析失败就直接修改原始数据。

### 8.10 二进制类型

BLOB、BINARY 等字段不应该直接当作普通文本展示。

建议：

- 小型二进制字段显示为十六进制或 Base64 摘要。
- 大型二进制字段显示类型、长度和预览信息。
- 后续可增加二进制导出能力。

## 9. SQL 执行设计

### 9.1 执行场景

SQL 编辑器需要支持：

1. 执行当前语句。
2. 执行选中 SQL。
3. 执行全部 SQL。
4. 执行 SELECT。
5. 执行 INSERT。
6. 执行 UPDATE。
7. 执行 DELETE。
8. 执行 DDL。
9. 执行事务语句。
10. 执行存储过程等数据库支持的语句。

### 9.2 执行请求

建议：

```
interface ExecuteQueryRequest {
    taskId: string;

    dataSourceId: string;

    database?: string;

    sql: string;

    parameters?: unknown[];

    executionMode: "current" | "selection" | "all";

    resultLimit?: number;
}
```

`taskId` 用于关联一次 SQL 执行任务。

`resultLimit` 主要用于控制结果展示数量。

### 9.3 执行结果

建议：

```
interface QueryExecutionResult {
    taskId: string;

    status: "success" | "failed" | "cancelled";

    statements: StatementResult[];

    totalDurationMs: number;
}
```

语句结果：

```
interface StatementResult {
    statementIndex: number;

    statementType: string;

    columns?: ColumnDefinition[];

    rows?: CellValue[][];

    affectedRows?: string;

    lastInsertId?: string;

    durationMs: number;

    error?: QueryError;
}
```

`affectedRows` 和 `lastInsertId` 建议使用字符串，以免超出安全整数范围。

### 9.4 SQL 类型判断

可以根据 SQL 解析结果进行初步分类：

```
SELECT
INSERT
UPDATE
DELETE
CREATE
ALTER
DROP
TRUNCATE
CALL
OTHER
```

但是不能仅凭 SQL 开头判断最终执行结果。

例如：

```
WITH temp AS (
    SELECT * FROM users
)
SELECT * FROM temp;
```

以及存储过程、CTE 等语法都可能让简单的字符串判断失效。

SQL 分类主要用于 UI 展示和执行策略，最终仍应以数据库实际返回结果为准。

### 9.5 结果行数限制

普通 SQL 查询可以提供默认结果展示上限。

例如：

```
默认展示上限：10000 行
```

但必须注意：

不能为了限制结果数量，自动给所有用户 SQL 添加 LIMIT。

例如：

```
UPDATE users SET status = 1;
```

不能被修改成其他 SQL。

正确做法：

- 对用户原始 SQL 保持语义。
- 在结果读取层控制内存和展示数量。
- 如果结果超过展示上限，提示用户。
- 允许后续加载更多结果（前提是执行模型支持）。
- 不允许将未读取的数据伪装成完整结果。

### 9.6 大结果集处理

不能一次性将几十万行结果全部加载到内存后再交给前端。

建议使用流式读取：

```
数据库
  |
  v
Rust 驱动
  |
  v
结果读取缓冲区
  |
  v
批量序列化
  |
  v
Tauri IPC
  |
  v
前端结果表格
```

建议初始每批返回 100～500 行，具体数值需要性能测试后确定。

### 9.7 多语句执行

多语句执行时，每条语句需要独立记录执行结果。

例如：

```
SELECT * FROM users;

UPDATE users
SET status = 1
WHERE id = 1;

SELECT * FROM users WHERE id = 1;
```

结果应该能够体现：

```
语句 1：查询成功，返回 100 行
语句 2：执行成功，影响 1 行
语句 3：查询成功，返回 1 行
```

不能只返回最后一条 SQL 的执行结果。

### 9.8 SQL 执行错误

建议统一错误结构：

```
interface QueryError {
    code?: string;

    message: string;

    sqlState?: string;

    statementIndex?: number;

    position?: number;
}
```

数据库错误应尽可能保留：

- MySQL 错误码。
- SQLSTATE。
- 错误描述。
- 对应 SQL 语句序号。

不要在错误中返回数据库密码、完整凭据或不必要的连接信息。

## 10. 查询任务取消

### 10.1 设计目标

用户执行 SQL 后，可能需要点击停止按钮。

例如：

```
SELECT * FROM large_table;
```

或者执行耗时较长的统计 SQL。

DataDock 需要提供查询取消能力。

### 10.2 取消接口

建议：

```
async fn cancel_query(
    task_id: &str,
) -> Result<(), AdapterError>;
```

### 10.3 取消状态

```
Pending
Running
Cancelling
Cancelled
Success
Failed
```

### 10.4 取消实现注意事项

必须区分：

1. 取消前端等待。
2. 取消 Rust 异步任务。
3. 关闭当前数据库连接。
4. 真正终止 MySQL 服务端正在执行的 SQL。

这几种操作不是同一件事。

如果只是取消 Rust Future，不代表 MySQL 服务端已经停止执行。

如果只是关闭连接，也不能在没有验证的情况下声称服务端查询已经立即终止。

### 10.5 服务端取消

如果驱动不支持直接取消查询，可以评估：

- 通过独立控制连接执行 KILL QUERY。
- 使用 MySQL 连接 ID 定位当前查询。
- 使用驱动提供的取消机制。
- 必要时终止专用执行连接。

使用 KILL QUERY 需要注意：

- 执行账户必须具备相应权限。
- 控制连接必须独立管理。
- 必须确认目标连接属于当前 DataDock 数据源。
- 不能错误终止其他用户的数据库操作。
- 取消失败时应明确反馈。

只有在服务端确认查询已终止，或者驱动提供了可信的终止结果时，才能将状态明确标记为服务端已取消。

## 11. 数据浏览器设计

### 11.1 设计目标

数据浏览器用于查看和操作 MySQL 数据表。

主要功能：

- 数据查询。
- 分页。
- 排序。
- 筛选。
- 新增。
- 修改。
- 删除。
- 数据刷新。

数据浏览器生成的 SQL 和 SQL 编辑器执行的用户 SQL 必须区分。

数据浏览器可以根据用户操作生成 SQL，但不能修改用户在 SQL 编辑器中输入的原始 SQL。

### 11.2 查询请求

建议：

```
interface FetchTableDataRequest {
    dataSourceId: string;

    database: string;

    table: string;

    page: number;

    pageSize: number;

    sort?: SortCondition[];

    filters?: FilterCondition[];
}
```

排序：

```
interface SortCondition {
    column: string;

    direction: "ASC" | "DESC";
}
```

筛选：

```
interface FilterCondition {
    column: string;

    operator:
        | "="
        | "!="
        | ">"
        | ">="
        | "<"
        | "<="
        | "LIKE"
        | "IS NULL"
        | "IS NOT NULL";

    value?: unknown;
}
```

### 11.3 标识符安全引用

表名、字段名、数据库名不能使用普通参数占位符代替。

例如：

```
SELECT * FROM ?
```

不能用来安全替换表名。

必须采用经过校验的标识符引用方式。

MySQL 标识符通常使用反引号：

```
SELECT *
FROM `users`;
```

如果标识符中本身存在反引号，需要正确转义。

建议统一实现：

```
fn quote_mysql_identifier(
    identifier: &str,
) -> String;
```

要求：

- 正确转义反引号。
- 不允许将输入当作任意 SQL 片段。
- 数据库名、表名、字段名分别处理。
- 不使用简单字符串替换作为安全策略。

### 11.4 参数化查询

筛选条件中的值必须使用参数化查询。

例如：

```
SELECT *
FROM `users`
WHERE `username` = ?
  AND `status` = ?;
```

参数：

```
["admin", 1]
```

不能这样拼接：

```
let sql = format!(
    "SELECT * FROM users WHERE username = '{}'",
    username
);
```

### 11.5 排序安全

排序字段不能直接接受任意 SQL 内容。

例如：

```
column = id DESC; DROP TABLE users;
```

必须拒绝这种输入。

正确处理：

1. 从当前表结构中获取合法字段。
2. 校验排序字段是否存在。
3. direction 只允许 ASC 或 DESC。
4. 经过标识符引用后生成 SQL。

### 11.6 筛选安全

筛选条件需要限制操作符。

不能允许前端传入任意 SQL 操作符或表达式。

对于 NULL 条件：

```
IS NULL
IS NOT NULL
```

不能错误地生成：

```
column = NULL
```

对于 LIKE：

```
column LIKE ?
```

由后端根据用户输入构造参数值。

### 11.7 分页

初期建议使用 LIMIT/OFFSET：

```
SELECT *
FROM `users`
ORDER BY `id` ASC
LIMIT ? OFFSET ?;
```

页码计算：

```
offset = (page - 1) * pageSize
```

需要校验：

- page 大于等于 1。
- pageSize 大于 0。
- pageSize 不超过系统最大值。
- offset 不发生整数溢出。

LIMIT/OFFSET 在大偏移量场景下可能性能较差。

后续可以评估 Keyset Pagination，但 V1 不必一开始实现复杂分页机制。

### 11.8 总行数

可以使用：

```
SELECT COUNT(*)
FROM `users`;
```

但是大表 COUNT(*) 可能耗时较长。

建议：

- 将总数查询作为独立任务。
- 允许关闭精确总数统计。
- 大表可以采用只显示当前页和是否存在下一页的方式。
- 不要将 information_schema.TABLES.TABLE_ROWS 当成精确行数。

## 12. 表数据新增、修改、删除

### 12.1 新增数据

新增操作建议根据表结构生成 INSERT。

例如：

```
INSERT INTO `users` (
    `username`,
    `status`
)
VALUES (?, ?);
```

需要处理：

- 自增主键。
- 默认值。
- 可空字段。
- 必填字段。
- JSON 字段。
- 二进制字段。
- 时间字段。

自增字段如果由数据库自动生成，通常不需要在 INSERT 中主动提交。

### 12.2 修改数据

修改数据必须有可靠的行定位条件。

优先使用：

1. 主键。
2. 唯一索引。
3. 能够唯一确定记录的复合键。

例如：

```
UPDATE `users`
SET `username` = ?
WHERE `id` = ?;
```

复合键：

```
UPDATE `user_roles`
SET `status` = ?
WHERE `user_id` = ?
  AND `role_id` = ?;
```

不能假设每张表都只有一个主键字段。

### 12.3 无主键表

无主键表是数据浏览器必须谨慎处理的场景。

例如：

```
CREATE TABLE logs (
    content TEXT,
    created_at DATETIME
);
```

如果两条数据完全相同，单纯根据字段值构造 WHERE 条件，可能同时匹配多行。

因此：

- 有主键时，优先使用主键更新。
- 有唯一索引时，可以使用唯一索引定位。
- 没有可靠唯一键时，默认限制编辑和删除。
- 不允许静默执行可能影响多行的操作。
- 如需支持无主键表编辑，必须设计额外的行定位与影响行数确认机制。

V1 建议对无可靠唯一定位条件的表禁用直接修改和删除。

### 12.4 删除数据

删除操作：

```
DELETE FROM `users`
WHERE `id` = ?;
```

删除前必须：

- 确认当前行定位条件。
- 显示待删除记录。
- 要求用户确认。
- 执行后检查影响行数。
- 刷新数据列表。

如果影响行数超过预期，应将异常情况反馈给用户。

### 12.5 并发修改冲突

数据浏览器中可能出现：

用户打开某行数据后，其他客户端修改了同一行。

建议后续支持乐观锁式的数据冲突检测。

V1 可以先通过原始值比较实现基础检测：

```
UPDATE `users`
SET `username` = ?
WHERE `id` = ?
  AND `username` = ?;
```

如果影响行数为 0，需要提示：

```
数据可能已被其他用户修改，请刷新后重试。
```

对于包含 NULL、二进制字段或复杂类型的表，原始值比较需要特别处理。

不能将其简单视为完整的通用乐观锁实现。

## 13. 事务管理

### 13.1 使用场景

事务主要用于：

- 多条相关数据修改。
- 批量插入。
- 批量更新。
- 需要保证一致性的操作。

### 13.2 事务流程

```
开始事务
   |
   v
执行 SQL
   |
   v
检查执行结果
   |
   +---- 成功 ----> COMMIT
   |
   +---- 失败 ----> ROLLBACK
```

### 13.3 连接绑定

事务必须绑定到同一个数据库连接。

不能在事务开始后，每条 SQL 都随意从连接池获取不同连接。

### 13.4 事务边界

V1 需要明确区分：

- 单条 SQL 自动提交。
- 用户显式开启事务。
- 应用内部批量操作事务。

不要擅自将用户输入的多条 SQL 包装到一个事务中。

因为 DDL 在 MySQL 中存在隐式提交等行为，不能假设所有语句都可以通过普通事务完全回滚。

### 13.5 事务界面

SQL 编辑器后续可以提供：

```
START TRANSACTION;

UPDATE users SET status = 1 WHERE id = 1;

COMMIT;
```

用户显式执行事务语句时，执行结果必须能够反映事务状态。

V1 不需要实现复杂的事务可视化管理界面。

## 14. 错误分类

建议统一定义 AdapterError。

```
pub enum AdapterError {
    ConnectionError(String),

    AuthenticationError(String),

    DatabaseNotFound(String),

    TableNotFound(String),

    ColumnNotFound(String),

    SqlExecutionError(String),

    QueryCancelled,

    Timeout,

    SerializationError(String),

    UnsupportedOperation(String),

    InternalError(String),
}
```

以上是逻辑分类示意。

实际实现应尽量保留底层 MySQL 错误码、SQLSTATE 和原始错误链，避免所有错误都退化成一个字符串。

### 14.1 常见错误

| 错误场景         | 处理方式             |
| ---------------- | -------------------- |
| 数据库地址错误   | 提示连接失败         |
| 端口无法访问     | 提示网络连接异常     |
| 用户名密码错误   | 提示认证失败         |
| 数据库不存在     | 提示数据库不存在     |
| 表不存在         | 提示表不存在         |
| 字段不存在       | 提示字段不存在       |
| SQL 语法错误     | 返回 MySQL 错误信息  |
| 权限不足         | 提示当前账户权限不足 |
| 连接超时         | 提示连接超时         |
| 查询取消         | 返回取消状态         |
| 数据类型解析失败 | 返回类型解析错误     |

### 14.2 错误信息安全

不能直接将完整底层异常全部展示给用户。

例如：

```
mysql://root:password@127.0.0.1:3306/test
```

这样的信息绝对不能出现在普通错误提示和日志中。

建议：

- UI 显示经过处理的错误。
- 日志保留必要上下文。
- 密码和凭据字段始终脱敏。
- SQL 是否记录到日志应由安全策略控制。

## 15. 性能与资源管理

### 15.1 结果集读取

不能无上限读取数据库结果集。

需要控制：

- 单次查询最大返回行数。
- 单批 IPC 返回行数。
- 单次传输的数据字节数。
- 单个字段的最大预览长度。
- 并发查询数量。

### 15.2 超大字段

例如：

```
LONGTEXT
LONGBLOB
```

可能包含非常大的数据。

不能为了显示一张表，默认完整读取所有超大字段。

建议：

- 支持按需加载字段内容。
- 对大型二进制字段显示摘要。
- 对超大文本提供预览。
- 明确显示内容已截断或未完整加载。
- 提供单独查看完整内容的入口。

禁止静默截断。

### 15.3 并发查询

每个数据源应限制同时执行的查询数量。

建议初始配置：

```
普通查询并发：3
后台元数据查询：2
```

这是初始建议，不是强制固定值。

需要避免：

- 一个查询长期占用所有连接。
- 大量元数据查询同时执行。
- 前端重复点击导致任务无限增加。

### 15.4 资源释放

以下场景需要释放对应资源：

- 用户关闭数据源。
- 用户删除数据源。
- 用户关闭 SQL 编辑器。
- 用户取消查询。
- 应用退出。
- 网络连接异常。

正在执行的任务不能因为前端组件销毁就失去管理。

任务生命周期应该由 Rust 后端统一维护。

## 16. MySQL 安全设计

### 16.1 SQL 注入防护

必须做到：

- 值参数化。
- 标识符安全引用。
- 排序字段白名单。
- 筛选操作符白名单。
- 不允许将前端输入直接作为 SQL 片段。

### 16.2 高危操作提醒

以下操作可能造成数据丢失：

```
DROP DATABASE
DROP TABLE
TRUNCATE TABLE
DELETE FROM users
UPDATE users SET status = 1
```

可以考虑增加高危操作提示。

但不能在 V1 中擅自禁止用户执行合法 SQL。

建议：

- 对危险操作进行识别。
- 对明显缺少 WHERE 的 UPDATE/DELETE 提示风险。
- 对 DROP/TRUNCATE 提示不可逆风险。
- 是否二次确认由产品交互规范确定。
- 最终执行权限仍由 MySQL 账户决定。

SQL 风险识别不能仅靠简单正则表达式，应尽可能使用 SQL 解析器，且不能将解析失败等同于安全。

### 16.3 生产环境保护

可以考虑为数据源增加环境标记：

```
开发
测试
预发布
生产
```

生产环境可以在 UI 中显示更明显的提示。

但是环境标记只是用户配置，不代表系统能够自动识别数据库的真实环境。

## 17. 测试设计

### 17.1 单元测试

重点测试：

- MySQL 标识符转义。
- SQL 请求参数校验。
- 数据类型映射。
- BIGINT 精度。
- DECIMAL 精度。
- NULL 处理。
- 日期时间处理。
- 排序条件校验。
- 筛选条件生成。
- 分页参数校验。
- 错误映射。

### 17.2 集成测试

使用真实 MySQL 测试实例。

测试：

- 创建连接。
- 断开连接。
- 重新连接。
- 数据库列表查询。
- 表列表查询。
- 字段结构查询。
- 索引查询。
- 建表 SQL 查询。
- SELECT。
- INSERT。
- UPDATE。
- DELETE。
- DDL。
- 事务。
- 多语句。
- 多结果集。
- 查询取消。
- 大结果集。
- 大字段。
- 网络异常。

### 17.3 类型测试

建议准备覆盖以下类型的测试表：

```
CREATE TABLE datadock_type_test (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,

    tiny_int TINYINT,
    small_int SMALLINT,
    medium_int MEDIUMINT,
    normal_int INT,
    big_int BIGINT,

    decimal_value DECIMAL(30, 10),

    float_value FLOAT,
    double_value DOUBLE,

    char_value CHAR(10),
    varchar_value VARCHAR(255),

    text_value TEXT,
    long_text_value LONGTEXT,

    date_value DATE,
    time_value TIME,
    datetime_value DATETIME,
    timestamp_value TIMESTAMP NULL,

    json_value JSON,

    enum_value ENUM('A', 'B', 'C'),

    binary_value BLOB,

    nullable_value VARCHAR(100) NULL
);
```

测试数据应包含：

- 超大整数。
- 高精度小数。
- 空字符串。
- NULL。
- 中文。
- 特殊字符。
- 超长文本。
- JSON。
- 二进制数据。
- 不同时间值。

### 17.4 安全测试

需要验证：

- SQL 参数注入。
- 表名特殊字符。
- 字段名特殊字符。
- 非法排序字段。
- 非法筛选操作符。
- 密码是否进入日志。
- 凭据删除是否正确。
- 无主键表是否可能误修改多行。
- 查询取消是否可能影响其他连接。

### 17.5 版本测试

不同 MySQL 版本需要单独验证。

不能只在一个 MySQL 版本中通过测试，就宣称支持所有 MySQL 版本。

## 18. V1 实施范围

### 18.1 必须实现

| 功能             | 优先级 |
| ---------------- | ------ |
| MySQL 连接       | P0     |
| 连接测试         | P0     |
| 连接关闭         | P0     |
| 数据库列表       | P0     |
| 表和视图列表     | P0     |
| 字段结构查询     | P0     |
| 索引查询         | P0     |
| 建表 SQL 查看    | P0     |
| SQL 执行         | P0     |
| SELECT 结果展示  | P0     |
| DML 执行         | P0     |
| DDL 执行         | P0     |
| 查询历史记录     | P0     |
| 表数据分页       | P0     |
| 表数据排序       | P0     |
| 表数据筛选       | P0     |
| 表数据新增       | P0     |
| 表数据修改       | P0     |
| 表数据删除       | P0     |
| 数据类型精度保护 | P0     |
| 连接断开处理     | P0     |
| 基础错误处理     | P0     |

### 18.2 可以后续实现

| 功能              | 说明         |
| ----------------- | ------------ |
| SSH 隧道          | 后续版本     |
| 数据库备份        | 后续版本     |
| 数据库恢复        | 后续版本     |
| 数据库同步        | 后续版本     |
| 表结构修改可视化  | 后续版本     |
| 数据库用户管理    | 后续版本     |
| 数据库权限管理    | 后续版本     |
| 复杂事务可视化    | 后续版本     |
| 高级执行计划分析  | 后续版本     |
| Keyset Pagination | 性能优化阶段 |

V1 的表结构功能以查看为主，不要求实现完整的可视化 DDL 修改工具。

## 19. AI 开发约束

AI 在实现 MySQL Adapter 时必须遵守以下要求。

### 19.1 不得擅自改变架构

- MySQL 逻辑必须放在独立 Adapter 中。
- Tauri Commands 不直接编写数据库业务 SQL。
- Service 层负责业务流程协调。
- Repository 负责本地 SQLite 数据访问。
- MySQL Adapter 负责远程 MySQL 操作。

### 19.2 不得牺牲数据精度

- BIGINT 不得无条件转换为 JavaScript Number。
- DECIMAL 必须保留精度。
- NULL 和空字符串必须区分。
- 日期时间不能随意进行时区转换。
- 二进制数据不能直接当普通文本处理。

### 19.3 不得忽略安全问题

- 密码不能写入 SQLite。
- 密码不能写入日志。
- SQL 参数必须参数化。
- 标识符必须安全引用。
- 不允许静默执行可能影响多行的无主键更新和删除。

### 19.4 不得虚构驱动能力

如果驱动不支持：

- 多结果集。
- 多语句。
- 流式读取。
- 查询取消。
- 特定类型解码。

必须先验证，再制定替代方案。

不能通过简单修改接口名称，让一个实际不支持的功能看起来已经实现。

### 19.5 不得一次性实现所有功能

建议按以下顺序实施：

1. 驱动 PoC。
2. 连接管理。
3. 数据库和表元数据。
4. 字段与索引查询。
5. 基础 SQL 执行。
6. SQL 结果类型映射。
7. 结果集分页与流式读取。
8. 数据浏览器查询。
9. 数据新增、修改、删除。
10. 事务和取消能力。
11. 异常场景和安全测试。

每个阶段完成后进行实际验证，再进入下一阶段。

## 20. 验收标准

MySQL Adapter 完成后，必须满足以下要求。

### 20.1 连接验收

- 可以配置多个 MySQL 数据源。
- 每个数据源独立连接。
- 可以测试连接。
- 可以打开和关闭连接。
- 可以识别连接异常。
- 关闭一个数据源不会影响其他数据源。
- 密码不会明文写入 SQLite。

### 20.2 元数据验收

- 可以查看数据库列表。
- 可以查看表列表。
- 可以查看视图列表。
- 可以查看字段信息。
- 可以查看索引。
- 可以查看主键和唯一键。
- 可以查看建表 SQL。

### 20.3 SQL 执行验收

- 可以执行 SELECT。
- 可以执行 INSERT。
- 可以执行 UPDATE。
- 可以执行 DELETE。
- 可以执行 DDL。
- 可以正确展示查询结果。
- 可以正确展示影响行数。
- 可以记录执行耗时。
- 可以正确处理 SQL 错误。
- 多语句和多结果集能力有明确的实际测试结论。

### 20.4 数据浏览器验收

- 可以分页读取表数据。
- 可以排序。
- 可以筛选。
- 可以新增记录。
- 可以修改有可靠唯一定位条件的记录。
- 可以删除有可靠唯一定位条件的记录。
- 无主键表不会默认执行存在多行误操作风险的修改和删除。

### 20.5 数据安全验收

- BIGINT 不丢失精度。
- DECIMAL 不丢失精度。
- NULL 和空字符串可以区分。
- 日期时间展示没有未经授权的时区偏移。
- 二进制字段不会被错误解码成普通文本。
- 超大字段不会被静默截断。
- 查询取消不会错误地宣称服务端已经停止执行。

## 21. 最终设计说明

MySQL Adapter 是 DataDock V1 的核心模块之一。

实现过程中，需要优先保证：

1. 连接稳定性。
2. SQL 执行正确性。
3. 数据读取准确性。
4. 数据修改安全性。
5. 数据类型精度。
6. 大结果集处理能力。
7. 后续数据库类型扩展能力。

对于暂时无法确认的驱动能力，必须通过 PoC 验证。

对于暂时不需要的高级功能，不要提前引入复杂实现。

DataDock 的目标是先完成一个稳定、实用、可长期扩展的 MySQL 数据库管理工具，而不是在 V1 阶段实现所有数据库管理软件的全部功能。