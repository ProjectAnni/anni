const SESSION_KEY = "annim.adminToken";
const VIEWS = new Set(["workflow", "intake", "artists"]);

const state = {
  token: sessionStorage.getItem(SESSION_KEY) ?? "",
  jobs: [],
  artists: [],
  selectedArtistId: null,
  selectedRelease: null,
  selectedCollection: null,
  catalogSources: [],
  catalogSourcesStatus: "idle",
  catalogSourcesError: "",
  catalogRunsBySource: {},
  catalogHistoryStateBySource: {},
  catalogRefreshTimer: null,
  catalogPollGeneration: 0,
  artistLoadGeneration: 0,
  view: "workflow",
  reviewMutationPending: false,
  catalogMutationPending: false,
};

const elements = {
  serverLamp: document.querySelector("#server-lamp"),
  serverLabel: document.querySelector("#server-label"),
  sessionButton: document.querySelector("#session-button"),
  sessionState: document.querySelector("#session-state"),
  sessionDialog: document.querySelector("#session-dialog"),
  sessionForm: document.querySelector("#session-form"),
  sessionError: document.querySelector("#session-error"),
  adminToken: document.querySelector("#admin-token"),
  forgetSession: document.querySelector("#forget-session"),
  intakeCount: document.querySelector("#intake-count"),
  artistCount: document.querySelector("#artist-count"),
  queueSummary: document.querySelector("#queue-summary"),
  intakeList: document.querySelector("#intake-list"),
  ingestDetail: document.querySelector("#ingest-detail"),
  ingestDetailContent: document.querySelector("#ingest-detail-content"),
  artistSearchForm: document.querySelector("#artist-search-form"),
  artistSearch: document.querySelector("#artist-search"),
  artistList: document.querySelector("#artist-list"),
  collectionSheet: document.querySelector("#collection-sheet"),
  createArtistButton: document.querySelector("#create-artist-button"),
  artistDialog: document.querySelector("#artist-dialog"),
  artistCreateForm: document.querySelector("#artist-create-form"),
  artistFormError: document.querySelector("#artist-form-error"),
  releaseDialog: document.querySelector("#release-dialog"),
  releaseCreateForm: document.querySelector("#release-create-form"),
  releaseFormError: document.querySelector("#release-form-error"),
  releaseDialogArtist: document.querySelector("#release-dialog-artist"),
  releaseManageDialog: document.querySelector("#release-manage-dialog"),
  releaseManageTitle: document.querySelector("#release-manage-title"),
  releaseManageState: document.querySelector("#release-manage-state"),
  collectionCopyForm: document.querySelector("#collection-copy-form"),
  collectionCopyFormError: document.querySelector("#collection-copy-form-error"),
  catalogSourceDialog: document.querySelector("#catalog-source-dialog"),
  catalogSourceForm: document.querySelector("#catalog-source-form"),
  catalogSourceArtist: document.querySelector("#catalog-source-artist"),
  catalogSourceFormError: document.querySelector("#catalog-source-form-error"),
  toastRegion: document.querySelector("#toast-region"),
};

const METADATA_FIELD_OPTIONS = {
  ALBUM: ["TITLE", "EDITION", "ARTIST", "ARTISTS", "RELEASE_DATE", "TRACK_TYPE", "CATALOG", "TAGS"],
  DISC: ["TITLE", "CATALOG", "ARTIST", "ARTISTS", "TRACK_TYPE", "TAGS"],
  TRACK: ["TITLE", "ARTIST", "ARTISTS", "TRACK_TYPE", "TAGS"],
};

const TRACK_TYPE_OPTIONS = [
  ["NORMAL", "Normal · 普通歌曲 / 网播"],
  ["INSTRUMENTAL", "Instrumental · 伴奏"],
  ["ABSOLUTE", "Absolute · 纯音乐"],
  ["DRAMA", "Drama · 广播剧"],
  ["RADIO", "Radio · 广播节目"],
  ["VOCAL", "Vocal · 人声"],
];

const CATALOG_SOURCE_KIND_LABELS = {
  APPLE_MUSIC: "Apple Music",
  RECORD_LABEL: "唱片公司官网",
  ARTIST_WEBSITE: "艺人官网",
  VGMDB: "VGMDB",
  MANUAL: "人工目录",
};

const CATALOG_PROVISIONING_LABELS = {
  READY_TO_QUEUE: "可执行",
  DISABLED: "已停用",
  CREDENTIAL_NOT_CONFIGURED: "服务端凭据未配置",
  CREDENTIAL_BINDING_INVALID: "凭据引用无效",
  ADAPTER_UNAVAILABLE: "Worker adapter 未安装",
};

const CATALOG_RUN_STATUS_LABELS = {
  QUEUED: "排队中",
  RUNNING: "同步中",
  SUCCEEDED: "已完成",
  FAILED: "失败",
  CANCELLED: "已取消",
};

const ACTIVE_CATALOG_RUN_STATES = new Set(["QUEUED", "RUNNING"]);

const EVIDENCE_SOURCE_OPTIONS = [
  ["CD_BOOKLET", "CD Booklet"],
  ["CD_PACKAGING", "CD 盘面 / 包装"],
  ["OFFICIAL_LABEL", "唱片公司官网"],
  ["OFFICIAL_ARTIST", "艺人官网"],
  ["OFFICIAL_STORE", "官方商店"],
  ["STREAMING_SERVICE", "流媒体服务"],
  ["VGMDB", "VGMDB"],
  ["COMMUNITY_SOURCE", "社区来源"],
  ["FILENAME", "文件名"],
  ["DERIVED_INFERENCE", "推断"],
];

const EVIDENCE_METHOD_OPTIONS = {
  CD_BOOKLET: [["MANUAL_TRANSCRIPTION", "人工转录"], ["AUTOMATED_EXTRACTION", "自动提取"]],
  CD_PACKAGING: [["MANUAL_TRANSCRIPTION", "人工转录"], ["AUTOMATED_EXTRACTION", "自动提取"]],
  OFFICIAL_LABEL: [["WEB_IMPORT", "网页导入"], ["MANUAL_TRANSCRIPTION", "人工转录"], ["AUTOMATED_EXTRACTION", "自动提取"]],
  OFFICIAL_ARTIST: [["WEB_IMPORT", "网页导入"], ["MANUAL_TRANSCRIPTION", "人工转录"], ["AUTOMATED_EXTRACTION", "自动提取"]],
  OFFICIAL_STORE: [["WEB_IMPORT", "网页导入"], ["MANUAL_TRANSCRIPTION", "人工转录"], ["AUTOMATED_EXTRACTION", "自动提取"]],
  STREAMING_SERVICE: [["WEB_IMPORT", "网页导入"], ["MANUAL_TRANSCRIPTION", "人工转录"], ["AUTOMATED_EXTRACTION", "自动提取"]],
  VGMDB: [["WEB_IMPORT", "网页导入"], ["MANUAL_TRANSCRIPTION", "人工转录"], ["AUTOMATED_EXTRACTION", "自动提取"]],
  COMMUNITY_SOURCE: [["WEB_IMPORT", "网页导入"], ["MANUAL_TRANSCRIPTION", "人工转录"], ["AUTOMATED_EXTRACTION", "自动提取"]],
  FILENAME: [["MANUAL_TRANSCRIPTION", "人工转录"], ["AUTOMATED_EXTRACTION", "自动提取"], ["INFERENCE", "推断"]],
  DERIVED_INFERENCE: [["INFERENCE", "推断"]],
};

const ABSENT_METADATA_FIELDS = new Set([
  "ALBUM:EDITION",
  "DISC:TITLE",
  "DISC:ARTIST",
  "DISC:ARTISTS",
  "DISC:TRACK_TYPE",
  "TRACK:ARTIST",
  "TRACK:ARTISTS",
  "TRACK:TRACK_TYPE",
]);

const INTAKE_QUERY = `
  query WebIntakeJobs($limit: Int!, $offset: Int!) {
    ingestJobs(limit: $limit, offset: $offset) {
      jobId
      state
      metadataRevision
      approvedRevision
      manifestDigest
      planDigest
      verificationDigest
      rowVersion
      createdAt
      updatedAt
    }
  }
`;

const INGEST_REVIEW_QUERY = `
  query WebIngestReview($jobId: UUID!) {
    ingestJob(jobId: $jobId) {
      jobId
      state
      metadataRevision
      approvedRevision
      manifestDigest
      planDigest
      verificationDigest
      rowVersion
      createdAt
      updatedAt
    }
    ingestMetadataDraft(jobId: $jobId) {
      job {
        jobId
        state
        metadataRevision
        approvedRevision
        rowVersion
        updatedAt
      }
      draft {
        revision
        profile
        trackCounts
        requirementsConfigured
        totalRequired
        acceptedRequired
        complete
        missingFields {
          scope
          disc
          track
          field
        }
        candidates {
          candidateId
          field {
            scope
            disc
            track
            field
          }
          value {
            kind
            text
            date
            trackType
            textList
            textMap {
              key
              value
            }
          }
          evidence {
            sourceKind
            locator
            detail
            method
          }
          confidenceBasisPoints
          decision
          recommended
        }
        createdAt
        updatedAt
      }
    }
  }
`;

const ARTISTS_QUERY = `
  query WebCatalogArtists($search: String, $limit: Int!, $offset: Int!) {
    catalogArtists(search: $search, limit: $limit, offset: $offset) {
      artistId
      displayName
      sortName
      notes
      rowVersion
      updatedAt
    }
  }
`;

const COLLECTION_QUERY = `
  query WebArtistCollection($artistId: UUID!, $limit: Int!, $offset: Int!) {
    catalogArtistCollection(artistId: $artistId, limit: $limit, offset: $offset) {
      artist {
        artistId
        displayName
        sortName
        notes
        rowVersion
      }
      summary {
        total
        missing
        wanted
        acquired
        ingesting
        published
        unavailable
        collected
      }
      releaseTotalCount
      releases {
        releaseId
        title
        edition
        catalog
        releaseDate
        kind
        collectionState
        wanted
        unavailable
        matchedAlbumId
        activeIngestJobId
        notes
        rowVersion
        copies {
          copyId
          sourceKind
          sourceLabel
          codec
          qualityTier
          sampleRateHz
          bitDepth
          channels
          trackCount
          byteLength
          qualityVerified
          notes
          acquiredAt
        }
      }
    }
  }
`;

const CATALOG_SYNC_SOURCES_QUERY = `
  query WebCatalogSyncSources($artistId: UUID!) {
    catalogSyncSources(artistId: $artistId) {
      sourceId
      artistId
      kind
      storefront
      locale
      enabled
      provisioningState
      rowVersion
      createdAt
      updatedAt
    }
  }
`;

const CATALOG_SYNC_SOURCE_QUERY = `
  query WebCatalogSyncSource($sourceId: UUID!) {
    catalogSyncSource(sourceId: $sourceId) {
      sourceId
      artistId
      kind
      storefront
      locale
      enabled
      provisioningState
      rowVersion
      createdAt
      updatedAt
    }
  }
`;

const CATALOG_SYNC_RUNS_QUERY = `
  query WebCatalogSyncRuns($sourceId: UUID!, $limit: Int!, $offset: Int!) {
    catalogSyncRuns(sourceId: $sourceId, limit: $limit, offset: $offset) {
      runId
      sourceId
      status
      coverage
      startedFromRoot
      snapshotComplete
      observedCount
      attemptCount
      nextAttemptAt
      rowVersion
      createdAt
      startedAt
      finishedAt
    }
  }
`;

const CATALOG_SYNC_RUN_QUERY = `
  query WebCatalogSyncRun($runId: UUID!) {
    catalogSyncRun(runId: $runId) {
      runId
      sourceId
      status
      coverage
      startedFromRoot
      snapshotComplete
      observedCount
      attemptCount
      nextAttemptAt
      rowVersion
      createdAt
      startedAt
      finishedAt
    }
  }
`;

const CREATE_CATALOG_ARTIST_MUTATION = `
  mutation WebCreateCatalogArtist($input: CreateCatalogArtistInput!) {
    createCatalogArtist(input: $input) {
      artistId
      displayName
      sortName
      notes
      rowVersion
      updatedAt
    }
  }
`;

const CREATE_CATALOG_RELEASE_MUTATION = `
  mutation WebCreateCatalogRelease($input: CreateCatalogReleaseInput!) {
    createCatalogRelease(input: $input) {
      releaseId
      artistId
      title
      edition
      catalog
      releaseDate
      kind
      collectionState
      rowVersion
      updatedAt
    }
  }
`;

const CREATE_CATALOG_SYNC_SOURCE_MUTATION = `
  mutation WebCreateCatalogSyncSource($input: CreateCatalogSyncSourceInput!) {
    createCatalogSyncSource(input: $input) {
      sourceId
      artistId
      kind
      storefront
      locale
      enabled
      provisioningState
      rowVersion
      createdAt
      updatedAt
    }
  }
`;

const START_CATALOG_SYNC_RUN_MUTATION = `
  mutation WebStartCatalogSyncRun($input: StartCatalogSyncRunInput!) {
    startCatalogSyncRun(input: $input) {
      runId
      sourceId
      status
      coverage
      startedFromRoot
      snapshotComplete
      observedCount
      attemptCount
      nextAttemptAt
      rowVersion
      createdAt
      startedAt
      finishedAt
    }
  }
`;

const SET_CATALOG_SYNC_SOURCE_ENABLED_MUTATION = `
  mutation WebSetCatalogSyncSourceEnabled($input: SetCatalogSyncSourceEnabledInput!) {
    setCatalogSyncSourceEnabled(input: $input) {
      sourceId
      artistId
      kind
      storefront
      locale
      enabled
      provisioningState
      rowVersion
      createdAt
      updatedAt
    }
  }
`;

const EXECUTE_CATALOG_RELEASE_COMMAND_MUTATION = `
  mutation WebExecuteCatalogReleaseCommand($input: ExecuteCatalogReleaseCommandInput!) {
    executeCatalogReleaseCommand(input: $input) {
      releaseId
      artistId
      title
      collectionState
      rowVersion
      copies {
        copyId
        sourceKind
        sourceLabel
        codec
        qualityTier
        sampleRateHz
        bitDepth
        channels
        trackCount
        byteLength
        manifestDigest
        qualityVerified
        ingestJobId
        notes
        acquiredAt
      }
    }
  }
`;

const EDIT_METADATA_MUTATION = `
  mutation WebEditIngestMetadata($input: EditIngestMetadataInput!) {
    editIngestMetadata(input: $input) {
      job {
        jobId
        rowVersion
      }
      draft {
        revision
      }
    }
  }
`;

const EXECUTE_INGEST_COMMAND_MUTATION = `
  mutation WebExecuteIngestCommand($input: ExecuteIngestJobCommandInput!) {
    executeIngestJobCommand(input: $input) {
      jobId
      state
      metadataRevision
      approvedRevision
      rowVersion
    }
  }
`;

const APPROVE_METADATA_MUTATION = `
  mutation WebApproveIngestMetadata($input: IngestMetadataRevisionActionInput!) {
    approveIngestMetadata(input: $input) {
      job {
        jobId
        rowVersion
        approvedRevision
      }
      draft {
        revision
        complete
      }
    }
  }
`;

const REVISE_METADATA_MUTATION = `
  mutation WebReviseIngestMetadata($input: IngestMetadataRevisionActionInput!) {
    reviseIngestMetadata(input: $input) {
      job {
        jobId
        rowVersion
        metadataRevision
      }
      draft {
        revision
      }
    }
  }
`;

class SessionRequiredError extends Error {}

class GraphqlRequestError extends Error {
  constructor(message, code = "GRAPHQL_REQUEST_FAILED") {
    super(message);
    this.code = code;
  }
}

function node(tag, options = {}, children = []) {
  const element = document.createElement(tag);
  if (options.className) element.className = options.className;
  if (options.text !== undefined) element.textContent = String(options.text);
  for (const [name, value] of Object.entries(options.attributes ?? {})) {
    if (value !== null && value !== undefined) {
      element.setAttribute(name, String(value));
    }
  }
  const childList = Array.isArray(children) ? children : [children];
  for (const child of childList) {
    if (child !== null && child !== undefined) element.append(child);
  }
  return element;
}

function replaceSelectOptions(select, options, selected = null) {
  select.replaceChildren(
    ...options.map(([value, label]) =>
      node("option", {
        text: label,
        attributes: { value },
      }),
    ),
  );
  if (selected !== null && options.some(([value]) => value === selected)) {
    select.value = selected;
  }
}

function selectControl(name, options, selected = null) {
  const select = node("select", { attributes: { name } });
  replaceSelectOptions(select, options, selected);
  return select;
}

function labeledControl(label, control, hint = null) {
  const children = [node("span", { text: label }), control];
  if (hint) children.push(node("small", { text: hint }));
  return node("label", { className: "review-form__field" }, children);
}

function parseTrackCounts(value) {
  const parts = value.split(/[,，+\s]+/u).filter(Boolean);
  if (!parts.length) throw new Error("请至少填写一张碟片的轨道数。");
  return parts.map((part) => {
    if (!/^\d+$/.test(part)) throw new Error("轨道数只接受正整数，例如 12, 11。");
    const count = Number(part);
    if (!Number.isSafeInteger(count) || count < 1 || count > 65535) {
      throw new Error("每张碟片的轨道数必须在 1–65535 之间。");
    }
    return count;
  });
}

function metadataValueKind(field) {
  if (field === "RELEASE_DATE") return "DATE";
  if (field === "TRACK_TYPE") return "TRACK_TYPE";
  if (field === "ARTISTS") return "TEXT_MAP";
  if (field === "TAGS") return "TEXT_LIST";
  return "TEXT";
}

function textOrDash(value) {
  return value === null || value === undefined || value === "" ? "—" : String(value);
}

function humanizeEnum(value) {
  if (!value) return "—";
  return value
    .toLowerCase()
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatCodec(value) {
  return ["FLAC", "WAV", "ALAC", "AAC", "MP3", "OPUS"].includes(value)
    ? value
    : humanizeEnum(value);
}

function compactId(value) {
  if (!value || value.length < 15) return textOrDash(value);
  return `${value.slice(0, 8)}…${value.slice(-6)}`;
}

function formatDateTime(value) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function formatField(field) {
  const location = [
    field.scope,
    field.disc ? `D${field.disc}` : null,
    field.track ? `T${field.track}` : null,
  ]
    .filter(Boolean)
    .join(" · ");
  return `${location} / ${field.field}`;
}

function formatCandidateValue(value) {
  switch (value.kind) {
    case "TEXT":
      return textOrDash(value.text);
    case "DATE":
      return textOrDash(value.date);
    case "TRACK_TYPE":
      return humanizeEnum(value.trackType);
    case "TEXT_LIST":
      return value.textList.length ? value.textList.join(" / ") : "（空列表）";
    case "TEXT_MAP":
      return value.textMap.length
        ? value.textMap.map((entry) => `${entry.key}: ${entry.value}`).join(" / ")
        : "（空映射）";
    case "ABSENT":
      return "明确缺省";
    default:
      return "不支持的值类型";
  }
}

function showToast(message, type = "info") {
  const toast = node("div", {
    className: `toast${type === "error" ? " toast--error" : ""}`,
    text: message,
  });
  elements.toastRegion.append(toast);
  window.setTimeout(() => toast.remove(), 4200);
}

function renderMessage(target, kind, title, detail) {
  target.replaceChildren(
    node("div", { className: `${kind}-state` }, [
      node("strong", { text: title }),
      node("p", { text: detail }),
    ]),
  );
}

function updateSessionUi() {
  const connected = Boolean(state.token);
  elements.sessionState.textContent = connected ? "当前标签页已连接" : "未连接";
  elements.sessionButton.classList.toggle("is-connected", connected);
}

function openSessionDialog(message = "") {
  elements.sessionError.textContent = message;
  elements.adminToken.value = "";
  if (!elements.sessionDialog.open) elements.sessionDialog.showModal();
  window.setTimeout(() => elements.adminToken.focus(), 0);
}

function stopCatalogRefresh() {
  if (state.catalogRefreshTimer !== null) {
    window.clearTimeout(state.catalogRefreshTimer);
    state.catalogRefreshTimer = null;
  }
  state.catalogPollGeneration += 1;
}

function clearSession({ notify = true } = {}) {
  stopCatalogRefresh();
  sessionStorage.removeItem(SESSION_KEY);
  state.token = "";
  state.jobs = [];
  state.artists = [];
  state.selectedArtistId = null;
  state.selectedRelease = null;
  state.selectedCollection = null;
  state.catalogSources = [];
  state.catalogSourcesStatus = "idle";
  state.catalogSourcesError = "";
  state.catalogRunsBySource = {};
  state.catalogHistoryStateBySource = {};
  state.artistLoadGeneration += 1;
  updateSessionUi();
  elements.intakeCount.textContent = "—";
  elements.artistCount.textContent = "—";
  renderMessage(elements.intakeList, "empty", "连接 Annim 后读取队列", "令牌只保存在当前浏览器标签页。");
  renderMessage(elements.artistList, "empty", "尚未读取艺人", "建立管理会话后读取 Artist 列表。");
  renderMessage(elements.collectionSheet, "empty", "选择一位艺人", "查看发行列表、来源与音质。");
  if (notify) showToast("当前管理会话已清除");
}

async function graphql(query, variables = {}, tokenOverride = null) {
  const token = tokenOverride ?? state.token;
  if (!token) throw new SessionRequiredError("需要管理会话");

  let response;
  try {
    response = await fetch("/", {
      method: "POST",
      cache: "no-store",
      credentials: "same-origin",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ query, variables }),
    });
  } catch {
    throw new GraphqlRequestError("无法连接 Annim，请检查服务状态。", "NETWORK_ERROR");
  }

  if (response.status === 401) {
    if (tokenOverride === null) clearSession({ notify: false });
    throw new SessionRequiredError("管理令牌无效或已更换。");
  }
  if (!response.ok) {
    throw new GraphqlRequestError(`Annim 返回 HTTP ${response.status}。`, "HTTP_ERROR");
  }

  let payload;
  try {
    payload = await response.json();
  } catch {
    throw new GraphqlRequestError("Annim 返回了无法解析的响应。", "INVALID_RESPONSE");
  }
  if (payload.errors?.length) {
    const error = payload.errors[0];
    const code = error.extensions?.code ?? "GRAPHQL_ERROR";
    throw new GraphqlRequestError(error.message || "GraphQL 请求失败。", code);
  }
  return payload.data;
}

async function checkServer() {
  try {
    const response = await fetch("/health/ready", {
      cache: "no-store",
      credentials: "same-origin",
    });
    if (!response.ok) throw new Error("not ready");
    elements.serverLamp.className = "state-lamp is-ready";
    elements.serverLabel.textContent = "Annim 已就绪";
  } catch {
    elements.serverLamp.className = "state-lamp is-down";
    elements.serverLabel.textContent = "Annim 暂不可用";
  }
}

function setView(view) {
  const safeView = VIEWS.has(view) ? view : "workflow";
  state.view = safeView;
  if (safeView !== "artists") stopCatalogRefresh();
  for (const section of document.querySelectorAll("[data-view]")) {
    section.hidden = section.dataset.view !== safeView;
  }
  for (const link of document.querySelectorAll("[data-view-link]")) {
    const active = link.dataset.viewLink === safeView;
    link.classList.toggle("is-active", active);
    if (active) link.setAttribute("aria-current", "page");
    else link.removeAttribute("aria-current");
  }
  document.querySelector("#main-content")?.focus({ preventScroll: true });

  if (safeView === "intake") void ensureIntakeLoaded();
  if (safeView === "artists") {
    void ensureArtistsLoaded();
    if (state.selectedArtistId) void selectArtist(state.selectedArtistId, { background: true });
  }
}

function routeFromHash() {
  const view = window.location.hash.slice(1).split("/")[0];
  setView(view || "workflow");
}

function stateLabel(value) {
  return (
    {
      CREATED: "已创建",
      REVIEWING: "审阅中",
      PLANNED: "计划已冻结",
      EXECUTING: "执行中",
      VERIFYING: "验证中",
      READY_TO_COMMIT: "等待提交",
      COMMITTING: "提交中",
      PUBLISHED: "已发布",
      QUARANTINED: "已隔离",
      CANCELLED: "已取消",
    }[value] ?? humanizeEnum(value)
  );
}

function renderQueueSummary(jobs) {
  const groups = [
    ["全部", jobs.length],
    ["待审阅", jobs.filter((job) => ["CREATED", "REVIEWING"].includes(job.state)).length],
    ["处理中", jobs.filter((job) => ["PLANNED", "EXECUTING", "VERIFYING", "READY_TO_COMMIT", "COMMITTING"].includes(job.state)).length],
    ["已发布", jobs.filter((job) => job.state === "PUBLISHED").length],
    ["需处理", jobs.filter((job) => job.state === "QUARANTINED").length],
  ];
  elements.queueSummary.replaceChildren(
    ...groups.map(([label, value]) =>
      node("div", { className: "summary-cell" }, [
        node("span", { text: label }),
        node("strong", { text: value }),
      ]),
    ),
  );
}

function renderIntake(jobs) {
  elements.intakeCount.textContent = String(jobs.length);
  renderQueueSummary(jobs);
  if (!jobs.length) {
    renderMessage(elements.intakeList, "empty", "当前没有入库任务", "从 staging 接收新任务后会显示在这里。");
    return;
  }

  const rows = jobs.map((job) => {
    const row = node(
      "button",
      {
        className: "queue-row",
        attributes: { type: "button", "aria-label": `查看入库任务 ${job.jobId}` },
      },
      [
        node("span", { className: "queue-row__id", text: job.jobId }),
        node("span", { className: "queue-row__state", text: stateLabel(job.state) }),
        node("span", { className: "queue-row__time", text: formatDateTime(job.updatedAt) }),
        node("span", { className: "queue-row__revision", text: `rev ${job.metadataRevision}` }),
      ],
    );
    row.addEventListener("click", () => void openIngestDetail(job.jobId));
    return row;
  });
  elements.intakeList.replaceChildren(...rows);
}

async function loadIntake() {
  if (!state.token) {
    openSessionDialog("查看入库队列需要管理令牌。");
    throw new SessionRequiredError();
  }
  renderMessage(elements.intakeList, "loading", "正在读取入库队列", "读取 Annim 中最新的任务状态。");
  const data = await graphql(INTAKE_QUERY, { limit: 200, offset: 0 });
  state.jobs = data.ingestJobs;
  renderIntake(state.jobs);
}

async function ensureIntakeLoaded(force = false) {
  if (!force && state.jobs.length) return;
  try {
    await loadIntake();
  } catch (error) {
    if (error instanceof SessionRequiredError) return;
    renderMessage(elements.intakeList, "error", "无法读取入库队列", error.message);
  }
}

function reviewIsEditable(job, draft) {
  return job.state === "REVIEWING" && job.approvedRevision !== draft.revision;
}

async function runReviewMutation({ query, input, jobId, successMessage }) {
  if (state.reviewMutationPending) return;
  state.reviewMutationPending = true;
  elements.ingestDetailContent.setAttribute("aria-busy", "true");
  const controls = Array.from(
    elements.ingestDetailContent.querySelectorAll("button, input, select, textarea"),
  );
  const priorDisabledStates = controls.map((control) => control.disabled);
  for (const control of controls) {
    control.disabled = true;
  }
  try {
    await graphql(query, { input });
    showToast(successMessage);
    await openIngestDetail(jobId);
    await ensureIntakeLoaded(true);
  } catch (error) {
    const conflict = error instanceof GraphqlRequestError && error.code === "INGEST_JOB_CONFLICT";
    showToast(conflict ? "审阅稿已被其他会话更新，已重新载入。" : error.message, "error");
    if (error instanceof SessionRequiredError) {
      openSessionDialog(error.message);
    } else {
      await openIngestDetail(jobId);
    }
  } finally {
    state.reviewMutationPending = false;
    elements.ingestDetailContent.removeAttribute("aria-busy");
    controls.forEach((control, index) => {
      if (control.isConnected) control.disabled = priorDisabledStates[index];
    });
  }
}

function candidateNode(candidate, job, draft) {
  const decision = candidate.recommended
    ? `${humanizeEnum(candidate.decision)} · 推荐`
    : humanizeEnum(candidate.decision);
  const evidence = [
    humanizeEnum(candidate.evidence.sourceKind),
    humanizeEnum(candidate.evidence.method),
    candidate.evidence.locator,
    candidate.evidence.detail,
    `${(candidate.confidenceBasisPoints / 100).toFixed(2)}%`,
  ].filter(Boolean);
  const article = node("article", { className: "candidate-row" }, [
    node("div", { className: "candidate-row__head" }, [
      node("span", { className: "candidate-row__field", text: formatField(candidate.field) }),
      node("span", { className: "candidate-row__decision", text: decision }),
    ]),
    node("div", { className: "candidate-row__value", text: formatCandidateValue(candidate.value) }),
    node("div", {
      className: "candidate-row__source",
      text: evidence.join(" · "),
    }),
  ]);

  if (reviewIsEditable(job, draft)) {
    const accept = node("button", {
      className: "candidate-action candidate-action--accept",
      text: candidate.decision === "ACCEPTED" ? "已接受" : "接受",
      attributes: {
        type: "button",
        disabled: candidate.decision === "ACCEPTED" ? "" : null,
      },
    });
    const reject = node("button", {
      className: "candidate-action candidate-action--reject",
      text: candidate.decision === "REJECTED" ? "已拒绝" : "拒绝",
      attributes: {
        type: "button",
        disabled: candidate.decision === "REJECTED" ? "" : null,
      },
    });
    accept.addEventListener("click", () =>
      void runReviewMutation({
        query: EDIT_METADATA_MUTATION,
        input: {
          jobId: job.jobId,
          expectedRowVersion: job.rowVersion,
          expectedRevision: draft.revision,
          edit: { acceptCandidate: { candidateId: candidate.candidateId } },
        },
        jobId: job.jobId,
        successMessage: "候选值已接受",
      }),
    );
    reject.addEventListener("click", () =>
      void runReviewMutation({
        query: EDIT_METADATA_MUTATION,
        input: {
          jobId: job.jobId,
          expectedRowVersion: job.rowVersion,
          expectedRevision: draft.revision,
          edit: { rejectCandidate: { candidateId: candidate.candidateId } },
        },
        jobId: job.jobId,
        successMessage: "候选值已拒绝",
      }),
    );
    article.append(node("div", { className: "candidate-row__actions" }, [accept, reject]));
  }
  return article;
}

function reviewConfigurationNode(job, draft) {
  const profile = selectControl(
    "profile",
    [
      ["CD", "CD · 以 Booklet / 包装为最高权威"],
      ["STREAMING", "Streaming · 以官方流媒体版本为对象"],
    ],
    "CD",
  );
  const trackCounts = node("input", {
    attributes: {
      name: "trackCounts",
      type: "text",
      inputmode: "numeric",
      placeholder: "例如 12, 11",
      autocomplete: "off",
      required: "",
    },
  });
  const error = node("p", { className: "form-error", attributes: { "aria-live": "polite" } });
  const submit = node("button", {
    className: "primary-action",
    text: "建立审阅布局",
    attributes: { type: "submit" },
  });
  const form = node("form", { className: "review-form review-form--configuration" }, [
    node("div", { className: "review-form__heading" }, [
      node("strong", { text: "先定义本次发行的物理布局" }),
      node("p", { text: "布局一旦开始录入候选就不能在当前 revision 内改写；需要变更时创建新 revision。" }),
    ]),
    node("div", { className: "review-form__grid" }, [
      labeledControl("来源 Profile", profile),
      labeledControl("每张碟片的轨道数", trackCounts, "多碟用逗号或加号分隔"),
    ]),
    error,
    submit,
  ]);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    try {
      error.textContent = "";
      const counts = parseTrackCounts(trackCounts.value);
      void runReviewMutation({
        query: EDIT_METADATA_MUTATION,
        input: {
          jobId: job.jobId,
          expectedRowVersion: job.rowVersion,
          expectedRevision: draft.revision,
          edit: { configureReview: { profile: profile.value, trackCounts: counts } },
        },
        jobId: job.jobId,
        successMessage: "元数据审阅布局已建立",
      });
    } catch (validationError) {
      error.textContent = validationError.message;
    }
  });
  return form;
}

function buildCandidateValue(kind, control, explicitlyAbsent) {
  if (explicitlyAbsent) return { absent: "USE" };
  if (kind === "TEXT") {
    if (control.value.length === 0) throw new Error("元数据原文不能为空。");
    // Exact source text is the value: do not trim or Unicode-normalize it.
    return { text: control.value };
  }
  if (kind === "DATE") {
    if (control.value.length === 0) throw new Error("请按 YYYY、YYYY-MM 或 YYYY-MM-DD 填写日期。");
    return { date: control.value };
  }
  if (kind === "TRACK_TYPE") return { trackType: control.value };
  if (kind === "TEXT_LIST") {
    const values = control.value.split("\n").filter((value) => value !== "");
    if (!values.length) throw new Error("请至少填写一项；每行作为一个原样值提交。");
    return { textList: { values } };
  }
  const entries = [];
  const keys = new Set();
  for (const line of control.value.split("\n").filter((value) => value !== "")) {
    const separator = line.indexOf("=");
    if (separator <= 0) throw new Error("多艺人映射每行使用 key=value，例如 vocal=歌手名。");
    const key = line.slice(0, separator);
    if (keys.has(key)) throw new Error(`多艺人映射包含重复 key：${key}`);
    keys.add(key);
    entries.push({ key, value: line.slice(separator + 1) });
  }
  if (!entries.length) throw new Error("请至少填写一条 key=value 映射。");
  return { textMap: { entries } };
}

function candidateEditorNode(job, draft) {
  const initial = draft.missingFields[0] ?? {
    scope: "ALBUM",
    disc: null,
    track: null,
    field: "TITLE",
  };
  const scope = selectControl(
    "scope",
    [
      ["ALBUM", "Album"],
      ["DISC", "Disc"],
      ["TRACK", "Track"],
    ],
    initial.scope,
  );
  const field = selectControl("field", [], initial.field);
  const disc = node("input", {
    attributes: {
      name: "disc",
      type: "number",
      min: "1",
      value: initial.disc ?? 1,
      required: "",
    },
  });
  const track = node("input", {
    attributes: {
      name: "track",
      type: "number",
      min: "1",
      value: initial.track ?? 1,
      required: "",
    },
  });
  const discField = labeledControl("Disc", disc);
  const trackField = labeledControl("Track", track);
  const valueHost = node("div", { className: "review-form__value" });
  const explicitlyAbsent = node("input", {
    attributes: { name: "absent", type: "checkbox" },
  });
  const absentField = node("label", { className: "review-form__absent" }, [
    explicitlyAbsent,
    node("span", { text: "此字段在原始来源中明确缺省" }),
  ]);
  let valueControl;
  let valueKind;

  function syncPositions() {
    const needsDisc = scope.value !== "ALBUM";
    const needsTrack = scope.value === "TRACK";
    discField.hidden = !needsDisc;
    disc.disabled = !needsDisc;
    disc.max = String(draft.trackCounts.length);
    if (Number(disc.value) > draft.trackCounts.length) disc.value = "1";
    const discIndex = Math.max(0, Number(disc.value) - 1);
    const trackMaximum = draft.trackCounts[discIndex] ?? 1;
    trackField.hidden = !needsTrack;
    track.disabled = !needsTrack;
    track.max = String(trackMaximum);
    if (Number(track.value) > trackMaximum) track.value = "1";
  }

  function renderValueControl() {
    valueKind = metadataValueKind(field.value);
    if (valueKind === "TRACK_TYPE") {
      valueControl = selectControl("value", TRACK_TYPE_OPTIONS, "NORMAL");
    } else if (valueKind === "TEXT_LIST" || valueKind === "TEXT_MAP") {
      valueControl = node("textarea", {
        attributes: {
          name: "value",
          rows: "4",
          placeholder: valueKind === "TEXT_LIST" ? "每行一个 tag" : "每行 key=value",
          required: "",
        },
      });
    } else {
      valueControl = node("input", {
        attributes: {
          name: "value",
          type: "text",
          placeholder: valueKind === "DATE" ? "YYYY / YYYY-MM / YYYY-MM-DD" : "严格按原文录入",
          autocomplete: "off",
          required: "",
        },
      });
    }
    const kindLabel = {
      TEXT: "原文值",
      DATE: "发行日期",
      TRACK_TYPE: "音乐类型",
      TEXT_LIST: "标签列表",
      TEXT_MAP: "多艺人职责映射",
    }[valueKind];
    valueHost.replaceChildren(
      labeledControl(
        kindLabel,
        valueControl,
        valueKind === "TEXT" ? "不会 trim，也不会做 Unicode 归一化" : null,
      ),
    );
    const absentAllowed = ABSENT_METADATA_FIELDS.has(`${scope.value}:${field.value}`);
    absentField.hidden = !absentAllowed;
    explicitlyAbsent.disabled = !absentAllowed;
    if (!absentAllowed) explicitlyAbsent.checked = false;
    valueControl.disabled = explicitlyAbsent.checked;
  }

  function syncFields(selected = null) {
    const options = METADATA_FIELD_OPTIONS[scope.value].map((value) => [value, humanizeEnum(value)]);
    replaceSelectOptions(field, options, selected);
    syncPositions();
    renderValueControl();
  }

  scope.addEventListener("change", () => syncFields());
  field.addEventListener("change", renderValueControl);
  disc.addEventListener("change", syncPositions);
  explicitlyAbsent.addEventListener("change", () => {
    valueControl.disabled = explicitlyAbsent.checked;
  });
  syncFields(initial.field);

  const defaultSource = draft.profile === "CD" ? "CD_BOOKLET" : "STREAMING_SERVICE";
  const sourceKind = selectControl("sourceKind", EVIDENCE_SOURCE_OPTIONS, defaultSource);
  const method = selectControl("method", EVIDENCE_METHOD_OPTIONS[defaultSource]);
  sourceKind.addEventListener("change", () => {
    replaceSelectOptions(method, EVIDENCE_METHOD_OPTIONS[sourceKind.value]);
  });
  const locator = node("input", {
    attributes: {
      name: "locator",
      type: "text",
      value: draft.profile === "CD" ? "booklet.pdf#page=" : "",
      placeholder: "可复核的页码、URL 或文件定位",
      autocomplete: "off",
      required: "",
    },
  });
  const detail = node("input", {
    attributes: {
      name: "detail",
      type: "text",
      placeholder: "可选：位置或转录说明",
      autocomplete: "off",
    },
  });
  const confidence = node("input", {
    attributes: {
      name: "confidence",
      type: "number",
      min: "0",
      max: "10000",
      value: draft.profile === "CD" ? "10000" : "9000",
      required: "",
    },
  });
  const error = node("p", { className: "form-error", attributes: { "aria-live": "polite" } });
  const form = node("form", { className: "review-form review-form--candidate" }, [
    node("div", { className: "review-form__grid review-form__grid--path" }, [
      labeledControl("Scope", scope),
      discField,
      trackField,
      labeledControl("字段", field),
    ]),
    valueHost,
    absentField,
    node("div", { className: "review-form__grid" }, [
      labeledControl("证据来源", sourceKind),
      labeledControl("采集方法", method),
      labeledControl("定位信息", locator),
      labeledControl("证据详情", detail),
      labeledControl("置信度（basis points）", confidence, "0–10000"),
    ]),
    error,
    node("button", {
      className: "primary-action",
      text: "添加候选证据",
      attributes: { type: "submit" },
    }),
  ]);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    try {
      error.textContent = "";
      if (!locator.value.trim()) throw new Error("证据定位信息不能为空。");
      const confidenceBasisPoints = Number(confidence.value);
      if (
        !Number.isSafeInteger(confidenceBasisPoints) ||
        confidenceBasisPoints < 0 ||
        confidenceBasisPoints > 10000
      ) {
        throw new Error("置信度必须是 0–10000 之间的整数。");
      }
      const fieldInput = { scope: scope.value, field: field.value };
      if (scope.value !== "ALBUM") fieldInput.disc = Number(disc.value);
      if (scope.value === "TRACK") fieldInput.track = Number(track.value);
      const evidence = {
        sourceKind: sourceKind.value,
        locator: locator.value,
        method: method.value,
      };
      if (detail.value !== "") evidence.detail = detail.value;
      void runReviewMutation({
        query: EDIT_METADATA_MUTATION,
        input: {
          jobId: job.jobId,
          expectedRowVersion: job.rowVersion,
          expectedRevision: draft.revision,
          edit: {
            addCandidate: {
              field: fieldInput,
              value: buildCandidateValue(valueKind, valueControl, explicitlyAbsent.checked),
              evidence,
              confidenceBasisPoints,
            },
          },
        },
        jobId: job.jobId,
        successMessage: "候选证据已添加",
      });
    } catch (validationError) {
      error.textContent = validationError.message;
    }
  });

  const editor = node("details", { className: "candidate-editor" }, [
    node("summary", { text: "录入 Booklet / 其他来源候选" }),
    form,
  ]);
  editor.open = draft.candidates.length === 0;
  return editor;
}

function reviewActions(job, draft) {
  const actions = node("div", { className: "review-actions" });
  const currentApproved = job.approvedRevision === draft.revision;

  if (job.state === "CREATED") {
    const begin = node("button", {
      className: "primary-action",
      text: "开始元数据审阅",
      attributes: { type: "button" },
    });
    begin.addEventListener("click", () =>
      void runReviewMutation({
        query: EXECUTE_INGEST_COMMAND_MUTATION,
        input: {
          jobId: job.jobId,
          expectedRowVersion: job.rowVersion,
          command: { beginReview: "EXECUTE" },
        },
        jobId: job.jobId,
        successMessage: "任务已进入元数据审阅阶段",
      }),
    );
    actions.append(begin);
  }

  if (reviewIsEditable(job, draft) && draft.requirementsConfigured) {
    const approve = node("button", {
      className: "primary-action",
      text: draft.complete ? "批准此 revision" : "必填项尚未完成",
      attributes: { type: "button", disabled: draft.complete ? null : "" },
    });
    approve.addEventListener("click", () =>
      void runReviewMutation({
        query: APPROVE_METADATA_MUTATION,
        input: {
          jobId: job.jobId,
          expectedRowVersion: job.rowVersion,
          expectedRevision: draft.revision,
        },
        jobId: job.jobId,
        successMessage: `Metadata revision ${draft.revision} 已批准`,
      }),
    );
    actions.append(approve);
  }

  if (reviewIsEditable(job, draft) && !draft.requirementsConfigured) {
    actions.append(node("p", { text: "先建立 CD / Streaming profile 与多碟轨道布局。" }));
  }

  if (
    currentApproved &&
    ["REVIEWING", "PLANNED", "READY_TO_COMMIT", "QUARANTINED"].includes(job.state)
  ) {
    const revise = node("button", {
      className: "secondary-action",
      text: "创建新 revision",
      attributes: { type: "button" },
    });
    revise.addEventListener("click", () =>
      void runReviewMutation({
        query: REVISE_METADATA_MUTATION,
        input: {
          jobId: job.jobId,
          expectedRowVersion: job.rowVersion,
          expectedRevision: draft.revision,
        },
        jobId: job.jobId,
        successMessage: "已创建新的元数据 revision",
      }),
    );
    actions.append(revise);
  }

  if (!actions.childElementCount) {
    actions.append(
      node("p", {
        text: currentApproved
          ? "当前 revision 已冻结；任务进入安全检查点后才能修订。"
          : "当前任务状态不允许编辑元数据。",
      }),
    );
  }
  return actions;
}

function missingFieldsNode(draft) {
  return node("details", { className: "missing-fields" }, [
    node("summary", { text: `缺失字段 ${draft.missingFields.length}` }),
    node(
      "ul",
      {},
      draft.missingFields.map((field) => node("li", { text: formatField(field) })),
    ),
  ]);
}

function renderIngestDetail(data) {
  const job = data.ingestJob;
  if (!job) {
    renderMessage(elements.ingestDetailContent, "error", "任务不存在", "它可能已被其他管理会话删除或迁移。");
    return;
  }
  const draft = data.ingestMetadataDraft?.draft;
  const metadata = [
    ["状态", stateLabel(job.state)],
    ["Row version", job.rowVersion],
    ["Metadata revision", job.metadataRevision],
    ["Approved revision", textOrDash(job.approvedRevision)],
    ["Manifest", compactId(job.manifestDigest)],
    ["Plan", compactId(job.planDigest)],
    ["Verification", compactId(job.verificationDigest)],
    ["更新时间", formatDateTime(job.updatedAt)],
  ];

  const content = node("div", { className: "drawer-content" }, [
    node("span", { className: "eyebrow", text: "Ingest review" }),
    node("h2", { text: compactId(job.jobId) }),
    node(
      "div",
      { className: "drawer-metadata" },
      metadata.map(([label, value]) =>
        node("div", {}, [node("span", { text: label }), node("strong", { text: value })]),
      ),
    ),
  ]);

  if (!draft) {
    content.append(
      node("div", { className: "empty-state" }, [
        node("strong", { text: "尚未建立元数据审阅稿" }),
        node("p", { text: "任务进入 Reviewing 并配置布局后，候选证据会显示在这里。" }),
      ]),
    );
  } else {
    const accepted = draft.acceptedRequired ?? "0";
    const total = draft.totalRequired ?? "—";
    content.append(
      node("div", { className: "collection-heading" }, [
        node("div", {}, [
          node("h2", { text: `Metadata revision ${draft.revision}` }),
          node("p", {
            text: draft.requirementsConfigured
              ? `${humanizeEnum(draft.profile)} · ${draft.trackCounts.join(" + ")} tracks · 必填完成 ${accepted} / ${total} · ${draft.complete ? "可以批准" : "仍有缺失"}`
              : "尚未配置 CD / 流媒体审阅 profile 与轨道布局",
          }),
        ]),
      ]),
      reviewActions(job, draft),
      ...(reviewIsEditable(job, draft) && !draft.requirementsConfigured
        ? [reviewConfigurationNode(job, draft)]
        : []),
      ...(reviewIsEditable(job, draft) && draft.requirementsConfigured
        ? [candidateEditorNode(job, draft)]
        : []),
      ...(draft.missingFields.length ? [missingFieldsNode(draft)] : []),
      node(
        "div",
        { className: "candidate-list" },
        draft.candidates.length
          ? draft.candidates.map((candidate) => candidateNode(candidate, job, draft))
          : [
              node("div", { className: "empty-state" }, [
                node("strong", { text: "没有候选证据" }),
                node("p", { text: "先录入 Booklet，再让网络 Agent 补充缺失字段。" }),
              ]),
            ],
      ),
    );
  }
  elements.ingestDetailContent.replaceChildren(content);
}

async function openIngestDetail(jobId) {
  elements.ingestDetail.hidden = false;
  renderMessage(elements.ingestDetailContent, "loading", "正在读取审阅稿", "加载候选值与证据来源。");
  try {
    const data = await graphql(INGEST_REVIEW_QUERY, { jobId });
    renderIngestDetail(data);
  } catch (error) {
    if (error instanceof SessionRequiredError) openSessionDialog(error.message);
    renderMessage(elements.ingestDetailContent, "error", "无法读取审阅稿", error.message);
  }
}

function renderArtists(artists) {
  elements.artistCount.textContent = String(artists.length);
  if (!artists.length) {
    renderMessage(elements.artistList, "empty", "没有匹配的艺人", "调整搜索词，或先在 Annim 中建立 Artist。");
    return;
  }
  const buttons = artists.map((artist, index) => {
    const button = node(
      "button",
      {
        className: `artist-button${artist.artistId === state.selectedArtistId ? " is-active" : ""}`,
        attributes: { type: "button" },
      },
      [
        node("strong", { text: artist.displayName }),
        node("span", { text: String(index + 1).padStart(2, "0") }),
      ],
    );
    button.addEventListener("click", () => void selectArtist(artist.artistId));
    return button;
  });
  elements.artistList.replaceChildren(...buttons);
}

async function loadArtists(search = null) {
  if (!state.token) {
    openSessionDialog("查看艺人拉表需要管理令牌。");
    throw new SessionRequiredError();
  }
  renderMessage(elements.artistList, "loading", "正在读取艺人", "读取 Artist 索引。");
  const data = await graphql(ARTISTS_QUERY, {
    search: search || null,
    limit: 200,
    offset: 0,
  });
  state.artists = data.catalogArtists;
  renderArtists(state.artists);
}

async function ensureArtistsLoaded(force = false) {
  if (!force && state.artists.length) return;
  try {
    await loadArtists();
  } catch (error) {
    if (error instanceof SessionRequiredError) return;
    renderMessage(elements.artistList, "error", "无法读取艺人", error.message);
  }
}

function optionalExactFormValue(form, name) {
  const value = form.elements.namedItem(name).value;
  return value === "" ? null : value;
}

async function runCatalogOperation(container, errorTarget, operation) {
  if (state.catalogMutationPending) {
    errorTarget.dataset.errorCode = "CATALOG_MUTATION_BUSY";
    errorTarget.textContent = "另一项目录写操作仍在进行，请等待其完成。";
    return null;
  }
  state.catalogMutationPending = true;
  delete errorTarget.dataset.errorCode;
  errorTarget.textContent = "正在保存…";
  const controls = Array.from(container.querySelectorAll("button, input, select, textarea"));
  const priorDisabledStates = controls.map((control) => control.disabled);
  controls.forEach((control) => {
    control.disabled = true;
  });
  try {
    const data = await operation();
    errorTarget.textContent = "";
    return data;
  } catch (error) {
    errorTarget.textContent = error.message;
    errorTarget.dataset.errorCode = error.code ?? "UNKNOWN_ERROR";
    if (error instanceof SessionRequiredError) {
      container.closest("dialog")?.close();
      openSessionDialog(error.message);
    }
    return null;
  } finally {
    state.catalogMutationPending = false;
    controls.forEach((control, index) => {
      if (control.isConnected) control.disabled = priorDisabledStates[index];
    });
  }
}

async function runCatalogMutation(form, errorTarget, query, input) {
  return runCatalogOperation(form, errorTarget, () => graphql(query, { input }));
}

function openArtistDialog() {
  if (!state.token) {
    openSessionDialog("新建 Artist 需要管理会话。");
    return;
  }
  elements.artistCreateForm.reset();
  elements.artistFormError.textContent = "";
  elements.artistDialog.showModal();
  window.setTimeout(() => elements.artistCreateForm.elements.namedItem("displayName").focus(), 0);
}

function openReleaseDialog(artist) {
  if (!state.token) {
    openSessionDialog("登记发行需要管理会话。");
    return;
  }
  elements.releaseCreateForm.reset();
  elements.releaseFormError.textContent = "";
  elements.releaseDialogArtist.textContent = `发行会归入 ${artist.displayName} 的拉表。`;
  elements.releaseDialog.showModal();
  window.setTimeout(() => elements.releaseCreateForm.elements.namedItem("title").focus(), 0);
}

function openCatalogSourceDialog(artist) {
  if (!state.token) {
    openSessionDialog("绑定官方目录需要管理会话。");
    return;
  }
  elements.catalogSourceForm.reset();
  elements.catalogSourceFormError.textContent = "";
  elements.catalogSourceArtist.textContent = `Apple Music 信源会归入 ${artist.displayName}。`;
  elements.catalogSourceDialog.showModal();
  window.setTimeout(() => elements.catalogSourceForm.elements.namedItem("locator").focus(), 0);
}

function openReleaseManager(release) {
  if (!state.token) {
    openSessionDialog("管理发行需要管理会话。");
    return;
  }
  state.selectedRelease = release;
  elements.collectionCopyForm.reset();
  elements.collectionCopyFormError.textContent = "";
  elements.releaseManageTitle.textContent = release.title;
  elements.releaseManageState.className = collectionStateClass(release.collectionState);
  elements.releaseManageState.textContent = humanizeEnum(release.collectionState);
  const allowedStates = {
    markWanted: ["MISSING", "UNAVAILABLE"],
    markMissing: ["WANTED", "UNAVAILABLE"],
    markUnavailable: ["MISSING", "WANTED"],
  };
  for (const button of elements.collectionCopyForm.querySelectorAll("[data-release-command]")) {
    button.disabled = !allowedStates[button.dataset.releaseCommand].includes(
      release.collectionState,
    );
  }
  elements.collectionCopyForm.querySelector("[data-register-copy]").disabled = ![
    "MISSING",
    "WANTED",
    "UNAVAILABLE",
    "ACQUIRED",
  ].includes(release.collectionState);
  elements.releaseManageDialog.showModal();
}

function optionalPositiveIntegerFormValue(form, name, label, maximum = Number.MAX_SAFE_INTEGER) {
  const raw = form.elements.namedItem(name).value;
  if (raw === "") return null;
  if (!/^\d+$/.test(raw)) throw new Error(`${label}必须是正整数。`);
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value < 1 || value > maximum) {
    throw new Error(`${label}超出支持范围。`);
  }
  return value;
}

function optionalByteLengthFormValue(form) {
  const raw = form.elements.namedItem("byteLength").value;
  if (raw === "") return null;
  if (!/^\d+$/.test(raw) || BigInt(raw) === 0n) {
    throw new Error("字节数必须是正整数。");
  }
  if (BigInt(raw) > 9_223_372_036_854_775_807n) {
    throw new Error("字节数超出支持范围。");
  }
  return raw;
}

async function finishReleaseMutation(data, message) {
  if (!data) return;
  const release = data.executeCatalogReleaseCommand;
  state.selectedRelease = release;
  elements.releaseManageDialog.close();
  showToast(message(release));
  if (state.selectedArtistId) await selectArtist(state.selectedArtistId);
}

function collectionStateClass(value) {
  const safe = String(value || "unknown").toLowerCase().replaceAll("_", "-");
  return `collection-state-pill collection-state-pill--${safe}`;
}

function describeCopies(copies) {
  if (!copies.length) return "—";
  return copies
    .map((copy) => `${copy.sourceLabel} · ${humanizeEnum(copy.sourceKind)}`)
    .join(" / ");
}

function describeQuality(copies) {
  if (!copies.length) return "—";
  return copies
    .map((copy) => {
      const resolution = [
        copy.sampleRateHz ? `${copy.sampleRateHz} Hz` : null,
        copy.bitDepth ? `${copy.bitDepth} bit` : null,
      ]
        .filter(Boolean)
        .join(" / ");
      return [
        humanizeEnum(copy.qualityTier),
        formatCodec(copy.codec),
        resolution,
        copy.qualityVerified ? "已验证" : "待验证",
      ]
        .filter(Boolean)
        .join(" · ");
    })
    .join(" / ");
}

function catalogProvisioningClass(value) {
  return `catalog-provisioning-pill catalog-provisioning-pill--${String(value || "unknown")
    .toLowerCase()
    .replaceAll("_", "-")}`;
}

function catalogCoverageLabel(value) {
  return (
    {
      FULL_SNAPSHOT: "完整快照",
      INCREMENTAL: "增量观察",
      DISCOVERY_ONLY: "目录发现",
    }[value] ?? humanizeEnum(value)
  );
}

function catalogRunDetail(run, source) {
  const attempts = String(run.attemptCount ?? "0");
  const observed = String(run.observedCount ?? "0");
  if (run.status === "QUEUED") {
    if (!source.enabled) return "任务已暂停；启用信源后继续排队。";
    if (run.nextAttemptAt) {
      const retryAt = new Date(run.nextAttemptAt);
      if (!Number.isNaN(retryAt.getTime()) && retryAt.getTime() > Date.now()) {
        return `第 ${attempts} 次尝试未完成；${formatDateTime(run.nextAttemptAt)} 自动重试。`;
      }
      return `第 ${attempts} 次尝试未完成；重试时间已到，等待 Worker。`;
    }
    return attempts === "0" ? "等待 Worker 接管。" : `等待第 ${attempts} 次重试。`;
  }
  if (run.status === "RUNNING") {
    return `第 ${attempts} 次执行 · 已形成 ${observed} 条 observation。`;
  }
  if (run.status === "SUCCEEDED") {
    return `${catalogCoverageLabel(run.coverage)} · ${run.snapshotComplete ? "快照完整" : "快照未完整"} · ${observed} 条 observation。`;
  }
  if (run.status === "FAILED") {
    return "任务已结束；错误详情只保留在受控 Worker 日志中。";
  }
  if (run.status === "CANCELLED") return "任务已取消，历史记录仍然保留。";
  return `${catalogCoverageLabel(run.coverage)} · ${observed} 条 observation。`;
}

function catalogRunRow(run, source) {
  const time = run.finishedAt ?? run.startedAt ?? run.nextAttemptAt ?? run.createdAt;
  return node(
    "article",
    {
      className: `catalog-run-row catalog-run-row--${run.status.toLowerCase()}`,
      attributes: { "data-catalog-run-id": run.runId },
    },
    [
      node("div", { className: "catalog-run-row__header" }, [
        node("strong", { text: CATALOG_RUN_STATUS_LABELS[run.status] ?? humanizeEnum(run.status) }),
        node("time", { text: formatDateTime(time), attributes: time ? { datetime: time } : {} }),
      ]),
      node("p", { className: "catalog-run-detail", text: catalogRunDetail(run, source) }),
      node("span", {
        className: "catalog-run-id",
        text: `Run ${compactId(run.runId)}`,
        attributes: { title: run.runId },
      }),
    ],
  );
}

function catalogHistoryForSource(source) {
  const historyState = state.catalogHistoryStateBySource[source.sourceId] ?? {
    status: "loading",
    error: "",
  };
  const runs = state.catalogRunsBySource[source.sourceId] ?? [];
  const children = [node("h4", { text: "最近运行" })];
  if (historyState.status === "loading") {
    children.push(node("p", { className: "catalog-run-detail", text: "正在读取运行历史…" }));
  } else if (historyState.status === "error") {
    children.push(
      node("p", {
        className: "catalog-source-error",
        text: historyState.error || "无法读取运行历史；为避免重复任务，同步按钮已锁定。",
      }),
    );
  } else if (!runs.length) {
    children.push(node("p", { className: "catalog-run-detail", text: "尚未执行同步。" }));
  } else {
    children.push(...runs.slice(0, 5).map((run) => catalogRunRow(run, source)));
  }
  return node("div", { className: "catalog-run-history" }, children);
}

function catalogSourceCard(source, panelError) {
  const historyState = state.catalogHistoryStateBySource[source.sourceId] ?? {
    status: "loading",
  };
  const runs = state.catalogRunsBySource[source.sourceId] ?? [];
  const activeRun = runs.find((run) => ACTIVE_CATALOG_RUN_STATES.has(run.status));
  const running = runs.some((run) => run.status === "RUNNING");
  const historyUnknown = historyState.status !== "ready";
  const startDisabled =
    historyState.status !== "ready" ||
    source.provisioningState !== "READY_TO_QUEUE" ||
    Boolean(activeRun);

  const start = node("button", {
    className: "primary-action",
    text: activeRun ? "已有活动任务" : "同步目录",
    attributes: {
      type: "button",
      "data-catalog-action": "start",
      "data-source-id": source.sourceId,
    },
  });
  start.disabled = startDisabled;
  start.addEventListener("click", () => void startCatalogSync(source, panelError));

  const toggle = node("button", {
    className: "secondary-action",
    text: source.enabled ? "暂停信源" : "启用信源",
    attributes: {
      type: "button",
      "data-catalog-action": "toggle",
      "data-source-id": source.sourceId,
      "aria-describedby": running || historyUnknown ? `catalog-action-note-${source.sourceId}` : null,
    },
  });
  toggle.disabled = running || historyUnknown;
  if (running) toggle.title = "运行中的任务不会被启停操作取消；请等待本次任务结束。";
  toggle.addEventListener("click", () => void toggleCatalogSource(source, panelError));

  const retry = node("button", {
    className: "secondary-action",
    text: "重试运行历史",
    attributes: {
      type: "button",
      "data-catalog-action": "retry-history",
      "data-source-id": source.sourceId,
    },
  });
  retry.addEventListener("click", () => void retryCatalogHistory(source, panelError));

  const actionNote =
    running || historyUnknown
      ? node("p", {
          className: "catalog-action-note",
          text: running
            ? "任务正在执行；暂停不是取消，因此本次结束前不能更改信源状态。"
            : "确认运行历史前，启停和新建任务都会保持锁定。",
          attributes: { id: `catalog-action-note-${source.sourceId}` },
        })
      : null;

  const meta = node("dl", { className: "catalog-source-meta" }, [
    node("div", {}, [node("dt", { text: "Storefront" }), node("dd", { text: textOrDash(source.storefront) })]),
    node("div", {}, [node("dt", { text: "Locale" }), node("dd", { text: textOrDash(source.locale) })]),
    node("div", {}, [
      node("dt", { text: "Source" }),
      node("dd", { text: compactId(source.sourceId), attributes: { title: source.sourceId } }),
    ]),
    node("div", {}, [
      node("dt", { text: "更新" }),
      node("dd", { text: formatDateTime(source.updatedAt) }),
    ]),
  ]);

  return node(
    "article",
    { className: "catalog-source-card", attributes: { "data-catalog-source-id": source.sourceId } },
    [
      node("header", { className: "catalog-source-card__header" }, [
        node("div", {}, [
          node("span", { className: "eyebrow", text: "Managed source" }),
          node("h4", { text: CATALOG_SOURCE_KIND_LABELS[source.kind] ?? humanizeEnum(source.kind) }),
        ]),
        node("span", {
          className: catalogProvisioningClass(source.provisioningState),
          text: CATALOG_PROVISIONING_LABELS[source.provisioningState] ?? humanizeEnum(source.provisioningState),
        }),
      ]),
      meta,
      catalogHistoryForSource(source),
      actionNote,
      node(
        "div",
        { className: "catalog-source-actions" },
        [historyState.status === "error" ? retry : null, toggle, start],
      ),
    ],
  );
}

function catalogSourcePanel(artist) {
  const addSource = node("button", {
    className: "secondary-action",
    text: "绑定 Apple Music",
    attributes: { type: "button", "data-catalog-action": "create-source" },
  });
  addSource.addEventListener("click", () => openCatalogSourceDialog(artist));

  const panelError = node("p", {
    className: "catalog-source-error",
    attributes: { role: "alert", "data-catalog-panel-error": "" },
  });
  const body = node("div");
  if (state.catalogSourcesStatus === "loading" || state.catalogSourcesStatus === "idle") {
    body.className = "catalog-run-detail";
    body.textContent = "正在读取官方目录信源…";
  } else if (state.catalogSourcesStatus === "error") {
    body.className = "catalog-source-error";
    body.textContent = state.catalogSourcesError || "无法读取目录信源。发行拉表仍可继续使用。";
  } else if (!state.catalogSources.length) {
    body.className = "catalog-sync-intro";
    body.append(
      node("strong", { text: "尚未绑定官方目录" }),
      node("span", { text: "先绑定 Apple Music Artist ID；其他官方来源会沿用同一条受控同步链路。" }),
    );
  } else {
    body.className = "catalog-sync-grid";
    body.replaceChildren(
      ...state.catalogSources.map((source) => catalogSourceCard(source, panelError)),
    );
  }

  return node("section", { className: "catalog-sync-panel", attributes: { id: "catalog-sync-panel" } }, [
    node("div", { className: "catalog-sync-heading" }, [
      node("div", {}, [
        node("span", { className: "eyebrow", text: "Official discography" }),
        node("h3", { text: "目录同步" }),
      ]),
      addSource,
    ]),
    node("p", {
      className: "catalog-sync-intro",
      text: "同步只生成待核对的 observation，不会覆盖 Booklet 结论，也不会把发行自动标记为已收集。",
    }),
    body,
    panelError,
  ]);
}

function updateCatalogSourcePanel() {
  const current = document.querySelector("#catalog-sync-panel");
  const artist = state.selectedCollection?.artist;
  if (!current || !artist) return;
  current.replaceWith(catalogSourcePanel(artist));
}

function upsertCatalogSource(source) {
  const index = state.catalogSources.findIndex((item) => item.sourceId === source.sourceId);
  if (index === -1) state.catalogSources.push(source);
  else state.catalogSources[index] = source;
}

function upsertCatalogRun(run) {
  const runs = [...(state.catalogRunsBySource[run.sourceId] ?? [])];
  const index = runs.findIndex((item) => item.runId === run.runId);
  if (index === -1) runs.unshift(run);
  else runs[index] = run;
  runs.sort((left, right) => String(right.createdAt).localeCompare(String(left.createdAt)));
  state.catalogRunsBySource[run.sourceId] = runs.slice(0, 5);
}

function catalogSelectionIsCurrent(artistId, token, loadGeneration = null) {
  return (
    state.view === "artists" &&
    state.selectedArtistId === artistId &&
    state.token === token &&
    (loadGeneration === null || state.artistLoadGeneration === loadGeneration)
  );
}

function catalogRunsToPoll() {
  const sources = new Map(state.catalogSources.map((source) => [source.sourceId, source]));
  const active = [];
  for (const [sourceId, runs] of Object.entries(state.catalogRunsBySource)) {
    const source = sources.get(sourceId);
    if (!source) continue;
    for (const run of runs) {
      if (run.status === "RUNNING" || (run.status === "QUEUED" && source.enabled)) {
        active.push(run);
      }
    }
  }
  return active;
}

function catalogPollDelay(runs) {
  if (runs.some((run) => run.status === "RUNNING")) return 3_000;
  let delay = 5_000;
  const futureRetries = runs
    .map((run) => (run.nextAttemptAt ? new Date(run.nextAttemptAt).getTime() - Date.now() : 0))
    .filter((value) => Number.isFinite(value) && value > 0);
  if (futureRetries.length) delay = Math.min(...futureRetries);
  return Math.max(5_000, Math.min(30_000, delay));
}

function scheduleCatalogRefresh() {
  stopCatalogRefresh();
  if (!state.token || state.view !== "artists" || document.hidden || !state.selectedArtistId) return;
  const runs = catalogRunsToPoll();
  if (!runs.length) return;
  const artistId = state.selectedArtistId;
  const token = state.token;
  const generation = state.catalogPollGeneration;
  state.catalogRefreshTimer = window.setTimeout(() => {
    state.catalogRefreshTimer = null;
    void pollCatalogRuns(runs, artistId, token, generation);
  }, catalogPollDelay(runs));
}

async function pollCatalogRuns(runs, artistId, token, generation) {
  const results = await Promise.allSettled(
    runs.map(async (run) => {
      const [runData, sourceData] = await Promise.all([
        graphql(CATALOG_SYNC_RUN_QUERY, { runId: run.runId }),
        graphql(CATALOG_SYNC_SOURCE_QUERY, { sourceId: run.sourceId }),
      ]);
      if (!runData.catalogSyncRun) {
        throw new GraphqlRequestError("同步任务已不存在。", "CATALOG_SYNC_RUN_NOT_FOUND");
      }
      if (!sourceData.catalogSyncSource) {
        throw new GraphqlRequestError("目录信源已不存在。", "CATALOG_SYNC_SOURCE_NOT_FOUND");
      }
      return { run: runData.catalogSyncRun, source: sourceData.catalogSyncSource };
    }),
  );
  if (
    generation !== state.catalogPollGeneration ||
    !catalogSelectionIsCurrent(artistId, token)
  ) {
    return;
  }

  results.forEach((result, index) => {
    const sourceId = runs[index].sourceId;
    if (result.status === "fulfilled") {
      upsertCatalogRun(result.value.run);
      upsertCatalogSource(result.value.source);
      state.catalogHistoryStateBySource[sourceId] = { status: "ready", error: "" };
      return;
    }
    const error = result.reason;
    state.catalogHistoryStateBySource[sourceId] = {
      status: "error",
      error: `运行状态刷新失败：${error.message}`,
    };
  });
  updateCatalogSourcePanel();
  scheduleCatalogRefresh();
}

async function startCatalogSync(source, panelError) {
  const panel = panelError.closest("#catalog-sync-panel");
  if (!panel || state.selectedArtistId !== source.artistId) return;
  stopCatalogRefresh();
  const token = state.token;
  const loadGeneration = state.artistLoadGeneration;
  const contextIsCurrent = () =>
    catalogSelectionIsCurrent(source.artistId, token, loadGeneration);
  const result = await runCatalogOperation(panel, panelError, async () => {
    const runData = await graphql(START_CATALOG_SYNC_RUN_MUTATION, {
      input: { sourceId: source.sourceId },
    });
    if (!contextIsCurrent()) return { obsolete: true };
    const sourceData = await graphql(CATALOG_SYNC_SOURCE_QUERY, { sourceId: source.sourceId });
    if (!contextIsCurrent()) return { obsolete: true };
    if (!sourceData.catalogSyncSource) {
      throw new GraphqlRequestError("同步已排队，但信源无法重新读取。", "CATALOG_SYNC_SOURCE_NOT_FOUND");
    }
    return { run: runData.startCatalogSyncRun, source: sourceData.catalogSyncSource };
  });
  if (!result) {
    if (!contextIsCurrent()) return;
    const message = panelError.textContent;
    if (panelError.dataset.errorCode === "CATALOG_MUTATION_BUSY") {
      scheduleCatalogRefresh();
      return;
    }
    showToast(message, "error");
    await selectArtist(source.artistId, { background: true });
    return;
  }
  if (result.obsolete || !contextIsCurrent()) return;
  upsertCatalogSource(result.source);
  upsertCatalogRun(result.run);
  state.catalogHistoryStateBySource[source.sourceId] = { status: "ready", error: "" };
  updateCatalogSourcePanel();
  scheduleCatalogRefresh();
  showToast("目录同步任务已进入队列");
}

async function toggleCatalogSource(source, panelError) {
  const panel = panelError.closest("#catalog-sync-panel");
  if (!panel || state.selectedArtistId !== source.artistId) return;
  stopCatalogRefresh();
  const token = state.token;
  const loadGeneration = state.artistLoadGeneration;
  const desiredEnabled = !source.enabled;
  const contextIsCurrent = () =>
    catalogSelectionIsCurrent(source.artistId, token, loadGeneration);
  const result = await runCatalogOperation(panel, panelError, async () => {
    const [latestData, historyData] = await Promise.all([
      graphql(CATALOG_SYNC_SOURCE_QUERY, { sourceId: source.sourceId }),
      graphql(CATALOG_SYNC_RUNS_QUERY, { sourceId: source.sourceId, limit: 5, offset: 0 }),
    ]);
    if (!contextIsCurrent()) return { obsolete: true };
    const latest = latestData.catalogSyncSource;
    if (!latest) {
      throw new GraphqlRequestError("信源已不存在。", "CATALOG_SYNC_SOURCE_NOT_FOUND");
    }
    const runs = historyData.catalogSyncRuns;
    if (runs.some((run) => run.status === "RUNNING")) {
      return { blockedByRunning: true, source: latest, runs };
    }
    if (latest.enabled !== source.enabled) {
      return { staleIntent: true, source: latest, runs };
    }
    const mutationData = await graphql(SET_CATALOG_SYNC_SOURCE_ENABLED_MUTATION, {
      input: {
        sourceId: latest.sourceId,
        expectedRowVersion: latest.rowVersion,
        enabled: desiredEnabled,
      },
    });
    if (!contextIsCurrent()) return { obsolete: true };
    return { source: mutationData.setCatalogSyncSourceEnabled, runs };
  });
  if (!result) {
    if (!contextIsCurrent()) return;
    const message = panelError.textContent;
    const code = panelError.dataset.errorCode;
    if (code === "CATALOG_MUTATION_BUSY") {
      scheduleCatalogRefresh();
      return;
    }
    showToast(
      code === "CATALOG_SYNC_SOURCE_CONFLICT"
        ? `${message} 已重新读取最新状态。`
        : `${message} 正在核对服务端状态。`,
      "error",
    );
    await selectArtist(source.artistId, { background: true });
    return;
  }
  if (result.obsolete || !contextIsCurrent()) return;
  upsertCatalogSource(result.source);
  state.catalogRunsBySource[source.sourceId] = result.runs;
  state.catalogHistoryStateBySource[source.sourceId] = { status: "ready", error: "" };
  updateCatalogSourcePanel();
  scheduleCatalogRefresh();
  if (result.staleIntent) {
    showToast("信源状态已被其他会话更改；本次未执行相反操作，请按最新状态重新确认。", "error");
  } else if (result.blockedByRunning) {
    showToast("任务已开始执行；暂停不是取消，请等待本次运行结束。", "error");
  } else {
    showToast(result.source.enabled ? "目录信源已启用" : "目录信源已暂停");
  }
}

async function retryCatalogHistory(source, panelError) {
  const panel = panelError.closest("#catalog-sync-panel");
  if (!panel || state.selectedArtistId !== source.artistId) return;
  stopCatalogRefresh();
  const token = state.token;
  const loadGeneration = state.artistLoadGeneration;
  const contextIsCurrent = () =>
    catalogSelectionIsCurrent(source.artistId, token, loadGeneration);
  const result = await runCatalogOperation(panel, panelError, async () => {
    const [historyData, sourceData] = await Promise.all([
      graphql(CATALOG_SYNC_RUNS_QUERY, { sourceId: source.sourceId, limit: 5, offset: 0 }),
      graphql(CATALOG_SYNC_SOURCE_QUERY, { sourceId: source.sourceId }),
    ]);
    if (!contextIsCurrent()) return { obsolete: true };
    if (!sourceData.catalogSyncSource) {
      throw new GraphqlRequestError("信源已不存在。", "CATALOG_SYNC_SOURCE_NOT_FOUND");
    }
    return { runs: historyData.catalogSyncRuns, source: sourceData.catalogSyncSource };
  });
  if (!result) {
    if (contextIsCurrent()) scheduleCatalogRefresh();
    return;
  }
  if (result.obsolete || !contextIsCurrent()) return;
  upsertCatalogSource(result.source);
  state.catalogRunsBySource[source.sourceId] = result.runs;
  state.catalogHistoryStateBySource[source.sourceId] = { status: "ready", error: "" };
  updateCatalogSourcePanel();
  scheduleCatalogRefresh();
}

function renderCollection(collection) {
  if (!collection) {
    renderMessage(elements.collectionSheet, "empty", "尚未建立拉表", "该 Artist 当前没有 collection 记录。");
    return;
  }
  const { artist, summary, releases } = collection;
  const createRelease = node("button", {
    className: "secondary-action",
    text: "登记发行",
    attributes: { type: "button" },
  });
  createRelease.addEventListener("click", () => openReleaseDialog(artist));
  const heading = node("header", { className: "collection-heading" }, [
    node("div", {}, [
      node("h2", { text: artist.displayName }),
      node("p", { text: `${collection.releaseTotalCount} 个发行记录` }),
    ]),
    node("div", { className: "collection-heading__actions" }, [
      node("div", { className: "collection-summary" }, [
        node("span", { text: `已收集 ${summary.collected}` }),
        node("span", { text: `缺失 ${summary.missing}` }),
        node("span", { text: `想要 ${summary.wanted}` }),
        node("span", { text: `已发布 ${summary.published}` }),
      ]),
      createRelease,
    ]),
  ]);
  const sourcePanel = catalogSourcePanel(artist);

  if (!releases.length) {
    elements.collectionSheet.replaceChildren(
      heading,
      sourcePanel,
      node("div", { className: "empty-state" }, [
        node("strong", { text: "发行列表为空" }),
        node("p", { text: "可以手工登记发行，或绑定官方目录来源后同步。" }),
      ]),
    );
    return;
  }

  const tbody = node(
    "tbody",
    {},
    releases.map((release) => {
      const manage = node("button", {
        className: "table-action",
        text: "管理",
        attributes: { type: "button", "aria-label": `管理发行 ${release.title}` },
      });
      manage.addEventListener("click", () => openReleaseManager(release));
      return node("tr", {}, [
        node("td", {}, [
          node("span", { className: "release-title", text: release.title }),
          node("span", {
            className: "release-subtitle",
            text: [release.edition, release.catalog].filter(Boolean).join(" · ") || "—",
          }),
        ]),
        node("td", { text: textOrDash(release.releaseDate) }),
        node("td", {}, [
          node("span", {
            className: collectionStateClass(release.collectionState),
            text: humanizeEnum(release.collectionState),
          }),
        ]),
        node("td", { text: describeCopies(release.copies) }),
        node("td", {}, [
          node("span", { className: "quality-pill", text: describeQuality(release.copies) }),
        ]),
        node("td", {}, [manage]),
      ]);
    }),
  );
  const table = node("table", { className: "release-table" }, [
    node("thead", {}, [
      node("tr", {}, [
        node("th", { text: "发行" }),
        node("th", { text: "日期" }),
        node("th", { text: "状态" }),
        node("th", { text: "取得来源" }),
        node("th", { text: "音质" }),
        node("th", { text: "操作" }),
      ]),
    ]),
    tbody,
  ]);
  elements.collectionSheet.replaceChildren(heading, sourcePanel, table);
}

async function selectArtist(artistId, { background = false } = {}) {
  stopCatalogRefresh();
  const loadGeneration = ++state.artistLoadGeneration;
  const token = state.token;
  const switchingArtist = state.selectedArtistId !== artistId;
  state.selectedArtistId = artistId;
  state.selectedRelease = null;
  state.catalogSourcesStatus = "loading";
  state.catalogSourcesError = "";
  if (switchingArtist) {
    state.selectedCollection = null;
    state.catalogSources = [];
    state.catalogRunsBySource = {};
    state.catalogHistoryStateBySource = {};
  }
  renderArtists(state.artists);
  if (!background || !state.selectedCollection) {
    renderMessage(elements.collectionSheet, "loading", "正在读取 Artist 工作台", "并行汇总发行、副本与官方目录信源。");
  }

  const [collectionResult, sourcesResult] = await Promise.allSettled([
    graphql(COLLECTION_QUERY, { artistId, limit: 500, offset: 0 }),
    graphql(CATALOG_SYNC_SOURCES_QUERY, { artistId }),
  ]);
  for (const result of [collectionResult, sourcesResult]) {
    if (result.status === "rejected" && result.reason instanceof SessionRequiredError) {
      openSessionDialog(result.reason.message);
    }
  }
  if (!catalogSelectionIsCurrent(artistId, token, loadGeneration)) return;

  if (collectionResult.status === "rejected") {
    renderMessage(
      elements.collectionSheet,
      "error",
      "无法读取发行列表",
      collectionResult.reason.message,
    );
    return;
  }

  state.selectedCollection = collectionResult.value.catalogArtistCollection;
  if (sourcesResult.status === "rejected") {
    state.catalogSources = [];
    state.catalogSourcesStatus = "error";
    state.catalogSourcesError = `官方目录读取失败：${sourcesResult.reason.message}`;
    state.catalogRunsBySource = {};
    state.catalogHistoryStateBySource = {};
    renderCollection(state.selectedCollection);
    return;
  }

  state.catalogSources = sourcesResult.value.catalogSyncSources;
  state.catalogSourcesStatus = "ready";
  state.catalogRunsBySource = {};
  state.catalogHistoryStateBySource = Object.fromEntries(
    state.catalogSources.map((source) => [source.sourceId, { status: "loading", error: "" }]),
  );
  renderCollection(state.selectedCollection);

  const historyResults = await Promise.allSettled(
    state.catalogSources.map((source) =>
      graphql(CATALOG_SYNC_RUNS_QUERY, { sourceId: source.sourceId, limit: 5, offset: 0 }),
    ),
  );
  for (const result of historyResults) {
    if (result.status === "rejected" && result.reason instanceof SessionRequiredError) {
      openSessionDialog(result.reason.message);
    }
  }
  if (!catalogSelectionIsCurrent(artistId, token, loadGeneration)) return;

  historyResults.forEach((result, index) => {
    const sourceId = state.catalogSources[index].sourceId;
    if (result.status === "fulfilled") {
      state.catalogRunsBySource[sourceId] = result.value.catalogSyncRuns;
      state.catalogHistoryStateBySource[sourceId] = { status: "ready", error: "" };
    } else {
      state.catalogRunsBySource[sourceId] = [];
      state.catalogHistoryStateBySource[sourceId] = {
        status: "error",
        error: `运行历史读取失败：${result.reason.message}`,
      };
    }
  });
  updateCatalogSourcePanel();
  scheduleCatalogRefresh();
}

async function connectSession(token) {
  await graphql(INTAKE_QUERY, { limit: 1, offset: 0 }, token);
  sessionStorage.setItem(SESSION_KEY, token);
  state.token = token;
  state.jobs = [];
  state.artists = [];
  updateSessionUi();
  elements.sessionDialog.close();
  showToast("已连接 Annim 管理会话");
  if (state.view === "intake") await ensureIntakeLoaded(true);
  if (state.view === "artists") await ensureArtistsLoaded(true);
}

elements.sessionButton.addEventListener("click", () => openSessionDialog());

elements.sessionForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const token = elements.adminToken.value.trim();
  if (!token) {
    elements.sessionError.textContent = "请输入管理令牌。";
    return;
  }
  elements.sessionError.textContent = "正在验证令牌…";
  try {
    await connectSession(token);
  } catch (error) {
    elements.sessionError.textContent =
      error instanceof SessionRequiredError ? "令牌无效。" : error.message;
  } finally {
    elements.adminToken.value = "";
  }
});

elements.forgetSession.addEventListener("click", () => {
  clearSession();
  elements.sessionDialog.close();
});

elements.createArtistButton.addEventListener("click", openArtistDialog);

elements.artistCreateForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const displayName = elements.artistCreateForm.elements.namedItem("displayName").value;
  if (!displayName.trim()) {
    elements.artistFormError.textContent = "显示名称不能为空。";
    return;
  }
  const data = await runCatalogMutation(
    elements.artistCreateForm,
    elements.artistFormError,
    CREATE_CATALOG_ARTIST_MUTATION,
    {
      displayName,
      sortName: optionalExactFormValue(elements.artistCreateForm, "sortName"),
      notes: optionalExactFormValue(elements.artistCreateForm, "notes"),
    },
  );
  if (!data) return;
  const artist = data.createCatalogArtist;
  elements.artistDialog.close();
  elements.artistCreateForm.reset();
  elements.artistSearch.value = "";
  showToast(`Artist「${artist.displayName}」已创建`);
  await loadArtists();
  await selectArtist(artist.artistId);
});

elements.releaseCreateForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const artistId = state.selectedArtistId;
  if (!artistId) {
    elements.releaseFormError.textContent = "请先选择 Artist。";
    return;
  }
  const title = elements.releaseCreateForm.elements.namedItem("title").value;
  if (!title.trim()) {
    elements.releaseFormError.textContent = "发行标题不能为空。";
    return;
  }
  const releaseDate = optionalExactFormValue(elements.releaseCreateForm, "releaseDate");
  if (releaseDate && !/^\d{4}(?:-\d{2}){0,2}$/.test(releaseDate)) {
    elements.releaseFormError.textContent = "发行日期应为 YYYY、YYYY-MM 或 YYYY-MM-DD。";
    return;
  }
  const data = await runCatalogMutation(
    elements.releaseCreateForm,
    elements.releaseFormError,
    CREATE_CATALOG_RELEASE_MUTATION,
    {
      artistId,
      title,
      edition: optionalExactFormValue(elements.releaseCreateForm, "edition"),
      catalog: optionalExactFormValue(elements.releaseCreateForm, "catalog"),
      releaseDate,
      kind: elements.releaseCreateForm.elements.namedItem("kind").value,
      notes: optionalExactFormValue(elements.releaseCreateForm, "notes"),
    },
  );
  if (!data) return;
  const release = data.createCatalogRelease;
  elements.releaseDialog.close();
  elements.releaseCreateForm.reset();
  showToast(`发行「${release.title}」已登记`);
  await selectArtist(artistId);
});

elements.catalogSourceForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const artistId = state.selectedArtistId;
  if (!artistId) {
    elements.catalogSourceFormError.textContent = "请先选择 Artist。";
    return;
  }
  const locator = elements.catalogSourceForm.elements.namedItem("locator").value;
  const storefront = elements.catalogSourceForm.elements.namedItem("storefront").value;
  const locale = elements.catalogSourceForm.elements.namedItem("locale").value;
  if (!/^(?!0+$)\d{1,32}$/.test(locator)) {
    elements.catalogSourceFormError.textContent = "Apple Music Artist ID 必须是 1–32 位非全零数字。";
    return;
  }
  if (!/^[a-z]{2}$/.test(storefront)) {
    elements.catalogSourceFormError.textContent = "Storefront 必须是两个小写 ASCII 字母。";
    return;
  }
  if (locale !== "") {
    if (locale.length > 35 || !/^[\x21-\x7e]+$/.test(locale) || locale.includes("_")) {
      elements.catalogSourceFormError.textContent = "Locale 必须是最多 35 位的 ASCII BCP 47 标记，不能使用下划线。";
      return;
    }
    try {
      Intl.getCanonicalLocales(locale);
    } catch {
      elements.catalogSourceFormError.textContent = "Locale 不是有效的 BCP 47 标记。";
      return;
    }
  }
  const data = await runCatalogMutation(
    elements.catalogSourceForm,
    elements.catalogSourceFormError,
    CREATE_CATALOG_SYNC_SOURCE_MUTATION,
    {
      artistId,
      kind: "APPLE_MUSIC",
      locator,
      storefront,
      locale: locale === "" ? null : locale,
    },
  );
  if (!data) return;
  elements.catalogSourceDialog.close();
  elements.catalogSourceForm.reset();
  showToast("Apple Music 目录信源已建立");
  await selectArtist(artistId, { background: true });
});

for (const button of elements.collectionCopyForm.querySelectorAll("[data-release-command]")) {
  button.addEventListener("click", async () => {
    const release = state.selectedRelease;
    if (!release) {
      elements.collectionCopyFormError.textContent = "请重新打开要管理的发行。";
      return;
    }
    const data = await runCatalogMutation(
      elements.collectionCopyForm,
      elements.collectionCopyFormError,
      EXECUTE_CATALOG_RELEASE_COMMAND_MUTATION,
      {
        releaseId: release.releaseId,
        expectedRowVersion: release.rowVersion,
        command: { [button.dataset.releaseCommand]: "EXECUTE" },
      },
    );
    await finishReleaseMutation(
      data,
      (updated) => `发行状态已更新为 ${humanizeEnum(updated.collectionState)}`,
    );
  });
}

elements.collectionCopyForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const release = state.selectedRelease;
  if (!release) {
    elements.collectionCopyFormError.textContent = "请重新打开要管理的发行。";
    return;
  }

  const sourceLabel = elements.collectionCopyForm.elements.namedItem("sourceLabel").value;
  if (!sourceLabel.trim()) {
    elements.collectionCopyFormError.textContent = "来源名称不能为空。";
    return;
  }

  let sampleRateHz;
  let bitDepth;
  let channels;
  let trackCount;
  let byteLength;
  try {
    sampleRateHz = optionalPositiveIntegerFormValue(
      elements.collectionCopyForm,
      "sampleRateHz",
      "采样率",
      2_147_483_647,
    );
    bitDepth = optionalPositiveIntegerFormValue(
      elements.collectionCopyForm,
      "bitDepth",
      "位深",
      255,
    );
    channels = optionalPositiveIntegerFormValue(
      elements.collectionCopyForm,
      "channels",
      "声道数",
      255,
    );
    trackCount = optionalPositiveIntegerFormValue(
      elements.collectionCopyForm,
      "trackCount",
      "轨道数",
      2_147_483_647,
    );
    byteLength = optionalByteLengthFormValue(elements.collectionCopyForm);
  } catch (error) {
    elements.collectionCopyFormError.textContent = error.message;
    return;
  }

  const manifestDigest = optionalExactFormValue(
    elements.collectionCopyForm,
    "manifestDigest",
  );
  if (manifestDigest && !/^[0-9a-fA-F]{64}$/.test(manifestDigest)) {
    elements.collectionCopyFormError.textContent = "Manifest SHA-256 必须是 64 位十六进制。";
    return;
  }

  const ingestJobId = optionalExactFormValue(elements.collectionCopyForm, "ingestJobId");
  if (
    ingestJobId &&
    !/^[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}$/.test(
      ingestJobId,
    )
  ) {
    elements.collectionCopyFormError.textContent = "Ingest Job UUID 格式不正确。";
    return;
  }

  const data = await runCatalogMutation(
    elements.collectionCopyForm,
    elements.collectionCopyFormError,
    EXECUTE_CATALOG_RELEASE_COMMAND_MUTATION,
    {
      releaseId: release.releaseId,
      expectedRowVersion: release.rowVersion,
      command: {
        recordCopy: {
          sourceKind: elements.collectionCopyForm.elements.namedItem("sourceKind").value,
          sourceLabel,
          privateLocator: optionalExactFormValue(
            elements.collectionCopyForm,
            "privateLocator",
          ),
          codec: elements.collectionCopyForm.elements.namedItem("codec").value,
          sampleRateHz,
          bitDepth,
          channels,
          trackCount,
          byteLength,
          manifestDigest,
          qualityVerified:
            elements.collectionCopyForm.elements.namedItem("qualityVerified").checked,
          ingestJobId,
          notes: optionalExactFormValue(elements.collectionCopyForm, "notes"),
        },
      },
    },
  );
  await finishReleaseMutation(data, () => `音源副本「${sourceLabel}」已登记`);
});

for (const button of document.querySelectorAll("[data-close-catalog-dialog]")) {
  button.addEventListener("click", () => button.closest("dialog").close());
}

elements.artistSearchForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  stopCatalogRefresh();
  state.artistLoadGeneration += 1;
  state.selectedArtistId = null;
  state.selectedRelease = null;
  state.selectedCollection = null;
  state.catalogSources = [];
  state.catalogSourcesStatus = "idle";
  state.catalogSourcesError = "";
  state.catalogRunsBySource = {};
  state.catalogHistoryStateBySource = {};
  renderMessage(elements.collectionSheet, "empty", "选择一位艺人", "查看发行列表、来源与音质。");
  try {
    await loadArtists(elements.artistSearch.value.trim() || null);
  } catch (error) {
    if (error instanceof SessionRequiredError) return;
    renderMessage(elements.artistList, "error", "搜索失败", error.message);
  }
});

for (const button of document.querySelectorAll("[data-refresh]")) {
  button.addEventListener("click", () => {
    if (button.dataset.refresh === "intake") void ensureIntakeLoaded(true);
    if (button.dataset.refresh === "artists") {
      void (async () => {
        await ensureArtistsLoaded(true);
        if (state.selectedArtistId) {
          await selectArtist(state.selectedArtistId, { background: true });
        }
      })();
    }
  });
}

document.querySelector("[data-close-drawer]").addEventListener("click", () => {
  elements.ingestDetail.hidden = true;
});

window.addEventListener("hashchange", routeFromHash);
document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    stopCatalogRefresh();
  } else if (state.view === "artists" && state.selectedArtistId) {
    void selectArtist(state.selectedArtistId, { background: true });
  }
});

updateSessionUi();
routeFromHash();
void checkServer();
