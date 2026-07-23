import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { cadBridge } from "./cad-bridge";
import { CAD_VIEW_PROTOCOL, type CadProjection, type CadSelectionRequest } from "./cad-protocol";
import { cadSelectionReducer, initialCadSelectionState } from "./cad-workflow";

export type CadSessionStatus = "idle" | "loading" | "ready" | "unavailable";

/**
 * Own the asynchronous CAD application boundary for one exact compiled Model.
 * A Model change invalidates both projected geometry and in-flight selection.
 */
export function useCadSession(modelDigest: string | null) {
  const previewSequence = useRef(0);
  const selectionSequence = useRef(0);
  const [projection, setProjection] = useState<CadProjection | null>(null);
  const [selection, selectionDispatch] = useReducer(
    cadSelectionReducer,
    undefined,
    initialCadSelectionState,
  );
  const [status, setStatus] = useState<CadSessionStatus>("idle");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const requestId = ++previewSequence.current;
    selectionSequence.current += 1;
    setError(null);
    if (modelDigest === null) {
      setProjection(null);
      selectionDispatch({ type: "context-changed", projection: null });
      setStatus("idle");
      return;
    }
    setProjection(null);
    setStatus("loading");
    void cadBridge.preview({ protocol: CAD_VIEW_PROTOCOL, modelDigest }).then((response) => {
      if (requestId !== previewSequence.current) return;
      if (response.result === null) {
        selectionDispatch({ type: "context-changed", projection: null });
        setStatus("unavailable");
        return;
      }
      setProjection(response.result);
      selectionDispatch({ type: "context-changed", projection: response.result });
      setStatus("ready");
    });
  }, [modelDigest]);

  const requestSelection = useCallback(async (request: CadSelectionRequest) => {
    const requestId = ++selectionSequence.current;
    selectionDispatch({ type: "selection-started", requestId, request });
    setError(null);
    const response = await cadBridge.select(request);
    if (requestId !== selectionSequence.current) return;
    selectionDispatch({ type: "selection-finished", requestId, result: response.result });
    if (response.result === null) {
      setError(response.diagnostics[0]?.message ?? "Selection could not be replayed.");
    }
  }, []);

  return { projection, selection, status, error, requestSelection } as const;
}
