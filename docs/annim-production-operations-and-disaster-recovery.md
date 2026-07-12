# Annim 生产运维与灾备 Runbook

本文面向 Annim、catalog worker、cover worker 和 Anni workspace 的生产运维人员。内容以仓库当前实现为准，说明如何启动服务、轮换密钥、备份和恢复数据，以及如何定期验证备份确实可用。

文中的 `/srv/...`、`/mnt/...`、systemd unit 名称、PostgreSQL service 名称和对象存储产品均为部署方占位符，必须按实际环境替换。不要直接复制一个尚未替换占位符的命令到生产环境。

## 1. 不可破坏的安全规则

以下规则优先于恢复速度和运维便利性：

1. 备份账号对源音频只能拥有读取和遍历权限，不得拥有写入、重命名或删除权限。
2. 优先从存储平台提供的只读 snapshot/mount 读取音频。若只能读取生产挂载点，应由存储管理员确认其使用 `noatime`/等价策略；某些 `strictatime` 文件系统会因普通读取更新 atime，这也不符合“源数据零变更”的严格要求。
3. 每次备份都写入一个全新的、不可变的 generation 目录或对象存储前缀。不得把生产目录当作 rsync 目标。
4. 禁止在备份命令中使用 `rsync --delete`、`rsync --remove-source-files`、`mv`、`rm`、`cp -al` 或任何“同步后清理源文件”的选项。
5. 不得用源目录与备份目录之间的硬链接代替备份。两者共享 inode 时，后续写入会同时改变“备份”。文件系统原生的只读快照或 copy-on-write 快照可以使用，但需要由存储平台确认隔离语义。
6. 恢复必须落到新的数据库、新的 cover root 和新的音频目录。校验完成后通过配置或挂载切换；不得直接覆盖现有生产音频。
7. 备份和恢复验证期间禁止运行会改变工作区的命令，包括 `anni workspace fsck --gc`、`anni workspace fsck --fix-dangling`、`anni workspace publish`、`anni workspace revert` 和音频标签写入工具。
8. 用户可见的 workspace 目录可能只保存指向 `.anni/objects` 的符号链接。备份这些符号链接不能替代备份其实际音频目标。

如果任何脚本无法证明“源路径只读、目标路径为新 generation”，应立即停止，而不是尝试自动修复路径。

## 2. 当前运行边界和数据面

### 2.1 组件

| 组件 | 当前职责 | 写入位置 |
| --- | --- | --- |
| `annim` | GraphQL API、认证、迁移、健康检查、Tantivy 搜索 | Annim 数据库、`ANNIM_SEARCH_DIRECTORY` |
| `catalog-worker` | 领取 catalog sync run，从外部目录抓取版本化 observation | Annim 数据库；只读取 catalog secret root |
| `cover-worker` | 领取 cover candidate，安全下载并验证图片 | Annim 数据库、`ANNIM_COVER_ASSET_ROOT` |
| Anni workspace/repository | 保存传统 TOML 元数据、受控音频和发布后的音频 | `.anni/repo`、`.anni/objects`、配置中的 library root |

当前 `catalog-worker` 只注册了 Apple Music adapter。数据库模型可以记录其他信源，但不能因此假设 VGMDB、唱片公司或艺人官网已有可运行的 worker adapter。

当前 `anni-ingest-worker` 是供调用方集成的 Rust library，没有可直接部署的 worker binary、队列轮询器或环境变量契约。Annim 会保存 ingest job 状态、metadata revision 正文，以及 manifest/plan/verification 的 digest；它不保存 input manifest、execution plan 和 execution receipt 的完整正文。实际部署若已由外部协调器执行 ingest，协调器的 immutable artifact store、source root 和 staging/publish receipt 也是恢复面，不能只备份 Annim 数据库。

此外，当前 Web 的 catalog source 创建入口不会接收或返回 `secret_ref`，而 Apple adapter 运行时要求数据库中已有合法 `secret_ref`。生产部署必须使用受信任的后端配置流程预置该引用；本文不把“在 Web 中配置 Apple/VGMDB 多源同步”描述为现有能力。

实现入口：

- Annim 启动和迁移：[annim/src/main.rs](../annim/src/main.rs)
- 网络与 CORS 配置：[annim/src/config.rs](../annim/src/config.rs)
- Bearer token 规则：[annim/src/auth.rs](../annim/src/auth.rs)
- Catalog worker 配置和 secret resolver：[anni-catalog-worker/src/bin/catalog-worker.rs](../anni-catalog-worker/src/bin/catalog-worker.rs)
- Cover worker 配置：[annim/src/bin/cover-worker.rs](../annim/src/bin/cover-worker.rs)
- Cover 内容寻址规则：[anni-catalog/src/cover.rs](../anni-catalog/src/cover.rs)
- Workspace 目录语义：[anni-workspace/src/lib.rs](../anni-workspace/src/lib.rs)
- 传统元数据仓库语义：[anni-repo/src/manager.rs](../anni-repo/src/manager.rs)

### 2.2 必须纳入恢复集的数据

一个可以恢复的 Annim generation 至少包含：

| 数据 | 权威性和恢复要求 |
| --- | --- |
| Annim 数据库 | 权威数据。包含元数据、证据和修订、ingest job 状态与 digest、catalog observation、cover candidate/selection 及文件引用；不包含完整 ingest manifest/plan/receipt 正文 |
| `ANNIM_COVER_ASSET_ROOT` | 权威图片字节。已验证文件使用 `sha256/aa/bb/<64 位小写 SHA-256>.<ext>`；必须和数据库属于同一 generation |
| `ANNIM_SEARCH_DIRECTORY` | Tantivy 派生索引，可通过受保护的 GraphQL `rebuildSearchIndex` mutation 从数据库重建。备份仅用于缩短恢复时间，不能作为权威数据 |
| Catalog secret | 不进入普通文件备份。由 secret manager 单独做版本化、加密和访问审计；恢复时重新物化为普通文件 |
| `.anni/repo` | 传统 Anni TOML 元数据仓库。Git remote 不能替代本地备份，因为可能存在未提交或未推送内容 |
| `.anni/config.toml` | Workspace 布局和发布目标。远程 metadata 模式下可能含 token，必须按 secret 处理 |
| `.anni/objects` | 尚未硬发布的受控音频原件 |
| 每个 library root | 硬发布后音频可能只存在于这里；路径来自 `.anni/config.toml` 的 library 配置 |
| Workspace 用户区 | 目录和符号链接拓扑。它有助于恢复操作视图，但不能替代 `.anni/objects` 和 library root |
| 外部 ingest artifact store（若已部署） | 保存完整 input manifest、execution plan、execution receipt 和协调器审计记录；其字节必须能重新计算出 Annim 数据库中的 digest |
| CD/Booklet evidence store | 保存原始 CUE/WAV、抓轨日志、Booklet/包装扫描或 PDF。Metadata revision 只保存 evidence 的 locator/detail，不保存这些二进制原件 |

“Anni repository”在现有代码中主要指 `repo.toml`、album/tag TOML 等元数据；它本身不是完整音频库。灾备必须同时覆盖元数据仓库和真实音频根目录。

Annim 数据库还可能包含 private locator、提交/生效 URL、catalog raw document、source configuration 和 evidence locator。数据库 dump、row-count 附件和演练日志都应按敏感数据加密、最小授权和审计，不能上传到普通 CI artifact 或公开对象存储。

## 3. 上线前参数表

### 3.1 先确定恢复目标

生产负责人应在上线前填写以下值。没有明确的 RPO/RTO 时，不应声称系统已具备灾备能力。

| 项目 | 生产值 |
| --- | --- |
| 数据库 RPO | `[待填写，例如 15 分钟]` |
| Cover / metadata repo RPO | `[待填写]` |
| 新入库音频 RPO | `[待填写；不得晚于删除唯一上游副本的时间]` |
| Annim API RTO | `[待填写]` |
| 全量音频恢复 RTO | `[待填写]` |
| 不可变备份保留期 | `[待填写]` |
| 恢复演练频率 | `[待填写；建议至少每季度]` |
| 灾备负责人和替补 | `[待填写]` |

### 3.2 Annim server 环境变量

| 变量 | 必需 | 当前规则 |
| --- | --- | --- |
| `ANNIM_DATABASE_URL` | 是 | SQLite 或 PostgreSQL SeaORM URL。不要把含密码的 URL 写入仓库或日志 |
| `ANNIM_AUTH_TOKEN` | 是 | 至少 32 个字符，不允许空白；只在进程启动时读取 |
| `ANNIM_SEARCH_DIRECTORY` | 是 | Tantivy 目录；进程会创建目录，但运行用户必须拥有安全的读写权限 |
| `ANNIM_BIND_ADDR` | 否 | 默认 `127.0.0.1:8000`；非 loopback 监听必须放在受信任的 TLS reverse proxy 后 |
| `ANNIM_ALLOWED_ORIGINS` | 否 | 逗号分隔的精确 `http`/`https` origin；默认空。不能使用 `*`、`null`、路径、凭据或尾随 `/` |
| `ANNIM_GRAPHIQL_ENABLED` | 否 | 只能是 `true` 或 `false`，默认 `false` |

健康检查不需要 token：

```bash
curl --fail --silent --show-error http://127.0.0.1:8000/health/live
curl --fail --silent --show-error http://127.0.0.1:8000/health/ready
```

`live` 只证明进程能响应；`ready` 还会执行数据库 ping。两者返回 `Cache-Control: no-store`。

### 3.3 Catalog worker 环境变量

| 变量 | 必需 | 默认值 / 当前边界 |
| --- | --- | --- |
| `ANNIM_DATABASE_URL` | 是 | 必须指向与 Annim server 相同的数据库 |
| `ANNIM_CATALOG_SECRET_ROOT` | 是 | 必须是已存在的绝对目录 |
| `ANNIM_CATALOG_POLL_INTERVAL_SECONDS` | 否 | 默认 5；范围 1–3600 |
| `ANNIM_CATALOG_LEASE_SECONDS` | 否 | 默认 600；范围 30–3600 |
| `ANNIM_CATALOG_PAGE_TIMEOUT_SECONDS` | 否 | 默认 60；范围 2–600，且必须小于 lease |
| `ANNIM_CATALOG_CONNECT_TIMEOUT_SECONDS` | 否 | 默认 10；范围 1–60，不得大于 request timeout |
| `ANNIM_CATALOG_REQUEST_TIMEOUT_SECONDS` | 否 | 默认 30；范围 1–300，且必须小于 page timeout |
| `ANNIM_CATALOG_MAX_RESPONSE_BYTES` | 否 | 默认 4 MiB；范围 64 KiB–32 MiB |
| `ANNIM_CATALOG_MAX_SECRET_BYTES` | 否 | 默认 16 KiB；范围 256 B–64 KiB |

Apple Music source 的 `secret_ref` 是相对于 secret root 的普通文件路径。resolver 会拒绝绝对路径、`.`、`..`、反斜杠、控制字符、符号链接、非普通文件、越界路径和非 UTF-8 内容。文件内容必须是合法 compact JWT，且不能带尾随换行。数据库只保存 `secret_ref`，不应保存 token 本身。

### 3.4 Cover worker 环境变量

| 变量 | 必需 | 当前规则 |
| --- | --- | --- |
| `ANNIM_DATABASE_URL` | 是 | 必须指向同一 Annim 数据库 |
| `ANNIM_COVER_ASSET_ROOT` | 是 | 必须预先存在；生产中应使用绝对路径和专用挂载点 |
| `ANNIM_COVER_POLL_INTERVAL_SECONDS` | 否 | 默认 5；范围 1–3600 |

Worker 会在 cover root 下创建 `.incoming`，下载完成后再把已验证文件提交到内容寻址路径。备份不应把 `.incoming` 中的 partial 文件当作有效资产。

### 3.5 构建和启动顺序

默认 feature 是 PostgreSQL：

```bash
cargo build --locked --release -p annim --bins
cargo build --locked --release -p anni-catalog-worker --bin catalog-worker
```

SQLite 部署应分别显式构建：

```bash
cargo build --locked --release -p annim --no-default-features --features sqlite --bins
cargo build --locked --release -p anni-catalog-worker --no-default-features --features sqlite --bin catalog-worker
```

当前 SQLite 路径没有单独的 WAL/busy-timeout 运维配置，而 server、catalog worker 和 cover worker 都会连接同一数据库。多进程生产部署应优先 PostgreSQL；选择 SQLite 时必须在目标负载下验证锁等待，并在备份/恢复期间完整停止 writer。

二进制名称为 `annim`、`cover-worker` 和 `catalog-worker`。生产中应由 systemd、容器编排器或等价 supervisor 管理，并把环境变量放在权限受控的 secret/config 文件中。

启动顺序：

1. 确认数据库、search root、cover root 和 secret root 已挂载到预期设备。
2. 启动 `annim`。它会先执行数据库 migration。
3. 等待 `/health/ready` 成功。
4. 启动 `catalog-worker`。它也会执行 migration；此时应为 no-op。
5. 启动 `cover-worker`。
6. 确认 worker 日志没有持续 retry 或 cycle failure。

停止时使用相反顺序：先停止 catalog/cover worker，再阻断 API 写流量并停止 Annim。应等待 supervisor 报告进程已退出，不要直接发送不可捕获的强制终止信号。

由于 Annim 和 catalog worker 当前会在启动时自动 migration，它们使用的数据库身份需要相应 DDL 权限。若生产安全基线要求运行身份仅有 DML 权限，需要先建设独立 migration job；在此之前不能简单移除 DDL 权限并期待服务仍能启动。

## 4. Secret 和 token 轮换

### 4.1 通用规则

- Secret manager 是权威来源；环境文件和 catalog secret root 只是运行时物化副本。
- 不在 shell history、工单、日志、进程参数截图或备份 manifest 中记录 secret 值。
- 轮换前确认旧值仍可短期回滚，轮换成功后再在上游撤销旧值。
- 轮换窗口内暂停相关 worker，避免一个多页 run 同时使用新旧凭据。
- 每次轮换记录负责人、时间、secret version 和验证结果，不记录明文。

### 4.2 `ANNIM_AUTH_TOKEN`

当前实现只接受一个 token，没有双 token 过渡期，而且只在 Annim 启动时读取。轮换需要维护窗口：

1. 在 secret manager 中生成至少 32 字符且无空白的新 token。例如 `openssl rand -hex 32` 会生成 64 个十六进制字符。
2. 先把新版本分发到客户端的受控配置，但暂不让客户端切换请求。
3. 阻断新写请求并停止 Annim。
4. 更新 Annim 的 `ANNIM_AUTH_TOKEN` secret version，然后启动 Annim。
5. 检查 live/ready；用新 token 发起一个只读 GraphQL 请求，确认 HTTP 200。
6. 确认旧 token 得到 HTTP 401 和 `WWW-Authenticate: Bearer realm="annim"`。
7. 切换客户端并撤销旧 token。WebSocket 客户端需要重新连接并在 connection-init 中发送新的 Bearer 值。

如果验证失败，恢复 secret manager 中的旧 version 并重启 Annim。不要为了临时恢复服务而设置短 token 或默认 token。

### 4.3 Apple Music developer token

推荐使用稳定的 `secret_ref` 文件名，由 secret manager 在同一文件系统内原子替换文件。示例中的路径仅为部署占位符：

```bash
export CATALOG_SECRET_ROOT=/run/secrets/annim/catalog
export APPLE_SECRET_PATH="$CATALOG_SECRET_ROOT/apple/developer-token"

# [按部署平台替换] NEW_APPLE_TOKEN_FILE 是 secret manager 物化的新版本。
install -m 0600 -- /run/secrets/NEW_APPLE_TOKEN_FILE "$APPLE_SECRET_PATH.next"
mv -f -- "$APPLE_SECRET_PATH.next" "$APPLE_SECRET_PATH"
```

执行前停止 `catalog-worker`；执行后确认文件和每一级父目录都不是符号链接，再启动 worker。旧 token 仅保留在 secret manager 的受控版本历史中，不在 secret root 留明文副本。验证一次 Apple sync 成功后再撤销旧 token。

### 4.4 数据库凭据

1. 在数据库或托管平台中创建可重叠使用的新凭据，不先撤销旧凭据。
2. 更新 Annim、catalog worker 和 cover worker 的 `ANNIM_DATABASE_URL` secret version。
3. 按“worker 停止 → Annim 重启并 ready → worker 启动”的顺序重启。
4. 确认三个进程均已建立新连接，再撤销旧凭据。

PostgreSQL CLI 建议通过权限为 `0600` 的 `PGPASSFILE` 和 `PGSERVICE` 使用凭据，避免把密码直接写进命令行。SQLite 文件权限应限制到服务账号和备份只读账号。

## 5. 3-2-1 备份策略

每个已验证 generation 应满足：

- **3 份副本**：生产数据、同站点快速恢复副本、异地副本。
- **2 个故障域或介质**：例如生产 NVMe + 独立备份存储，而不是同一文件系统中的两个目录。
- **1 份异地且不可变**：启用对象锁/WORM 或离线介质，使用独立凭据并加密。

建议的起始调度如下，最终频率以第 3.1 节的 RPO 为准：

| 数据 | 起始调度 | 说明 |
| --- | --- | --- |
| PostgreSQL | 每日 custom dump；若 RPO 小于一天，再配置托管 PITR/WAL 归档 | PITR 命令和保留策略由数据库提供商实现并演练 |
| SQLite | 至少每日 `.backup`；高频写入时提高频率 | 不能用在线复制 `db`/`-wal`/`-shm` 文件代替 `.backup` |
| Cover/search/repo | 每日 generation；重大 ingest 后额外生成 | 必须与可识别的数据库 generation 关联 |
| 音频 objects/library | 每次入库完成后进入增量备份；至少每日封存 | 在满足音频 RPO 前，不得删除抓轨、下载或分享来源中的唯一副本 |
| Secret | 由 secret manager 自动版本化和异地加密 | 不与普通文件备份放在同一权限域 |

每个 generation 保存以下 manifest：

- UTC backup ID、应用 commit/version、数据库类型和数据库版本；
- 所有数据 root 的明确路径和存储卷 ID；
- 数据库 row-count 摘要；
- 数据库 dump 和文件树 SHA-256；
- metadata repo 的 Git HEAD、`git status --porcelain` 和 lint 结果；
- 备份开始/结束时间、停写窗口、执行人和验证结果；
- 对象存储 version/retention ID，但不包含任何 secret。

## 6. 一致性备份流程

### 6.1 创建新 generation

以下是 GNU/Linux + Bash 示例。`BACKUP_BASE` 必须是独立备份设备或备份 agent 的 staging 区，不得位于任何源 audio root 内。

```bash
set -euo pipefail
umask 077

export BACKUP_ID="$(date -u +%Y%m%dT%H%M%SZ)"
export BACKUP_BASE=/mnt/annim-backup
export BACKUP_ROOT="$BACKUP_BASE/$BACKUP_ID"

: "${BACKUP_ROOT:?BACKUP_ROOT is required}"
test ! -e "$BACKUP_ROOT"
install -d -m 0700 -- "$BACKUP_ROOT/database" "$BACKUP_ROOT/manifests"
```

如果 generation 已存在，命令应失败。不得“继续写入”旧 generation。

### 6.2 进入停写窗口

1. 在 reverse proxy 或维护开关中阻断 GraphQL 写请求。
2. 停止 `catalog-worker` 和 `cover-worker`，等待当前操作退出。
3. 停止所有外部 ingest/归位进程。
4. 停止 Annim。
5. 用部署平台的进程和连接检查确认没有 writer。
6. 记录数据库、cover、search、workspace、objects 和所有 library root 的真实挂载点。

若部署存在外部 ingest artifact store，还要冻结并备份其 immutable manifests/plans/receipts、source locator 映射和 staging receipt。仓库当前没有统一路径或命令，必须在部署方 SOP 中明确，且 artifact backup 账号同样不得写入源音频。

`systemctl stop ...`、Kubernetes scale-down、云数据库 snapshot 等命令因部署平台而异，必须在本地 SOP 中填写；不要把本文中的组件名误认为已存在的 unit 名称。

### 6.3 SQLite 备份

`ANNIM_SQLITE_DB` 是从 `ANNIM_DATABASE_URL` 中人工确认出的绝对文件路径，不要用脆弱的 shell 字符串替换自动解析 URL。

```bash
export ANNIM_SQLITE_DB=/srv/annim/database/annim.sqlite3
export SQLITE_BACKUP="$BACKUP_ROOT/database/annim.sqlite3"

test -f "$ANNIM_SQLITE_DB"
test ! -e "$SQLITE_BACKUP"
sqlite3 -batch "$ANNIM_SQLITE_DB" ".timeout 5000" ".backup \"$SQLITE_BACKUP\""

sqlite3 -batch "$SQLITE_BACKUP" "PRAGMA quick_check;"
sqlite3 -batch "$SQLITE_BACKUP" "PRAGMA foreign_key_check;"
sha256sum -- "$SQLITE_BACKUP" > "$BACKUP_ROOT/manifests/annim-sqlite.sha256"
```

`quick_check` 必须只输出 `ok`，`foreign_key_check` 必须没有输出。SQLite 的 `.backup` 使用在线 backup API 生成一致快照；即使已停写，也不要用 `cp` 直接复制正在打开的主数据库、WAL 和 SHM 文件。

保存关键 row count：

```bash
sqlite3 -batch -header -csv "$SQLITE_BACKUP" \
  "SELECT 'album' AS entity, COUNT(*) AS rows FROM album
   UNION ALL SELECT 'ingest_job', COUNT(*) FROM ingest_job
   UNION ALL SELECT 'catalog_artist', COUNT(*) FROM catalog_artist
   UNION ALL SELECT 'catalog_source', COUNT(*) FROM catalog_source
   UNION ALL SELECT 'cover_asset', COUNT(*) FROM cover_asset
   ORDER BY entity;" \
  > "$BACKUP_ROOT/manifests/database-row-counts.csv"
```

### 6.4 PostgreSQL 备份

以下命令假定部署方已配置 `[annim]` libpq service 和权限为 `0600` 的 `PGPASSFILE`。`service=annim` 是占位符，不是仓库内置配置。

```bash
export PG_BACKUP="$BACKUP_ROOT/database/annim.dump"

test ! -e "$PG_BACKUP"
pg_dump \
  --dbname="service=annim" \
  --format=custom \
  --no-owner \
  --no-acl \
  --file="$PG_BACKUP"

pg_restore --list "$PG_BACKUP" > "$BACKUP_ROOT/manifests/annim-pg-archive.list"
sha256sum -- "$PG_BACKUP" > "$BACKUP_ROOT/manifests/annim-pg.sha256"
```

必须检查 `pg_dump` 的 stderr 和退出码。Custom format 可由 `pg_restore` 检查目录并恢复；不要仅凭 dump 文件大小判断成功。

该 dump 使用 `--no-owner --no-acl`，不会恢复数据库 role、密码或原 ACL。它们必须由受版本控制的基础设施配置和 secret manager 重建，且不能依赖 dump 中的生产身份。

保存 row count：

```bash
psql --dbname="service=annim" --csv --set=ON_ERROR_STOP=1 \
  --command="SELECT 'album' AS entity, COUNT(*) AS rows FROM album
             UNION ALL SELECT 'ingest_job', COUNT(*) FROM ingest_job
             UNION ALL SELECT 'catalog_artist', COUNT(*) FROM catalog_artist
             UNION ALL SELECT 'catalog_source', COUNT(*) FROM catalog_source
             UNION ALL SELECT 'cover_asset', COUNT(*) FROM cover_asset
             ORDER BY entity;" \
  > "$BACKUP_ROOT/manifests/database-row-counts.csv"
```

托管 PostgreSQL 的 PITR、WAL archive、跨区副本和 snapshot 命令属于 **提供商特定配置**。它们应作为 custom dump 之外的恢复层，并且必须用独立数据库做实际恢复演练。

### 6.5 Cover、search、metadata repo 和音频

下面的示例假定 workspace 使用 `type = "repo"`。若使用 remote metadata，`.anni/repo` 可以不存在：省略 repo 专用命令，保留 `.anni/config.toml`，并确保远端 Annim 数据库已进入同一灾备计划。先显式设置实际路径：

```bash
export COVER_ROOT=/srv/annim/covers
export SEARCH_ROOT=/srv/annim/search
export ANNI_WORKSPACE_ROOT=/srv/anni/workspace
export ANNI_REPO_ROOT="$ANNI_WORKSPACE_ROOT/.anni/repo"
export ANNI_OBJECTS_ROOT="$ANNI_WORKSPACE_ROOT/.anni/objects"
export LIBRARY_ROOT=/srv/anni/library-main

for source in "$COVER_ROOT" "$SEARCH_ROOT" "$ANNI_REPO_ROOT" "$ANNI_OBJECTS_ROOT" "$LIBRARY_ROOT"; do
  test -d "$source"
done
test ! -e "$ANNI_REPO_ROOT/.repo_lock"
```

存在 `.repo_lock` 表示某个 owned repository manager 仍可能活动。不要删除 lock 来强行继续；先找到并停止持有者。

每个 library root 都要单独列出和备份；不要只保留一个示例变量。使用全新的目标目录：

```bash
install -d -m 0700 -- \
  "$BACKUP_ROOT/cover-assets" \
  "$BACKUP_ROOT/search-index" \
  "$BACKUP_ROOT/workspace-layout" \
  "$BACKUP_ROOT/metadata-repo" \
  "$BACKUP_ROOT/audio-objects" \
  "$BACKUP_ROOT/library-main"

rsync -aH --numeric-ids --exclude='/.incoming/' -- \
  "$COVER_ROOT/" "$BACKUP_ROOT/cover-assets/"
rsync -aH --numeric-ids -- "$SEARCH_ROOT/" "$BACKUP_ROOT/search-index/"
rsync -aH --numeric-ids \
  --exclude='/.anni/repo/' \
  --exclude='/.anni/objects/' \
  -- "$ANNI_WORKSPACE_ROOT/" "$BACKUP_ROOT/workspace-layout/"
rsync -aH --numeric-ids -- "$ANNI_REPO_ROOT/" "$BACKUP_ROOT/metadata-repo/"
rsync -aH --numeric-ids -- "$ANNI_OBJECTS_ROOT/" "$BACKUP_ROOT/audio-objects/"
rsync -aH --numeric-ids -- "$LIBRARY_ROOT/" "$BACKUP_ROOT/library-main/"
```

这些命令没有删除或移动源文件。`-H` 只在目标中保留源树内部已有的硬链接关系，不会在生产与备份之间创建硬链接。若平台需要 ACL/xattr，请先在演练环境验证 rsync 版本，再加入 `-A`/`-X`；不要猜测不同平台的选项兼容性。

如果原始 CD/CUE/WAV、抓轨日志和 Booklet 扫描位于 workspace 之外，为每个 source/evidence root 重复同样的只读复制。示例中的路径是部署方占位符：

```bash
export SOURCE_EVIDENCE_ROOT=/srv/anni/source-evidence
test -d "$SOURCE_EVIDENCE_ROOT"
install -d -m 0700 -- "$BACKUP_ROOT/source-evidence"
rsync -aH --numeric-ids -- "$SOURCE_EVIDENCE_ROOT/" "$BACKUP_ROOT/source-evidence/"
```

`rsync -a` 会保留但不会跟随 evidence root 中的符号链接。若发现链接，单独登记其目标为新的只读 source root；不要用全局 `--copy-links` 让备份进程越过已审核边界。

`workspace-layout` 保存 `.anni/config.toml`、用户区符号链接和尚未进入受控 objects 的普通文件，但排除已单独备份的 repo/objects。由于 config 可能含远程 token，这部分 generation 必须加密并限制访问。

Workspace 的 `.album` 和 track 链接可能保存绝对目标。Rsync 会原样保存链接而不会跟随它们；这正是备份时需要的行为。恢复到不同根目录后必须先审计并在 **恢复副本内** 重建或 rebase 这些链接，不能让 drill 链接继续指向生产 objects。

### 6.6 文件 hash manifest

以下 Bash helper 只读取源文件，并把相对路径 SHA-256 写入备份 generation。大型音频库会消耗大量 I/O，应在停写窗口或存储只读快照上执行。

```bash
hash_tree() (
  set -euo pipefail
  root=$1
  output=$2
  cd -- "$root"
  find . -type f -print0 \
    | LC_ALL=C sort -z \
    | while IFS= read -r -d '' path; do sha256sum -- "$path"; done \
    > "$output"
)

# Cover backup deliberately excludes .incoming, so hash the completed target tree.
hash_tree "$BACKUP_ROOT/cover-assets" "$BACKUP_ROOT/manifests/cover-tree.sha256"
hash_tree "$BACKUP_ROOT/workspace-layout" "$BACKUP_ROOT/manifests/workspace-layout-tree.sha256"
hash_tree "$ANNI_REPO_ROOT" "$BACKUP_ROOT/manifests/metadata-repo-tree.sha256"
hash_tree "$ANNI_OBJECTS_ROOT" "$BACKUP_ROOT/manifests/audio-objects-tree.sha256"
hash_tree "$LIBRARY_ROOT" "$BACKUP_ROOT/manifests/library-main-tree.sha256"
if [[ -n ${SOURCE_EVIDENCE_ROOT:-} ]]; then
  hash_tree "$SOURCE_EVIDENCE_ROOT" "$BACKUP_ROOT/manifests/source-evidence-tree.sha256"
fi
```

然后在备份副本上验证：

```bash
(cd -- "$BACKUP_ROOT/cover-assets" && sha256sum --check "$BACKUP_ROOT/manifests/cover-tree.sha256")
(cd -- "$BACKUP_ROOT/workspace-layout" && sha256sum --check "$BACKUP_ROOT/manifests/workspace-layout-tree.sha256")
(cd -- "$BACKUP_ROOT/metadata-repo" && sha256sum --check "$BACKUP_ROOT/manifests/metadata-repo-tree.sha256")
(cd -- "$BACKUP_ROOT/audio-objects" && sha256sum --check "$BACKUP_ROOT/manifests/audio-objects-tree.sha256")
(cd -- "$BACKUP_ROOT/library-main" && sha256sum --check "$BACKUP_ROOT/manifests/library-main-tree.sha256")
if [[ -n ${SOURCE_EVIDENCE_ROOT:-} ]]; then
  (cd -- "$BACKUP_ROOT/source-evidence" && sha256sum --check "$BACKUP_ROOT/manifests/source-evidence-tree.sha256")
fi
```

在 metadata repo 是 Git 仓库时，再记录：

```bash
git -C "$ANNI_REPO_ROOT" rev-parse HEAD > "$BACKUP_ROOT/manifests/metadata-repo-head.txt"
git -C "$ANNI_REPO_ROOT" status --porcelain=v1 > "$BACKUP_ROOT/manifests/metadata-repo-status.txt"
git -C "$ANNI_REPO_ROOT" fsck --full
```

`git fsck` 不验证未跟踪文件，因此 rsync generation 和文件 hash 仍是必须项。

### 6.7 封存并恢复服务

1. 把 generation manifest、dump digest 和备份执行日志写入不可变备份目录。
2. 由 **提供商特定工具** 将 generation 复制到异地加密、不可变存储；检查 object count 和 retention lock。
3. 先启动 Annim 并等待 ready，再启动 catalog/cover worker。
4. 解除 API 维护模式。
5. 记录实际停写时间和 generation ID。

不得在异地复制成功之前删除本地 generation，也不得把“备份任务进程退出 0”当作恢复验证的替代品。

## 7. 恢复到隔离环境

所有恢复演练都应使用与生产网络隔离的新路径和新数据库。Catalog/cover worker 初始保持停止，防止演练环境发出外部请求或改变恢复数据。

### 7.1 SQLite 恢复

```bash
set -euo pipefail
umask 077

export DRILL_ROOT=/srv/annim-drill/2026Q3
export RESTORED_SQLITE="$DRILL_ROOT/database/annim.sqlite3"

test ! -e "$DRILL_ROOT"
install -d -m 0700 -- "$DRILL_ROOT/database"
install -m 0600 -- "$BACKUP_ROOT/database/annim.sqlite3" "$RESTORED_SQLITE"

sqlite3 -batch "$RESTORED_SQLITE" "PRAGMA integrity_check;"
sqlite3 -batch "$RESTORED_SQLITE" "PRAGMA foreign_key_check;"
```

`integrity_check` 必须只输出 `ok`，foreign key check 必须无输出。演练 Annim URL 示例为：

```text
sqlite:///srv/annim-drill/2026Q3/database/annim.sqlite3?mode=rwc
```

不要让演练服务连接生产 SQLite 文件。

### 7.2 PostgreSQL 恢复

1. **[提供商特定]** 创建一个全新的空数据库和最小权限 restore role。
2. 配置新的 `service=annim-restore`，确认它不能解析到生产数据库。
3. 恢复 custom dump：

```bash
pg_restore \
  --dbname="service=annim-restore" \
  --no-owner \
  --no-acl \
  --single-transaction \
  --exit-on-error \
  "$BACKUP_ROOT/database/annim.dump"
```

目标数据库必须为空。不要对生产数据库使用 `--clean` 或 `--create`。大型数据库若因锁数量无法单事务恢复，可以在演练确认后去掉 `--single-transaction`，但必须保留 `--exit-on-error` 并在失败时丢弃整个目标数据库，而不是继续使用部分恢复结果。

PostgreSQL 14+ 可在隔离库执行结构检查：

```bash
pg_amcheck --on-error-stop "service=annim-restore"
```

`pg_amcheck` 需要数据库具备 `amcheck` 支持和足够权限。命令刻意不使用会安装 extension 的 `--install-missing`；如需安装，只能由数据库管理员在可丢弃恢复库完成并记录。`pg_amcheck` 不能替代业务 row count 和文件引用检查。

### 7.3 恢复文件数据

把 cover、repo 和音频恢复到全新目录。Search index 是派生数据，演练默认创建空目录并在 Annim 启动后重建，不复制旧 index。以下仍以 repo metadata 模式为例；remote 模式省略 `.anni/repo` 的创建、复制、hash 和 lint。示例使用 rsync 从只读 generation 复制到新 drill root；仍然不使用 `--delete`：

```bash
install -d -m 0700 -- \
  "$DRILL_ROOT/cover-assets" \
  "$DRILL_ROOT/search-index" \
  "$DRILL_ROOT/workspace" \
  "$DRILL_ROOT/workspace/.anni/repo" \
  "$DRILL_ROOT/workspace/.anni/objects" \
  "$DRILL_ROOT/library-main"

rsync -aH --numeric-ids -- "$BACKUP_ROOT/cover-assets/" "$DRILL_ROOT/cover-assets/"
rsync -aH --numeric-ids -- "$BACKUP_ROOT/workspace-layout/" "$DRILL_ROOT/workspace/"
rsync -aH --numeric-ids -- "$BACKUP_ROOT/metadata-repo/" "$DRILL_ROOT/workspace/.anni/repo/"
rsync -aH --numeric-ids -- "$BACKUP_ROOT/audio-objects/" "$DRILL_ROOT/workspace/.anni/objects/"
rsync -aH --numeric-ids -- "$BACKUP_ROOT/library-main/" "$DRILL_ROOT/library-main/"
if [[ -d "$BACKUP_ROOT/source-evidence" ]]; then
  install -d -m 0700 -- "$DRILL_ROOT/source-evidence"
  rsync -aH --numeric-ids -- "$BACKUP_ROOT/source-evidence/" "$DRILL_ROOT/source-evidence/"
fi
```

对恢复副本执行 hash，而不是对生产源执行任何修复：

```bash
(cd -- "$DRILL_ROOT/cover-assets" && sha256sum --check "$BACKUP_ROOT/manifests/cover-tree.sha256")
(cd -- "$DRILL_ROOT/workspace" && sha256sum --check "$BACKUP_ROOT/manifests/workspace-layout-tree.sha256")
(cd -- "$DRILL_ROOT/workspace/.anni/repo" && sha256sum --check "$BACKUP_ROOT/manifests/metadata-repo-tree.sha256")
(cd -- "$DRILL_ROOT/workspace/.anni/objects" && sha256sum --check "$BACKUP_ROOT/manifests/audio-objects-tree.sha256")
(cd -- "$DRILL_ROOT/library-main" && sha256sum --check "$BACKUP_ROOT/manifests/library-main-tree.sha256")
if [[ -f "$BACKUP_ROOT/manifests/source-evidence-tree.sha256" ]]; then
  (cd -- "$DRILL_ROOT/source-evidence" && sha256sum --check "$BACKUP_ROOT/manifests/source-evidence-tree.sha256")
fi
```

对恢复的 FLAC 副本做流校验：

```bash
find "$DRILL_ROOT/workspace/.anni/objects" "$DRILL_ROOT/library-main" \
  -type f -iname '*.flac' -print0 \
  | xargs -0 --no-run-if-empty -n 1 flac --test --silent --
```

`flac --test` 只对 drill 副本运行。任一文件失败都应使 generation 标记为不可用于无损恢复，并触发上一个 generation 的复验。

在运行任何 workspace 命令前，先确认恢复的符号链接没有逃出 drill root。以下是 GNU `readlink` 示例；若发现逃逸，停止并用部署方审核过的 rebase 工具只修改恢复副本：

```bash
while IFS= read -r -d '' link; do
  resolved=$(readlink -f -- "$link")
  case "$resolved" in
    "$DRILL_ROOT/workspace/"*) ;;
    *) printf 'link escapes drill root: %s -> %s\n' "$link" "$resolved" >&2; exit 1 ;;
  esac
done < <(find "$DRILL_ROOT/workspace" -type l -print0)
```

该检查本身只读。本文不提供生产链接的批量重写命令，因为错误前缀替换可能把链接指向生产或错误专辑；rebase 必须由部署方在隔离副本演练并双人复核。

### 7.4 数据库与 cover 文件交叉校验

SQLite 恢复库可导出所有数据库引用的 cover identity：

```bash
sqlite3 -batch -noheader -separator '|' "$RESTORED_SQLITE" \
  "SELECT lower(hex(content_sha256)), storage_key, byte_length, media_type
   FROM cover_asset ORDER BY storage_key;" \
  > "$DRILL_ROOT/cover-db-assets.tsv"
```

PostgreSQL 使用等价查询：

```bash
psql --dbname="service=annim-restore" --tuples-only --no-align \
  --field-separator='|' --set=ON_ERROR_STOP=1 \
  --command="SELECT encode(content_sha256, 'hex'), storage_key, byte_length, media_type
             FROM cover_asset ORDER BY storage_key;" \
  > "$DRILL_ROOT/cover-db-assets.tsv"
```

对两种数据库都执行同一文件检查：

```bash
while IFS='|' read -r expected_digest storage_key expected_bytes media_type; do
  [[ $storage_key =~ ^sha256/[0-9a-f]{2}/[0-9a-f]{2}/[0-9a-f]{64}\.(jpg|png|webp)$ ]]
  case "$media_type" in
    image/jpeg) expected_extension=jpg ;;
    image/png) expected_extension=png ;;
    image/webp) expected_extension=webp ;;
    *) printf 'unexpected cover media type: %s\n' "$media_type" >&2; exit 1 ;;
  esac
  expected_key="sha256/${expected_digest:0:2}/${expected_digest:2:2}/$expected_digest.$expected_extension"
  [[ $storage_key == "$expected_key" ]]

  file="$DRILL_ROOT/cover-assets/$storage_key"
  [[ -f $file && ! -L $file ]]
  actual_bytes=$(wc -c < "$file")
  [[ $actual_bytes -eq $expected_bytes ]]
  actual_digest=$(sha256sum -- "$file")
  [[ ${actual_digest%% *} == "$expected_digest" ]]
done < "$DRILL_ROOT/cover-db-assets.tsv"
```

该检查证明每个数据库引用的 key 具有正确 fan-out 形式，文件名 digest、实际字节数和实际 SHA-256 均与数据库一致。文件树 manifest 还会发现数据库未引用但 generation 中存在的额外文件。

### 7.5 元数据和业务校验

1. 在 repo metadata 模式下，对可丢弃的恢复 repo 运行内容 lint：

   ```bash
   anni repo --root "$DRILL_ROOT/workspace/.anni/repo" lint
   ```

   `repo lint` 会在恢复 repo 内短暂创建并删除 `.repo_lock`。因此只对可丢弃的恢复副本运行，不对不可变 generation 或生产 metadata repo 运行。

2. 对恢复 workspace 运行只读状态扫描：

   ```bash
   cd -- "$DRILL_ROOT/workspace"
   anni workspace status --json
   ```

   不要添加 `fsck --gc` 或 `fsck --fix-dangling`。

3. 在恢复数据库重新生成第 6 节的 row count，并与 `database-row-counts.csv` 比较。
4. 检查 queued/running ingest、catalog sync 和 cover candidate 的数量；恢复后先由人工决定是否重新启动 worker。
5. 若生产使用外部 ingest coordinator，从恢复 artifact store 重新计算 manifest/plan/receipt digest，并与 `ingest_job` 中的 digest 对比。正文缺失时只能审计状态，不能声称可重放该 job。
6. 使用专用 drill token、`ANNIM_BIND_ADDR=127.0.0.1:18000`、空 Origin allowlist、空的 drill search directory 和关闭的 GraphiQL 启动 Annim。
7. 确认 live/ready 和一个带 Bearer 的只读 GraphQL 请求成功。
8. 使用权限为 `0600` 的 curl config 提供 `Authorization: Bearer ...` header，在 drill 实例重建搜索索引：

   ```bash
   set -o pipefail
   curl --fail-with-body --silent --show-error \
     --config /run/secrets/annim-drill-curl.conf \
     --header 'Content-Type: application/json' \
     --data '{"query":"mutation { rebuildSearchIndex }"}' \
     http://127.0.0.1:18000/ \
     | jq --exit-status '.errors == null and .data.rebuildSearchIndex == true'
   ```

9. 抽样打开艺人 collection、metadata revision、catalog observation 和已选 cover，确认 Unicode 原文与证据链可读。

Annim 和 catalog worker 启动会执行 migration。若需要保留一份完全未改变的取证副本，应先复制恢复数据库，再在可丢弃副本上做启动验证。

### 7.6 演练通过条件

以下条件必须全部满足：

- 数据库结构检查、foreign key 检查和 row count 对比通过；
- 数据库 dump、cover、repo 和音频 hash 全部通过；
- 已部署的 ingest artifact store 完整，正文 digest 与 Annim job 引用一致；未部署 artifact store 时明确记录“历史 job 不可重放”；
- 恢复 FLAC 的流校验通过；
- metadata repo lint 通过或所有既有告警已被记录并与生产基线一致；
- Annim ready、认证、关键只读查询通过；
- 实际恢复耗时不超过 RTO；恢复点不早于 RPO；
- 演练过程中没有连接生产数据库、写入生产目录或发出 catalog/cover worker 外部任务；
- 结果、失败项和改进负责人已记录。

## 8. 生产恢复和回滚

### 8.1 切换前

1. 宣布事故窗口，冻结所有 writer。
2. 对故障现场做只读快照，保留取证和回滚能力。
3. 选择最近一个已通过恢复演练或完整校验的 generation。
4. 在新的数据库和文件 root 完成第 7 节全部检查。
5. 保存旧配置、旧数据库 endpoint、旧 cover/search/audio mount ID。

### 8.2 切换

1. 先切数据库、cover root、search root 和 workspace/library mount 的配置或挂载引用，不覆盖旧路径。
2. 从 secret manager 重新物化 Annim token、数据库凭据和 catalog secret root。Catalog secret 必须是权限受控的普通文件，路径中不得含符号链接；不要从普通数据 generation 恢复明文 token。
3. 只启动 Annim，等待 ready 并做只读查询。
4. 小范围恢复写流量，观察数据库错误、磁盘和认证失败。
5. 人工检查队列后再启动 catalog worker 和 cover worker。
6. 最后解除维护模式。

### 8.3 回滚触发条件

出现任一情况立即回滚：

- ready 持续失败；
- 数据库 row count 或 foreign key 不一致；
- cover reference hash 失败；
- 元数据关键查询出现损坏或大量缺失；
- 恢复音频 hash/FLAC 测试失败；
- worker 重复产生不可解释的失败或队列状态跳变。

### 8.4 回滚动作

1. 再次阻断写流量并停止 worker、Annim。
2. 保存失败恢复环境的只读 snapshot，不在其中做就地修补。
3. 把配置和挂载引用切回切换前保存的数据库和文件 root。
4. 先启动 Annim 并只读验证，再决定是否启动 worker。
5. 记录恢复窗口内是否已经接受新写入。若有，不要自动双向合并；由数据负责人基于 revision/manifest 做人工补录或重新 ingest。

旧生产路径至少保留到新环境超过观察期并完成一次新 generation + restore drill。清理旧路径属于独立、需人工审批的变更，不是恢复流程的一部分。

旧版本二进制不一定理解新 migration 后的 schema。应用版本回滚前必须在恢复副本验证兼容性；不能把旧二进制直接指向已升级的生产数据库。必要时切回升级前的数据库 generation，并明确处理升级后新增写入。

## 9. 监控和告警

当前实现没有完整 metrics endpoint。生产监控应先组合 supervisor、HTTP probe、结构化日志、数据库只读查询和备份平台指标；不要在文档中假设尚不存在的 Prometheus 指标。

表中的 ingest queue 指 Annim 持久化的 job 状态和部署方外部协调器，不代表仓库当前已有可监控的 ingest daemon。

### 9.1 初始告警基线

| 信号 | 建议初始告警 |
| --- | --- |
| `/health/live` | 连续 2 次失败或 1 分钟不可达 |
| `/health/ready` | 连续 2 次非 2xx；优先检查数据库 |
| 进程 | Annim、catalog worker、cover worker 意外退出或频繁重启 |
| Catalog queue | `running` 的 lease 过期后仍持续超过两个 poll 周期；可执行 run 长时间无成功 |
| Cover queue | `fetching` lease 过期后仍持续；queued age 超过业务 SLA；rejected/retry 比例突增 |
| Ingest queue | 状态长时间无变化；计划/manifest 冲突持续出现 |
| 日志 | `catalog worker cycle failed`、cover worker cycle failure、数据库迁移失败、digest collision |
| 存储 | DB/search/cover/audio 任一卷可用空间低于 20%，或按增长速度不足 7 天 |
| 备份 | 最新成功 generation 超过目标间隔的 2 倍；异地复制、object lock 或 checksum 失败 |
| 恢复演练 | 超过计划周期未完成，或最近一次未满足 RPO/RTO |
| Secret | Apple developer token 或数据库凭据将在 7 天内过期 |
| 安全 | 401/403 突增、Origin 配置漂移、GraphiQL 在生产被启用、服务直接暴露明文 HTTP |

阈值需要在获得真实吞吐和运行时间分布后调整，但 checksum、数据库完整性、备份不可变性和磁盘耗尽类告警应始终为高优先级。

### 9.2 只读队列检查

以下 SQL 不包含 secret，可用于 dashboard 或人工检查：

```sql
SELECT status, COUNT(*)
FROM catalog_sync_run
GROUP BY status
ORDER BY status;

SELECT run_id, status, lease_expires_at, next_attempt_at, attempt_count
FROM catalog_sync_run
WHERE status IN ('queued', 'running')
ORDER BY created_at
LIMIT 50;

SELECT state, COUNT(*)
FROM cover_candidate
GROUP BY state
ORDER BY state;

SELECT candidate_id, state, lease_expires_at, next_attempt_at, attempt_count,
       last_error_code
FROM cover_candidate
WHERE state IN ('queued', 'fetching')
ORDER BY updated_at
LIMIT 50;
```

比较 lease 时使用数据库服务器时间，避免监控主机时钟漂移。不得把 `submitted_url`、`effective_url`、catalog locator、configuration document、secret reference 或数据库 URL 采集到普通监控标签。

## 10. 事故值班清单

### 服务不可用

1. 看 live，再看 ready。
2. 若 live 失败，检查 Annim 进程和监听地址；若 ready 失败，先检查数据库连通性和 migration。
3. 检查磁盘空间和 inode，尤其是 DB、search 和 cover root。
4. 保持 worker 停止，直到 Annim 和数据库稳定。
5. 不通过删除 search、cover 或音频文件来“释放空间”；先扩容或切只读。

### 数据或文件疑似损坏

1. 立即冻结 writer，不做就地修复。
2. 对故障现场做只读 snapshot。
3. 从最近 generation 在隔离环境执行完整 restore drill。
4. 通过 hash、DB integrity 和 row count 决定恢复点。
5. 按第 8 节切换，保留故障现场。

### Backup job 失败

1. 不删除上一个成功 generation。
2. 确认失败命令没有对任何源 audio root 获得写权限。
3. 重新创建新的 generation ID，不复用半成品目录。
4. 在恢复 RPO 前暂停会删除唯一上游副本的 ingest 清理操作。
5. 修复后执行实际 restore，而不仅是重新跑 backup。

## 11. 外部工具依据

- SQLite `.backup` 使用官方 online backup 机制生成一致快照：[SQLite Backup API](https://www.sqlite.org/backup.html)、[SQLite CLI `.backup`](https://www.sqlite.org/cli.html)
- PostgreSQL custom dump 和 restore：[PostgreSQL `pg_dump`](https://www.postgresql.org/docs/current/app-pgdump.html)、[PostgreSQL `pg_restore`](https://www.postgresql.org/docs/current/app-pgrestore.html)
- PostgreSQL 14+ 物理结构检查：[PostgreSQL `pg_amcheck`](https://www.postgresql.org/docs/current/app-pgamcheck.html)
- FLAC `--test` 会解码校验流和已保存的 MD5，但不写出音频：[Xiph FLAC command-line tool](https://xiph.org/flac/documentation_tools_flac.html)

外部工具版本必须与生产数据库版本兼容。每次数据库大版本升级、存储平台迁移、路径变更或 backup agent 变更后，都要重新执行完整恢复演练。
