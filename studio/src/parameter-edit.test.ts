import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, expect, it, vi } from "vitest";
import { Inspector } from "./components";
import { studioCompileRequest } from "./control-protocol";
import { EXAMPLE_SOURCE } from "./example";
import { BRIDGE_PROTOCOL, type ProjectionNode } from "./protocol";

const valueEdit = {
  input: "2",
  validation: { value: 2, error: null },
  status: { kind: "idle" as const },
  disabledReason: null,
  onChange: () => {},
  onCommit: () => {},
};

afterEach(() => vi.unstubAllGlobals());

it("offers revision value edits only for Parameters even when a Field has a projected value", () => {
  const render = (kind: ProjectionNode["kind"]) =>
    renderToStaticMarkup(
      createElement(Inspector, {
        node: { id: `${kind}:x`, name: "x", kind, summary: "Scalar", dimension: "1", value: 1 },
        position: null,
        onNudge: () => {},
        valueEdit,
      }),
    );
  expect(render("parameter")).toContain("inspector-value-input");
  expect(render("field")).not.toContain("inspector-value-input");
});

it("rejects Field preview and commit while retaining Parameter revision edits", async () => {
  vi.stubGlobal("window", {});
  const { studioBridge } = await import("./bridge");
  const loaded = await studioBridge.loadReadOnlyExample(
    "decay",
    studioCompileRequest("parameter-edit", "decay.eqi", EXAMPLE_SOURCE),
  );
  const document = loaded.result;
  if (document === null) throw new Error("Preview example must load");
  const request = {
    protocol: BRIDGE_PROTOCOL,
    digest: document.digest,
    targetId: "Parameter:rate",
    value: 2,
  };
  const preview = await studioBridge.previewValueEdit(request);
  if (preview.result === null) throw new Error("Parameter edit must preview");
  const state = document.nodes.find((node) => node.id === "Field:state");
  if (state === undefined) throw new Error("Example retains the state Field");
  expect(state.value).toBeNull();
  // A projected state value must never turn the Field into an editable Parameter.
  Object.assign(state, { value: 1 });
  try {
    const fieldRequest = { ...request, targetId: state.id };
    expect((await studioBridge.previewValueEdit(fieldRequest)).result).toBeNull();
    expect(
      (await studioBridge.commitValueEdit({ ...fieldRequest, planKey: preview.result.key })).result,
    ).toBeNull();
    const committed = await studioBridge.commitValueEdit({
      ...request,
      planKey: preview.result.key,
    });
    expect(committed.result?.document.revision).toBe(document.revision + 1);
    expect(
      committed.result?.document.nodes.find((node) => node.id === request.targetId)?.value,
    ).toBe(2);
  } finally {
    Object.assign(state, { value: null });
  }
});
