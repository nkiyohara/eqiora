import { expect, type JSHandle, type Locator, type Page, test } from "@playwright/test";

const MESH_DIGEST = "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b";
const VIEW_SELECTOR = `[data-eqiora-mesh-digest="${MESH_DIGEST}"]`;
const BARE_MESH_TEST_TITLE =
  "bare Mesh owns exact interaction, lifecycle, identity, and loopback-only observations";
const WEBGL_FAILURE_TEST_TITLE =
  "WebGL construction failure is an accessible failure rather than a blank pass";
const COLLECTOR_MARKER = "EQIORA_TEMPORARY_MESH_COLLECTED";
const IDENTITY_MARKER = "EQIORA_MESH_UNCHANGED";
const COLLECTOR_ASSERTION = "assert temporary_mesh_reference() is None";
const COLLECTOR_SOURCE = [
  "# Claim: displaying a Mesh adds no Eqiora-owned strong reference to it. The",
  "# widget delegate outlives this cell inside ipywidgets' global registry, so a",
  "# delegate, comm, or Eqiora module that retained the Mesh fails this assertion.",
  "# Nothing is claimed here about a Mesh that the host's own caches still hold.",
  "gc.collect()",
  COLLECTOR_ASSERTION,
  `print('${COLLECTOR_MARKER}')`,
].join("\n");
const IDENTITY_SOURCE = [
  "assert_unchanged(mesh, accepted_snapshot)",
  `print('${IDENTITY_MARKER}')`,
].join("\n");

type MarkerObservation = {
  pageCount: number;
  ownerCount: number;
  ownerVisibleCount: number;
};

type JupyterDomSnapshot = {
  cellCount: number;
  collectorSource: string | null;
  identitySource: string | null;
  collectorPrompt: string | null;
  identityPrompt: string | null;
  collectorMarker: MarkerObservation;
  identityMarker: MarkerObservation;
  stderrCount: number;
  collectorStderrCount: number;
  collectorAssertionStderrCount: number;
};

type JupyterClassificationSnapshot = {
  before: JupyterDomSnapshot;
  after: JupyterDomSnapshot;
};

type JupyterFailureAtom =
  | "collector-assertion"
  | "collector-not-terminal"
  | "collector-success-dom-propagation"
  | "contradiction";

type JupyterClassification = JupyterFailureAtom | "ordinary-success";

type PromptState =
  | { kind: "blank" }
  | { kind: "running" }
  | { kind: "completed"; count: string }
  | { kind: "invalid" };

function parsePrompt(prompt: string | null): PromptState {
  if (prompt === "[ ]:") return { kind: "blank" };
  if (prompt === "[*]:") return { kind: "running" };
  const completed = /^\[(\d+)\]:$/.exec(prompt ?? "");
  if (completed === null) return { kind: "invalid" };
  return { kind: "completed", count: completed[1].replace(/^0+(?=\d)/, "") };
}

function compareDecimal(left: string, right: string): -1 | 0 | 1 {
  if (left.length !== right.length) return left.length > right.length ? 1 : -1;
  if (left === right) return 0;
  return left > right ? 1 : -1;
}

function promptTransition(
  before: string | null,
  after: string | null,
): "advanced" | "waiting" | "invalid" {
  const previous = parsePrompt(before);
  const current = parsePrompt(after);
  if (
    previous.kind === "invalid" ||
    previous.kind === "running" ||
    current.kind === "invalid"
  ) {
    return "invalid";
  }
  if (current.kind === "completed") {
    if (previous.kind === "blank") return "advanced";
    const order = compareDecimal(current.count, previous.count);
    if (order > 0) return "advanced";
    if (order === 0) return "waiting";
    return "invalid";
  }
  if (current.kind === "running") return "waiting";
  return previous.kind === "blank" ? "waiting" : "invalid";
}

function exactAbsent(marker: MarkerObservation): boolean {
  return (
    marker.pageCount === 0 && marker.ownerCount === 0 && marker.ownerVisibleCount === 0
  );
}

function exactVisibleOwner(marker: MarkerObservation): boolean {
  return (
    marker.pageCount === 1 && marker.ownerCount === 1 && marker.ownerVisibleCount === 1
  );
}

function exactHiddenOwner(marker: MarkerObservation): boolean {
  return (
    marker.pageCount === 1 && marker.ownerCount === 1 && marker.ownerVisibleCount === 0
  );
}

function exactSourceTopology(snapshot: JupyterDomSnapshot): boolean {
  return (
    snapshot.cellCount === 9 &&
    snapshot.collectorSource === COLLECTOR_SOURCE &&
    snapshot.identitySource === IDENTITY_SOURCE
  );
}

function cleanBefore(snapshot: JupyterDomSnapshot): boolean {
  return (
    exactSourceTopology(snapshot) &&
    parsePrompt(snapshot.collectorPrompt).kind !== "invalid" &&
    parsePrompt(snapshot.collectorPrompt).kind !== "running" &&
    parsePrompt(snapshot.identityPrompt).kind !== "invalid" &&
    parsePrompt(snapshot.identityPrompt).kind !== "running" &&
    exactAbsent(snapshot.collectorMarker) &&
    exactAbsent(snapshot.identityMarker) &&
    snapshot.stderrCount === 0 &&
    snapshot.collectorStderrCount === 0 &&
    snapshot.collectorAssertionStderrCount === 0
  );
}

function classifyJupyterSnapshot(
  snapshot: JupyterClassificationSnapshot,
): JupyterClassification {
  const { before, after } = snapshot;
  if (!cleanBefore(before) || !exactSourceTopology(after)) return "contradiction";

  const collector = promptTransition(before.collectorPrompt, after.collectorPrompt);
  const identity = promptTransition(before.identityPrompt, after.identityPrompt);
  if (collector === "invalid" || identity === "invalid") return "contradiction";

  const noErrors =
    after.stderrCount === 0 &&
    after.collectorStderrCount === 0 &&
    after.collectorAssertionStderrCount === 0;
  if (
    collector === "advanced" &&
    identity === "advanced" &&
    noErrors &&
    exactVisibleOwner(after.collectorMarker) &&
    exactVisibleOwner(after.identityMarker)
  ) {
    return "ordinary-success";
  }

  if (
    collector === "advanced" &&
    identity === "waiting" &&
    exactAbsent(after.collectorMarker) &&
    exactAbsent(after.identityMarker) &&
    after.stderrCount === 1 &&
    after.collectorStderrCount === 1 &&
    after.collectorAssertionStderrCount === 1
  ) {
    return "collector-assertion";
  }

  if (
    collector === "waiting" &&
    identity === "waiting" &&
    noErrors &&
    exactAbsent(after.collectorMarker) &&
    exactAbsent(after.identityMarker)
  ) {
    return "collector-not-terminal";
  }

  if (
    collector === "advanced" &&
    identity === "advanced" &&
    noErrors &&
    exactVisibleOwner(after.identityMarker) &&
    (exactAbsent(after.collectorMarker) || exactHiddenOwner(after.collectorMarker))
  ) {
    return "collector-success-dom-propagation";
  }

  return "contradiction";
}

function isJupyterClassificationTarget(projectName: string, title: string): boolean {
  return (
    projectName === "jupyterlab-4.6.2" &&
    (title === BARE_MESH_TEST_TITLE || title === WEBGL_FAILURE_TEST_TITLE)
  );
}

function isInlineJupyterClassificationTarget(): boolean {
  const info = test.info();
  return isJupyterClassificationTarget(info.project.name, info.title);
}

async function collectJupyterDomSnapshot(page: Page): Promise<JupyterDomSnapshot> {
  return page.evaluate(
    ({ collectorAssertion, collectorMarker, identityMarker }) => {
      const cells = Array.from(
        document.querySelectorAll<HTMLElement>(".jp-Notebook .jp-CodeCell"),
      );
      const collector = cells[5];
      const identity = cells[8];

      const source = (cell: HTMLElement | undefined): string | null => {
        if (cell === undefined) return null;
        const lines = Array.from(
          cell.querySelectorAll<HTMLElement>(
            ".jp-InputArea-editor .cm-content .cm-line",
          ),
        );
        if (lines.length === 0) return null;
        return lines.map((line) => line.textContent ?? "").join("\n");
      };
      const prompt = (cell: HTMLElement | undefined): string | null => {
        if (cell === undefined) return null;
        const prompts = cell.querySelectorAll<HTMLElement>(".jp-InputPrompt");
        return prompts.length === 1 ? prompts[0].textContent : null;
      };
      const markerTextVisible = (node: Text, start: number, end: number): boolean => {
        const element = node.parentElement;
        if (element === null || window.getComputedStyle(element).visibility !== "visible") {
          return false;
        }
        // Bind visibility to the exact marker text range. A laid-out output
        // wrapper cannot make a hidden rendered marker count as visible.
        const range = document.createRange();
        range.setStart(node, start);
        range.setEnd(node, end);
        return Array.from(range.getClientRects()).some(
          (rectangle) => rectangle.width > 0 && rectangle.height > 0,
        );
      };
      const exactTextObservation = (
        root: Node,
        value: string,
      ): { count: number; visibleCount: number } => {
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
        let count = 0;
        let visibleCount = 0;
        for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
          const text = node.textContent ?? "";
          const lines = text.split(/\r?\n/);
          let offset = 0;
          for (let index = 0; index < lines.length; index += 1) {
            const line = lines[index];
            if (line === value) {
              count += 1;
              if (markerTextVisible(node as Text, offset, offset + line.length)) {
                visibleCount += 1;
              }
            }
            offset += line.length;
            if (index < lines.length - 1) {
              offset += text.slice(offset, offset + 2) === "\r\n" ? 2 : 1;
            }
          }
        }
        return { count, visibleCount };
      };
      const marker = (
        owner: HTMLElement | undefined,
        value: string,
      ): MarkerObservation => {
        const pageObservation = exactTextObservation(document.body, value);
        const ownerOutputs =
          owner === undefined
            ? []
            : Array.from(owner.querySelectorAll<HTMLElement>(".jp-Cell-outputArea"));
        const ownerObservation = ownerOutputs.reduce(
          (total, output) => {
            const observation = exactTextObservation(output, value);
            return {
              count: total.count + observation.count,
              visibleCount: total.visibleCount + observation.visibleCount,
            };
          },
          { count: 0, visibleCount: 0 },
        );
        return {
          pageCount: pageObservation.count,
          ownerCount: ownerObservation.count,
          ownerVisibleCount: ownerObservation.visibleCount,
        };
      };
      const stderrSelector =
        '.jp-Cell-outputArea .jp-RenderedText[data-mime-type="application/vnd.jupyter.stderr"]';
      const stderr = Array.from(
        document.querySelectorAll<HTMLElement>(
          `.jp-Notebook .jp-CodeCell ${stderrSelector}`,
        ),
      );
      const collectorStderr =
        collector === undefined
          ? []
          : Array.from(collector.querySelectorAll<HTMLElement>(stderrSelector));

      return {
        cellCount: cells.length,
        collectorSource: source(collector),
        identitySource: source(identity),
        collectorPrompt: prompt(collector),
        identityPrompt: prompt(identity),
        collectorMarker: marker(collector, collectorMarker),
        identityMarker: marker(identity, identityMarker),
        stderrCount: stderr.length,
        collectorStderrCount: collectorStderr.length,
        collectorAssertionStderrCount: collectorStderr.filter((node) => {
          const text = node.textContent ?? "";
          return text.includes(collectorAssertion) && text.includes("AssertionError");
        }).length,
      };
    },
    {
      collectorAssertion: COLLECTOR_ASSERTION,
      collectorMarker: COLLECTOR_MARKER,
      identityMarker: IDENTITY_MARKER,
    },
  );
}

type Vector3 = [number, number, number];

type ViewSnapshot = {
  modelId: string;
  viewId: string;
  mode: "surface" | "wireframe" | "points";
  transport: {
    modelSetCalls: number;
    saveChangesCalls: number;
    sendCalls: number;
    customMessageHandlers: number;
  };
  camera: {
    position: Vector3;
    target: Vector3;
    initialPosition: Vector3;
    initialTarget: Vector3;
    preset: "initial" | "orbit" | "pan" | "zoom" | "top" | "isometric";
  };
  lifecycle: {
    cleanupCount: number;
    activeListeners: number;
    pendingAnimationFrames: number;
    controlsDisposed: boolean;
    geometryDisposed: boolean;
    materialsDisposed: boolean;
    rendererDisposed: boolean;
  };
};

// `interfaces.python-rich-mesh-display` evidence O8 keeps the shipped view
// snapshot-only. It exposes an
// observation seam and no client-side control seam. The comm-close triggers
// that previously lived here as `closeComm()` are host-driven kernel actions
// in the fixtures; see closeMainDelegate/closeTemporaryDelegate below.
type ViewOracle = {
  snapshot(): ViewSnapshot;
};

type OracleElement = HTMLElement & {
  __eqioraN1Oracle?: ViewOracle;
};

class RuntimeTraffic {
  readonly externalRequests: string[] = [];
  readonly externalWebSockets: string[] = [];
  readonly loopbackRequests: string[] = [];
  readonly loopbackWebSockets: string[] = [];

  constructor(page: Page) {
    page.on("request", (request) => this.classify(request.url(), false));
    page.on("websocket", (socket) => this.classify(socket.url(), true));
  }

  private classify(raw: string, websocket: boolean): void {
    const url = new URL(raw);
    if (url.protocol === "data:" || url.protocol === "blob:") return;
    const target = websocket
      ? url.hostname === "127.0.0.1"
        ? this.loopbackWebSockets
        : this.externalWebSockets
      : url.hostname === "127.0.0.1"
        ? this.loopbackRequests
        : this.externalRequests;
    target.push(raw);
  }

  expectLoopbackOnly(): void {
    expect(this.externalRequests).toEqual([]);
    expect(this.externalWebSockets).toEqual([]);
    expect(this.loopbackRequests.length).toBeGreaterThan(0);
    expect(this.loopbackWebSockets.length).toBeGreaterThan(0);
    for (const raw of [...this.loopbackRequests, ...this.loopbackWebSockets]) {
      expect(new URL(raw).hostname).toBe("127.0.0.1");
    }
  }
}

// `interfaces.python-rich-mesh-display` evidence O9 discharges
// `client_write_or_bridge` by observing the
// transport itself, not the view's self-reported counters. Any client model
// write, save, send, or state bridge must serialize the model identity into a
// client-to-server payload (a Jupyter `comm_msg` names its comm_id, which is
// the model id; a host model update must name its model), so a client payload
// containing a mesh model id is the observable that could be nonzero. The
// `ViewSnapshot.transport` assertions below remain only as implementation-
// reported corroboration; they are written by the implementation and cannot
// falsify it.
// Bounds: the predicate is literal containment of the kernel-generated 32-hex
// model ids in utf8-decoded payloads. A write path that never serializes the
// model identity in any client payload would evade it; the declared-handler
// half that produces no traffic (msg:custom registration, save_changes) is
// covered by the static module-text probes in test_rich_mesh_display.py.
class ClientPayloadTraffic {
  readonly payloads: string[] = [];

  constructor(page: Page) {
    page.on("request", (request) => {
      const body = request.postData();
      this.payloads.push(body === null ? request.url() : `${request.url()}\n${body}`);
    });
    page.on("websocket", (socket) => {
      socket.on("framesent", (frame) => {
        const payload = frame.payload;
        this.payloads.push(typeof payload === "string" ? payload : payload.toString("utf8"));
      });
    });
  }

  naming(modelIds: readonly string[]): string[] {
    const ids = modelIds.filter((id) => id !== "");
    return this.payloads.filter((payload) => ids.some((id) => payload.includes(id)));
  }
}

const OBSERVER_POSITIVE_PROBE = "/eqiora-issue312-o9-observer-positive/";

// Ordinary positive path before any negative probe: a deliberate loopback
// request naming the model id must be visible to the observer before its
// zeroes are trusted. The host answers 404; only the request matters.
async function proveClientPayloadObserverSees(
  page: Page,
  traffic: ClientPayloadTraffic,
  modelId: string,
): Promise<void> {
  await page.evaluate(
    (probe) =>
      fetch(probe).then(
        () => undefined,
        () => undefined,
      ),
    `${OBSERVER_POSITIVE_PROBE}${modelId}`,
  );
  await expect
    .poll(
      () =>
        traffic
          .naming([modelId])
          .filter((payload) => payload.includes(OBSERVER_POSITIVE_PROBE)).length,
    )
    .toBeGreaterThan(0);
}

function hostName(): "jupyterlab" | "marimo" {
  const project = test.info().project.name;
  if (project === "jupyterlab-4.6.2") return "jupyterlab";
  if (project === "marimo-0.23.16") return "marimo";
  throw new Error(`unexpected exact host project ${project}`);
}

async function prepareHost(page: Page): Promise<void> {
  await page.goto("");
  if (hostName() === "jupyterlab") {
    await expect(page.locator(".jp-Notebook")).toBeVisible({ timeout: 60_000 });
    const classifyInlineFailure = isInlineJupyterClassificationTarget();
    let before: JupyterDomSnapshot | null = null;
    if (classifyInlineFailure) {
      try {
        before = await collectJupyterDomSnapshot(page);
      } catch {
        before = null;
      }
    }
    // Make the notebook the current widget before opening the Run menu:
    // JupyterLab 4.6.2 labels these semantic commands "Run All" / "Restart Kernel
    // and Run All" until a notebook is current, and the labels do not change while
    // the menu stays open.
    await page.locator(".jp-Notebook .jp-Cell").first().click();
    const runMenu = page.locator(".lm-MenuBar-item").filter({ hasText: /^Run$/ });
    await runMenu.click();
    // Anchor the label: the unanchored /Run All Cells/ also matches
    // "Restart Kernel and Run All Cells…", which strict mode refuses to click.
    await page
      .locator(".lm-Menu-item")
      .filter({ hasText: /^Run All Cells$/ })
      .click();
    try {
      await expect(page.getByText(COLLECTOR_MARKER, { exact: true })).toBeVisible({
        timeout: 60_000,
      });
    } catch (error) {
      if (!classifyInlineFailure) throw error;
      let atom: JupyterFailureAtom = "contradiction";
      if (before !== null) {
        try {
          const selected = classifyJupyterSnapshot({
            before,
            after: await collectJupyterDomSnapshot(page),
          });
          atom = selected === "ordinary-success" ? "contradiction" : selected;
        } catch {
          atom = "contradiction";
        }
      }
      throw new Error(atom);
    }
    await expect(page.getByText(IDENTITY_MARKER, { exact: true })).toBeVisible();
  }
  await expect(page.locator(VIEW_SELECTOR)).toHaveCount(4, { timeout: 60_000 });
}

async function oracleHandle(view: Locator): Promise<JSHandle<ViewOracle>> {
  return view.evaluateHandle((element: OracleElement) => {
    const oracle = element.__eqioraN1Oracle;
    if (oracle === undefined) throw new Error("N1 view omitted its private oracle observation seam");
    return oracle;
  });
}

async function snapshot(view: Locator): Promise<ViewSnapshot> {
  return view.evaluate((element: OracleElement) => {
    const oracle = element.__eqioraN1Oracle;
    if (oracle === undefined) throw new Error("N1 view omitted its private oracle observation seam");
    return oracle.snapshot();
  });
}

async function handleSnapshot(handle: JSHandle<ViewOracle>): Promise<ViewSnapshot> {
  return handle.evaluate((oracle) => oracle.snapshot());
}

function subtract(left: Vector3, right: Vector3): Vector3 {
  return [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
}

function distanceSquared(position: Vector3, target: Vector3): number {
  const difference = subtract(position, target);
  return difference[0] ** 2 + difference[1] ** 2 + difference[2] ** 2;
}

function expectNonzero(vector: Vector3): void {
  expect(vector).not.toEqual([0, 0, 0]);
}

function expectDisposed(state: ViewSnapshot): void {
  expect(state.lifecycle).toEqual({
    cleanupCount: 1,
    activeListeners: 0,
    pendingAnimationFrames: 0,
    controlsDisposed: true,
    geometryDisposed: true,
    materialsDisposed: true,
    rendererDisposed: true,
  });
}

async function activate(view: Locator, name: string, keyboard: boolean): Promise<void> {
  const button = view.getByRole("button", { name, exact: true });
  await expect(button).toBeVisible();
  if (keyboard) {
    await button.focus();
    await expect(button).toBeFocused();
    await pageKeyboard(button, "Enter");
  } else {
    await button.click();
  }
}

async function pageKeyboard(locator: Locator, key: string): Promise<void> {
  await locator.press(key);
}

async function exerciseIndependentCameraOperations(view: Locator): Promise<void> {
  const initial = await snapshot(view);
  expect(initial.camera.position).toEqual(initial.camera.initialPosition);
  expect(initial.camera.target).toEqual(initial.camera.initialTarget);

  await activate(view, "Orbit camera", false);
  const orbited = await snapshot(view);
  expect(orbited.camera.preset).toBe("orbit");
  expect(orbited.camera.target).toEqual(initial.camera.target);
  expect(distanceSquared(orbited.camera.position, orbited.camera.target)).toBe(
    distanceSquared(initial.camera.position, initial.camera.target),
  );
  expect(subtract(orbited.camera.position, orbited.camera.target)).not.toEqual(
    subtract(initial.camera.position, initial.camera.target),
  );

  await activate(view, "Reset camera", true);
  expect((await snapshot(view)).camera).toMatchObject({
    position: initial.camera.initialPosition,
    target: initial.camera.initialTarget,
    preset: "initial",
  });

  const beforePan = await snapshot(view);
  await activate(view, "Pan camera", true);
  const panned = await snapshot(view);
  const cameraDisplacement = subtract(panned.camera.position, beforePan.camera.position);
  const targetDisplacement = subtract(panned.camera.target, beforePan.camera.target);
  expectNonzero(cameraDisplacement);
  expect(cameraDisplacement).toEqual(targetDisplacement);
  expect(subtract(panned.camera.position, panned.camera.target)).toEqual(
    subtract(beforePan.camera.position, beforePan.camera.target),
  );
  expect(panned.camera.preset).toBe("pan");

  await activate(view, "Reset camera", false);
  const beforeZoom = await snapshot(view);
  await activate(view, "Zoom camera", false);
  const zoomed = await snapshot(view);
  const beforeDirection = subtract(beforeZoom.camera.position, beforeZoom.camera.target);
  const afterDirection = subtract(zoomed.camera.position, zoomed.camera.target);
  expect(zoomed.camera.target).toEqual(beforeZoom.camera.target);
  expect(distanceSquared(zoomed.camera.position, zoomed.camera.target)).not.toBe(
    distanceSquared(beforeZoom.camera.position, beforeZoom.camera.target),
  );
  expect(afterDirection[0] * beforeDirection[1]).toBe(
    afterDirection[1] * beforeDirection[0],
  );
  expect(afterDirection[0] * beforeDirection[2]).toBe(
    afterDirection[2] * beforeDirection[0],
  );
  expect(afterDirection[1] * beforeDirection[2]).toBe(
    afterDirection[2] * beforeDirection[1],
  );
  expect(
    afterDirection[0] * beforeDirection[0] +
      afterDirection[1] * beforeDirection[1] +
      afterDirection[2] * beforeDirection[2],
  ).toBeGreaterThan(0);
  expect(zoomed.camera.preset).toBe("zoom");

  await activate(view, "Reset camera", true);
  expect((await snapshot(view)).camera).toMatchObject({
    position: initial.camera.initialPosition,
    target: initial.camera.initialTarget,
    preset: "initial",
  });

  await activate(view, "Top view", false);
  expect((await snapshot(view)).camera.preset).toBe("top");
  await activate(view, "Isometric view", true);
  expect((await snapshot(view)).camera.preset).toBe("isometric");
  await activate(view, "Reset camera", false);
}

async function exerciseModesAndAccessibility(view: Locator): Promise<void> {
  const canvas = view.getByRole("img", { name: new RegExp(`Mesh.*${MESH_DIGEST}`) });
  await expect(canvas).toBeVisible();
  await expect(canvas).toHaveAttribute("tabindex", "0");
  await canvas.focus();
  const focusIsVisible = await canvas.evaluate((element) => {
    const style = getComputedStyle(element);
    return style.outlineStyle !== "none" || style.boxShadow !== "none";
  });
  expect(focusIsVisible).toBe(true);

  for (const [name, mode] of [
    ["Surface", "surface"],
    ["Wireframe", "wireframe"],
    ["Points", "points"],
  ] as const) {
    const button = view.getByRole("button", { name, exact: true });
    await button.focus();
    await button.press("Enter");
    await expect(button).toHaveAttribute("aria-pressed", "true");
    expect((await snapshot(view)).mode).toBe(mode);
  }
}

async function runJupyterCommand(page: Page, label: RegExp): Promise<void> {
  await page.keyboard.press("Control+Shift+c");
  const palette = page.locator(".lm-CommandPalette");
  await expect(palette).toBeVisible();
  const search = palette.locator("input");
  await search.fill(label.source.replaceAll("\\s+", " ").replaceAll("\\", ""));
  const item = palette.locator(".lm-CommandPalette-item").filter({ hasText: label }).first();
  await expect(item).toBeVisible();
  await item.click();
}

async function clearOneMainView(page: Page, view: Locator): Promise<void> {
  if (hostName() === "jupyterlab") {
    const cell = view.locator("xpath=ancestor::*[contains(@class, 'jp-CodeCell')]");
    await cell.click();
    // JupyterLab 4.6.2 has exactly two notebook clear commands: "Clear Cell
    // Output" (notebook:clear-cell-output, caption "Clear outputs for the
    // selected cells") and "Clear Outputs of All Cells". Only the former clears
    // the one selected cell, which is what this flow needs; the latter would
    // clear every view. The palette item's text is label + caption, so the
    // pattern stays unanchored.
    await runJupyterCommand(page, /Clear Cell Output/);
  } else {
    await page.getByRole("checkbox", { name: "Show third Mesh", exact: true }).uncheck();
  }
}

async function rerunOneMainView(page: Page): Promise<void> {
  if (hostName() === "jupyterlab") {
    // Select the third main-view cell explicitly: the kernel-side close
    // triggers below run other cells, so the selection no longer stays on
    // this cell across the whole flow. The cell carries a source marker
    // because after a comm close it holds no view element to anchor on.
    await page.locator(".jp-CodeCell").filter({ hasText: "EQIORA_THIRD_MAIN_VIEW" }).click();
    await page.keyboard.press("Control+Enter");
  } else {
    const checkbox = page.getByRole("checkbox", { name: "Show third Mesh", exact: true });
    if (await checkbox.isChecked()) await checkbox.uncheck();
    await checkbox.check();
  }
}

async function assertPythonMeshUnchanged(page: Page): Promise<void> {
  if (hostName() === "jupyterlab") {
    const cell = page
      .locator(".jp-CodeCell")
      .filter({ hasText: "assert_unchanged(mesh, accepted_snapshot)" });
    await cell.click();
    await page.keyboard.press("Control+Enter");
    await expect(cell.getByText("EQIORA_MESH_UNCHANGED", { exact: true })).toBeVisible();
  } else {
    await page
      .getByRole("button", { name: "Assert accepted Mesh unchanged", exact: true })
      .click();
    await expect(page.getByText("EQIORA_MESH_UNCHANGED", { exact: true })).toBeVisible();
  }
}

// `interfaces.python-rich-mesh-display` evidence O7 keeps the comm-close
// triggers as kernel-side affordances in
// the fixtures, driven through host UI this spec already drives (JupyterLab
// cell click + Control+Enter; a marimo button). The kernel names the target
// delegate solely from kernel-side state — the Mesh object the fixture holds,
// or a model id the fixture recorded at display time. The browser supplies
// only the activation gesture, never an identifier or any other value.
async function runJupyterTriggerCell(
  page: Page,
  sourceMarker: string,
  outputMarker: string,
): Promise<void> {
  const cell = page.locator(".jp-CodeCell").filter({ hasText: sourceMarker });
  await cell.click();
  await page.keyboard.press("Control+Enter");
  await expect(cell.getByText(outputMarker, { exact: true })).toBeVisible();
}

async function closeMainDelegate(page: Page): Promise<void> {
  if (hostName() === "jupyterlab") {
    await runJupyterTriggerCell(
      page,
      "EQIORA_CLOSE_MAIN_TRIGGER",
      "EQIORA_MAIN_DELEGATE_CLOSED",
    );
  } else {
    await page
      .getByRole("button", { name: "Close accepted Mesh delegate", exact: true })
      .click();
  }
}

async function closeTemporaryDelegate(page: Page): Promise<void> {
  if (hostName() === "jupyterlab") {
    await runJupyterTriggerCell(
      page,
      "EQIORA_CLOSE_TEMPORARY_TRIGGER",
      "EQIORA_TEMPORARY_DELEGATE_CLOSED",
    );
  } else {
    await page
      .getByRole("button", { name: "Close temporary Mesh delegate", exact: true })
      .click();
  }
}

test(BARE_MESH_TEST_TITLE, async ({
  page,
}) => {
  const traffic = new RuntimeTraffic(page);
  const clientTraffic = new ClientPayloadTraffic(page);
  await prepareHost(page);
  const views = page.locator(VIEW_SELECTOR);
  const initialStates = await Promise.all([0, 1, 2, 3].map((index) => snapshot(views.nth(index))));
  const mainModelId = initialStates[0].modelId;
  expect(mainModelId).not.toBe("");
  await proveClientPayloadObserverSees(page, clientTraffic, mainModelId);
  expect(initialStates.slice(0, 3).map((state) => state.modelId)).toEqual([
    mainModelId,
    mainModelId,
    mainModelId,
  ]);
  expect(new Set(initialStates.slice(0, 3).map((state) => state.viewId)).size).toBe(3);
  expect(initialStates[3].modelId).not.toBe(mainModelId);
  for (const state of initialStates) {
    expect(state.transport).toEqual({
      modelSetCalls: 0,
      saveChangesCalls: 0,
      sendCalls: 0,
      customMessageHandlers: 0,
    });
    expect(state.lifecycle.activeListeners).toBeGreaterThan(0);
    expect(state.lifecycle.pendingAnimationFrames).toBe(0);
  }

  const first = views.nth(0);
  const second = views.nth(1);
  await exerciseIndependentCameraOperations(first);
  await exerciseModesAndAccessibility(first);
  expect((await snapshot(second)).mode).toBe("surface");
  expect((await snapshot(first)).transport).toEqual({
    modelSetCalls: 0,
    saveChangesCalls: 0,
    sendCalls: 0,
    customMessageHandlers: 0,
  });
  await expect.poll(async () => (await snapshot(first)).lifecycle.pendingAnimationFrames).toBe(0);
  await assertPythonMeshUnchanged(page);

  const firstHandle = await oracleHandle(first);
  const secondHandle = await oracleHandle(second);
  const clearedHandle = await oracleHandle(views.nth(2));
  const temporaryHandle = await oracleHandle(views.nth(3));
  await clearOneMainView(page, views.nth(2));
  await expect(views).toHaveCount(3);
  expectDisposed(await handleSnapshot(clearedHandle));
  expect((await handleSnapshot(firstHandle)).lifecycle.cleanupCount).toBe(0);
  expect((await handleSnapshot(secondHandle)).lifecycle.cleanupCount).toBe(0);

  await rerunOneMainView(page);
  await expect(views).toHaveCount(4);
  const rerunStates = await Promise.all([0, 1, 2, 3].map((index) => snapshot(views.nth(index))));
  expect(rerunStates.filter((state) => state.modelId === mainModelId)).toHaveLength(3);

  // A comm close cleans every remaining view exactly once. Redisplay of the
  // same outer Mesh must then create a distinct fresh delegate/model. The
  // close originates kernel-side (O7); from the kernel's self.close() onward
  // this is the same propagation path the accepted evidence already observed.
  await closeMainDelegate(page);
  await expect(views).toHaveCount(1);
  expectDisposed(await handleSnapshot(firstHandle));
  expectDisposed(await handleSnapshot(secondHandle));
  expectDisposed(await handleSnapshot(clearedHandle));

  await rerunOneMainView(page);
  await expect(views).toHaveCount(2);
  const redisplayed = await Promise.all([snapshot(views.nth(0)), snapshot(views.nth(1))]);
  const freshIndex = redisplayed.findIndex(
    (state) => state.modelId !== initialStates[3].modelId,
  );
  expect(freshIndex).not.toBe(-1);
  const fresh = redisplayed[freshIndex];
  expect(fresh.modelId).not.toBe(mainModelId);
  const freshHandle = await oracleHandle(views.nth(freshIndex));

  await closeTemporaryDelegate(page);
  await expect(views).toHaveCount(1);
  expectDisposed(await handleSnapshot(temporaryHandle));
  // The main-close trigger names whatever delegate the kernel-held Mesh
  // currently owns, so the same affordance closes the fresh redisplayed one.
  await closeMainDelegate(page);
  await expect(views).toHaveCount(0);
  expectDisposed(await handleSnapshot(freshHandle));

  traffic.expectLoopbackOnly();

  // O9 negative, after the positive probe above proved the observer decodes
  // and matches: across the whole run — every render, camera and mode
  // interaction, clear, rerun, and all three kernel-side closes — no client
  // payload named any mesh model id except the deliberate probe.
  const meshModelIds = [mainModelId, initialStates[3].modelId, fresh.modelId];
  const flagged = clientTraffic.naming(meshModelIds);
  expect(flagged.filter((payload) => !payload.includes(OBSERVER_POSITIVE_PROBE))).toEqual([]);
  expect(flagged.length).toBeGreaterThan(0);
});

test("Jupyter collector marker failure classification is complete and fail-closed", () => {
  test.skip(
    test.info().project.name !== "jupyterlab-4.6.2",
    "Jupyter-only classification",
  );

  const absent = (): MarkerObservation => ({
    pageCount: 0,
    ownerCount: 0,
    ownerVisibleCount: 0,
  });
  const visible = (): MarkerObservation => ({
    pageCount: 1,
    ownerCount: 1,
    ownerVisibleCount: 1,
  });
  const dom = (overrides: Partial<JupyterDomSnapshot> = {}): JupyterDomSnapshot => ({
    cellCount: 9,
    collectorSource: COLLECTOR_SOURCE,
    identitySource: IDENTITY_SOURCE,
    collectorPrompt: "[ ]:",
    identityPrompt: "[ ]:",
    collectorMarker: absent(),
    identityMarker: absent(),
    stderrCount: 0,
    collectorStderrCount: 0,
    collectorAssertionStderrCount: 0,
    ...overrides,
  });
  const before = dom();
  const classify = (
    after: JupyterDomSnapshot,
    initial = before,
  ): JupyterClassification => classifyJupyterSnapshot({ before: initial, after });

  expect(
    classify(
      dom({
        collectorPrompt: "[1]:",
        identityPrompt: "[2]:",
        collectorMarker: visible(),
        identityMarker: visible(),
      }),
    ),
  ).toBe("ordinary-success");

  expect(isJupyterClassificationTarget("jupyterlab-4.6.2", BARE_MESH_TEST_TITLE)).toBe(
    true,
  );
  expect(
    isJupyterClassificationTarget("jupyterlab-4.6.2", WEBGL_FAILURE_TEST_TITLE),
  ).toBe(true);
  expect(
    isJupyterClassificationTarget("marimo-0.23.16", WEBGL_FAILURE_TEST_TITLE),
  ).toBe(false);
  expect(
    isJupyterClassificationTarget(
      "jupyterlab-4.6.2",
      "shipped view oracle is snapshot-only",
    ),
  ).toBe(false);

  expect(
    classify(
      dom({
        collectorPrompt: "[9007199254740993]:",
        identityPrompt: "[9007199254740994]:",
        collectorMarker: visible(),
        identityMarker: visible(),
      }),
      dom({
        collectorPrompt: "[9007199254740992]:",
        identityPrompt: "[9007199254740993]:",
      }),
    ),
  ).toBe("ordinary-success");

  expect(
    classify(
      dom({
        collectorPrompt: "[1]:",
        stderrCount: 1,
        collectorStderrCount: 1,
        collectorAssertionStderrCount: 1,
      }),
    ),
  ).toBe("collector-assertion");

  expect(classify(dom({ collectorPrompt: "[*]:" }))).toBe("collector-not-terminal");

  expect(
    classify(
      dom({
        collectorPrompt: "[1]:",
        identityPrompt: "[2]:",
        identityMarker: visible(),
      }),
    ),
  ).toBe("collector-success-dom-propagation");

  expect(
    classify(
      dom({
        collectorPrompt: "[1]:",
        identityPrompt: "[2]:",
        collectorMarker: { pageCount: 1, ownerCount: 1, ownerVisibleCount: 0 },
        identityMarker: visible(),
      }),
    ),
  ).toBe("collector-success-dom-propagation");

  expect(
    classify(
      dom({
        collectorPrompt: "[1]:",
        stderrCount: 1,
        collectorStderrCount: 1,
        collectorAssertionStderrCount: 0,
      }),
    ),
  ).toBe("contradiction");

  for (const identity of [
    { identityPrompt: "[2]:" },
    { identityMarker: visible() },
  ] satisfies Partial<JupyterDomSnapshot>[]) {
    expect(
      classify(
        dom({
          collectorPrompt: "[1]:",
          stderrCount: 1,
          collectorStderrCount: 1,
          collectorAssertionStderrCount: 1,
          ...identity,
        }),
      ),
    ).toBe("contradiction");
  }

  for (const contamination of [
    { collectorMarker: visible() },
    {
      stderrCount: 1,
      collectorStderrCount: 1,
      collectorAssertionStderrCount: 1,
    },
  ] satisfies Partial<JupyterDomSnapshot>[]) {
    expect(classify(dom({ collectorPrompt: "[*]:", ...contamination }))).toBe(
      "contradiction",
    );
  }

  expect(classify(dom({ collectorPrompt: "[1]:" }))).toBe("contradiction");

  for (const misplaced of [
    { collectorMarker: { pageCount: 1, ownerCount: 0, ownerVisibleCount: 0 } },
    { identityMarker: { pageCount: 1, ownerCount: 0, ownerVisibleCount: 0 } },
    { collectorMarker: { pageCount: 2, ownerCount: 2, ownerVisibleCount: 2 } },
    { identityMarker: { pageCount: 2, ownerCount: 1, ownerVisibleCount: 1 } },
  ] satisfies Partial<JupyterDomSnapshot>[]) {
    expect(
      classify(
        dom({
          collectorPrompt: "[1]:",
          identityPrompt: "[2]:",
          collectorMarker: visible(),
          identityMarker: visible(),
          ...misplaced,
        }),
      ),
    ).toBe("contradiction");
  }

  expect(classify(dom({ identityMarker: visible() }))).toBe("contradiction");

  for (const invalidAfter of [
    { cellCount: 8 },
    { collectorSource: `${COLLECTOR_SOURCE}\npass` },
    { identitySource: `${IDENTITY_SOURCE}\npass` },
    { collectorPrompt: "1" },
    { identityPrompt: "2" },
  ] satisfies Partial<JupyterDomSnapshot>[]) {
    expect(classify(dom(invalidAfter))).toBe("contradiction");
  }
  for (const invalidBefore of [
    { collectorMarker: visible() },
    { identityMarker: visible() },
    { stderrCount: 1 },
    { collectorPrompt: "1" },
    { identityPrompt: "2" },
  ] satisfies Partial<JupyterDomSnapshot>[]) {
    expect(classify(dom(), dom(invalidBefore))).toBe("contradiction");
  }
});

// Precommitted falsifier for `interfaces.python-rich-mesh-display` evidence O6:
// the
// shipped per-view oracle must be snapshot-only. Expected RED on any build
// whose oracle still carries the retired `closeComm` client-to-kernel control
// member; GREEN once the write half is deleted. Kept as its own test so the
// pre-O6 failure stays precisely isolated from the flow above.
test("shipped view oracle is snapshot-only", async ({ page }) => {
  await prepareHost(page);
  const memberNames = await page
    .locator(VIEW_SELECTOR)
    .first()
    .evaluate((element: OracleElement) => {
      const oracle = element.__eqioraN1Oracle;
      if (oracle === undefined) {
        throw new Error("N1 view omitted its private oracle observation seam");
      }
      const names: string[] = [];
      for (
        let current: object | null = oracle;
        current !== null && current !== Object.prototype;
        current = Object.getPrototypeOf(current)
      ) {
        names.push(...Object.getOwnPropertyNames(current));
      }
      return names;
    });
  expect(memberNames).toContain("snapshot");
  expect(memberNames).not.toContain("closeComm");
});

test(WEBGL_FAILURE_TEST_TITLE, async ({ page }) => {
  const traffic = new RuntimeTraffic(page);
  await page.addInitScript(() => {
    const original = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (
      contextId: string,
      options?: unknown,
    ): RenderingContext | null {
      if (contextId === "webgl" || contextId === "webgl2") return null;
      return original.call(this, contextId, options as never) as RenderingContext | null;
    };
  });
  await prepareHost(page);
  // Scope to the Eqiora views: JupyterLab 4.6.2 ships its own page-wide
  // role="alert" chrome (the react-toastify news-opt-in notification), so a
  // page-wide count would bind this oracle to an incidental host detail rather
  // than to the four view diagnostics the claim is about.
  const diagnostics = page.locator(VIEW_SELECTOR).locator('[role="alert"]');
  await expect(diagnostics).toHaveCount(4);
  for (let index = 0; index < 4; index += 1) {
    await expect(diagnostics.nth(index)).toContainText("WebGL");
    await expect(diagnostics.nth(index)).toContainText(MESH_DIGEST);
  }
  traffic.expectLoopbackOnly();
});
