import { expect, type JSHandle, type Locator, type Page, test } from "@playwright/test";

const MESH_DIGEST = "148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a";
const VIEW_SELECTOR = `[data-eqiora-mesh-digest="${MESH_DIGEST}"]`;

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

type ViewOracle = {
  snapshot(): ViewSnapshot;
  closeComm(): Promise<void> | void;
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
    await expect(page.getByText("EQIORA_TEMPORARY_MESH_COLLECTED", { exact: true })).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByText("EQIORA_MESH_UNCHANGED", { exact: true })).toBeVisible();
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

async function closeComm(handle: JSHandle<ViewOracle>): Promise<void> {
  await handle.evaluate(async (oracle) => oracle.closeComm());
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

test("bare Mesh owns exact interaction, lifecycle, identity, and loopback-only observations", async ({
  page,
}) => {
  const traffic = new RuntimeTraffic(page);
  await prepareHost(page);
  const views = page.locator(VIEW_SELECTOR);
  const initialStates = await Promise.all([0, 1, 2, 3].map((index) => snapshot(views.nth(index))));
  const mainModelId = initialStates[0].modelId;
  expect(mainModelId).not.toBe("");
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
  // same outer Mesh must then create a distinct fresh delegate/model.
  await closeComm(firstHandle);
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

  await closeComm(temporaryHandle);
  await expect(views).toHaveCount(1);
  expectDisposed(await handleSnapshot(temporaryHandle));
  await closeComm(freshHandle);
  await expect(views).toHaveCount(0);
  expectDisposed(await handleSnapshot(freshHandle));

  traffic.expectLoopbackOnly();
});

test("WebGL construction failure is an accessible failure rather than a blank pass", async ({
  page,
}) => {
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
