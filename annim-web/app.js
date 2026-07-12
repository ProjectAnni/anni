const SESSION_KEY = "annim.adminToken";
const VIEWS = new Set(["workflow", "intake", "artists"]);

const state = {
  token: sessionStorage.getItem(SESSION_KEY) ?? "",
  jobs: [],
  artists: [],
  selectedArtistId: null,
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

function clearSession({ notify = true } = {}) {
  sessionStorage.removeItem(SESSION_KEY);
  state.token = "";
  state.jobs = [];
  state.artists = [];
  state.selectedArtistId = null;
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
  if (safeView === "artists") void ensureArtistsLoaded();
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

async function runCatalogMutation(form, errorTarget, query, input) {
  if (state.catalogMutationPending) return null;
  state.catalogMutationPending = true;
  errorTarget.textContent = "正在保存…";
  const controls = Array.from(form.querySelectorAll("button, input, select, textarea"));
  const priorDisabledStates = controls.map((control) => control.disabled);
  controls.forEach((control) => {
    control.disabled = true;
  });
  try {
    const data = await graphql(query, { input });
    errorTarget.textContent = "";
    return data;
  } catch (error) {
    errorTarget.textContent = error.message;
    if (error instanceof SessionRequiredError) {
      form.closest("dialog")?.close();
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
      return [humanizeEnum(copy.qualityTier), humanizeEnum(copy.codec), resolution]
        .filter(Boolean)
        .join(" · ");
    })
    .join(" / ");
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

  if (!releases.length) {
    elements.collectionSheet.replaceChildren(
      heading,
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
    releases.map((release) =>
      node("tr", {}, [
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
      ]),
    ),
  );
  const table = node("table", { className: "release-table" }, [
    node("thead", {}, [
      node("tr", {}, [
        node("th", { text: "发行" }),
        node("th", { text: "日期" }),
        node("th", { text: "状态" }),
        node("th", { text: "取得来源" }),
        node("th", { text: "音质" }),
      ]),
    ]),
    tbody,
  ]);
  elements.collectionSheet.replaceChildren(heading, table);
}

async function selectArtist(artistId) {
  state.selectedArtistId = artistId;
  renderArtists(state.artists);
  renderMessage(elements.collectionSheet, "loading", "正在读取发行列表", "汇总目录状态与音频副本。");
  try {
    const data = await graphql(COLLECTION_QUERY, { artistId, limit: 500, offset: 0 });
    renderCollection(data.catalogArtistCollection);
  } catch (error) {
    if (error instanceof SessionRequiredError) openSessionDialog(error.message);
    renderMessage(elements.collectionSheet, "error", "无法读取发行列表", error.message);
  }
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

for (const button of document.querySelectorAll("[data-close-catalog-dialog]")) {
  button.addEventListener("click", () => button.closest("dialog").close());
}

elements.artistSearchForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  state.selectedArtistId = null;
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
    if (button.dataset.refresh === "artists") void ensureArtistsLoaded(true);
  });
}

document.querySelector("[data-close-drawer]").addEventListener("click", () => {
  elements.ingestDetail.hidden = true;
});

window.addEventListener("hashchange", routeFromHash);

updateSessionUi();
routeFromHash();
void checkServer();
