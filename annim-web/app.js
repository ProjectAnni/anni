const SESSION_KEY = "annim.adminToken";
const VIEWS = new Set(["workflow", "intake", "artists"]);

const state = {
  token: sessionStorage.getItem(SESSION_KEY) ?? "",
  jobs: [],
  artists: [],
  selectedArtistId: null,
  view: "workflow",
  reviewMutationPending: false,
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
  toastRegion: document.querySelector("#toast-region"),
};

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
  const actions = Array.from(elements.ingestDetailContent.querySelectorAll("button"));
  const priorDisabledStates = actions.map((action) => action.disabled);
  for (const action of actions) {
    action.disabled = true;
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
    actions.forEach((action, index) => {
      if (action.isConnected) action.disabled = priorDisabledStates[index];
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

function reviewActions(job, draft) {
  const actions = node("div", { className: "review-actions" });
  const currentApproved = job.approvedRevision === draft.revision;

  if (reviewIsEditable(job, draft)) {
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
      draft.missingFields.length ? missingFieldsNode(draft) : null,
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
  const heading = node("header", { className: "collection-heading" }, [
    node("div", {}, [
      node("h2", { text: artist.displayName }),
      node("p", { text: `${collection.releaseTotalCount} 个发行记录` }),
    ]),
    node("div", { className: "collection-summary" }, [
      node("span", { text: `已收集 ${summary.collected}` }),
      node("span", { text: `缺失 ${summary.missing}` }),
      node("span", { text: `想要 ${summary.wanted}` }),
      node("span", { text: `已发布 ${summary.published}` }),
    ]),
  ]);

  if (!releases.length) {
    elements.collectionSheet.replaceChildren(
      heading,
      node("div", { className: "empty-state" }, [
        node("strong", { text: "发行列表为空" }),
        node("p", { text: "绑定官方目录来源并完成同步后，Release 会显示在这里。" }),
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
