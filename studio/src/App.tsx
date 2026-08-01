import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import iconUrl from "../assets/icon.svg";
import {
  COMMAND_REGISTRY,
  type FocusTarget,
  type RunActivity,
  type RunBlock,
  resolveApplication,
  resolveCommandAvailability,
  resolveElementFocusId,
  type ValueEditBlock,
  type WorkspaceId,
} from "./application";
import { type StudioExample, studioBridge } from "./bridge";
import { CadAuthoredWorkspace } from "./cad-authored-workspace";
import { useCadSession } from "./cad-session";
import { CadWorkspace } from "./cad-workspace";
import { type CommandAvailability, CommandPalette } from "./command-palette";
import type { CommandId } from "./commands";
import { Icon, Inspector, ModelOutline, SourceEditor } from "./components";
import { studioCompileRequest } from "./control-protocol";
import { CylinderDemoSession, type CylinderDemoSessionState } from "./cylinder-demo-session";
import { CylinderDemoWorkspace } from "./cylinder-demo-workspace";
import { DcMotorDemoSession, type DcMotorDemoSessionState } from "./dc-motor-demo-session";
import { DcMotorDemoWorkspace } from "./dc-motor-demo-workspace";
import { DemoFailureBanner, DemoLoadState } from "./demo-state";
import { type DiagnosticPresentation, Diagnostics } from "./diagnostics";
import { CAD_EXAMPLE_SOURCE, EXAMPLE_SOURCE, SPATIAL_EXAMPLE_SOURCE } from "./example";
import { ExampleMenu } from "./example-menu";
import { FsiDemoSession, type FsiDemoSessionState } from "./fsi-demo-session";
import { FsiDemoWorkspace } from "./fsi-demo-workspace";
import { formatMessage } from "./messages";
import { ModelCanvas } from "./projection";
import { BRIDGE_PROTOCOL, type SourceSpan, type StudioDiagnostic } from "./protocol";
import { Results } from "./results";
import { validateRunConfiguration } from "./run-configuration";
import { RunPanel } from "./run-panel";
import { scalarFieldDataBridge } from "./scalar-field-bridge";
import { initialScalarFieldSessionState, ScalarFieldDataSession } from "./scalar-field-session";
import { ScalarFieldWorkspace } from "./scalar-field-workspace";
import { sourceLocationLabel, sourceSelection } from "./source-span";
import { SpatialResults } from "./spatial-results";
import { SpatialRunPanel } from "./spatial-run-panel";
import {
  initialSpatialWorkflowState,
  spatialPlanIsCurrent,
  spatialWorkflowReducer,
  validateSpatialConfiguration,
} from "./spatial-workflow";
import { currentLayout, initialStudioState, type NodeLayout, studioReducer } from "./state";
import { StructuralDemoSession, type StructuralDemoSessionState } from "./structural-demo-session";
import { StructuralDemoWorkspace } from "./structural-demo-workspace";
import { nativeUnstructuredFieldDataBridge } from "./unstructured-field-bridge";
import { validateValueEditInput } from "./value-edit";
import { readWorkspace, writeWorkspace } from "./workspace";

export function App() {
  const [state, dispatch] = useReducer(studioReducer, EXAMPLE_SOURCE, initialStudioState);
  const [spatialState, spatialDispatch] = useReducer(
    spatialWorkflowReducer,
    undefined,
    initialSpatialWorkflowState,
  );
  const compileSequence = useRef(0);
  const runPreviewSequence = useRef(0);
  const runSequence = useRef(0);
  const spatialPreviewSequence = useRef(0);
  const spatialRunSequence = useRef(0);
  const valueEditPreviewSequence = useRef(0);
  const valueEditCommitSequence = useRef(0);
  const compiledOnce = useRef(false);
  const sourceEditor = useRef<HTMLTextAreaElement>(null);
  const relationView = useRef<HTMLElement>(null);
  const latestSource = useRef(state.source);
  const [requestedWorkspace, setRequestedWorkspace] = useState<WorkspaceId>("relations");
  const [scalarFieldState, setScalarFieldState] = useState(initialScalarFieldSessionState());
  const [selectedFieldOrdinal, setSelectedFieldOrdinal] = useState(0);
  const [selectedFieldWorkflow, setSelectedFieldWorkflow] = useState<
    "scalar-elliptic" | "cylinder-stokes" | null
  >(null);
  const [selectedCylinderVertex, setSelectedCylinderVertex] = useState(0);
  const [cylinderState, setCylinderState] = useState<CylinderDemoSessionState>({
    kind: "idle",
  });
  const [dcMotorState, setDcMotorState] = useState<DcMotorDemoSessionState>({
    kind: "idle",
  });
  const [structuralState, setStructuralState] = useState<StructuralDemoSessionState>({
    kind: "idle",
  });
  const [fsiState, setFsiState] = useState<FsiDemoSessionState>({ kind: "idle" });
  const scalarFieldSession = useMemo(
    () => new ScalarFieldDataSession(scalarFieldDataBridge, setScalarFieldState),
    [],
  );
  const cylinderSession = useMemo(
    () =>
      new CylinderDemoSession(studioBridge, nativeUnstructuredFieldDataBridge, setCylinderState),
    [],
  );
  const dcMotorSession = useMemo(() => new DcMotorDemoSession(studioBridge, setDcMotorState), []);
  const structuralSession = useMemo(
    () => new StructuralDemoSession(studioBridge, setStructuralState),
    [],
  );
  const fsiSession = useMemo(() => new FsiDemoSession(studioBridge, setFsiState), []);
  latestSource.current = state.source;
  const sourceEdited = state.compiledSource !== state.source;
  const {
    projection: cadProjection,
    selection: cadSelectionState,
    status: cadStatus,
    error: cadError,
    requestSelection: requestCadSelection,
  } = useCadSession(state.document === null || sourceEdited ? null : state.document.digest);
  const cylinderStatus =
    cylinderState.kind === "solving" || cylinderState.kind === "loading-field"
      ? "running"
      : cylinderState.kind;
  const dcMotorStatus = dcMotorState.kind;
  const structuralStatus = structuralState.kind;
  const fsiStatus = fsiState.kind;
  const fieldWorkflow =
    selectedFieldWorkflow === "scalar-elliptic" &&
    spatialState.latestResult?.plan.requirements.spatialDimension !== 2
      ? null
      : selectedFieldWorkflow;
  const application = useMemo(
    () =>
      resolveApplication(
        {
          acceptedProjection: state.document,
          cad: {
            status: cadStatus,
            acceptedModelDigest: cadProjection?.modelDigest ?? null,
          },
          cylinderStatus,
          dcMotorStatus,
          fsiStatus,
          structuralStatus,
          fieldWorkflow,
        },
        requestedWorkspace,
      ),
    [
      cadProjection?.modelDigest,
      cadStatus,
      cylinderStatus,
      dcMotorStatus,
      fsiStatus,
      structuralStatus,
      fieldWorkflow,
      requestedWorkspace,
      state.document,
    ],
  );
  const activeWorkspace = application.workspace;
  const sourceAncestor =
    state.document !== null &&
    state.sourceDigest !== null &&
    state.document.digest !== state.sourceDigest;
  const spatialWorkflow = state.document?.workflows.scalarElliptic ?? null;
  const isSpatialDocument = spatialWorkflow !== null;
  const layout = currentLayout(state);
  const selectedNode = useMemo(
    () => state.document?.nodes.find((node) => node.id === state.selectedNodeId) ?? null,
    [state.document, state.selectedNodeId],
  );
  const runValidation = useMemo(
    () => validateRunConfiguration(state.runConfiguration),
    [state.runConfiguration],
  );
  const spatialValidation = useMemo(
    () =>
      spatialWorkflow === null
        ? null
        : validateSpatialConfiguration(
            spatialState.configuration,
            spatialWorkflow.spatialDimension,
            spatialWorkflow.maximumHostWorkers,
          ),
    [spatialState.configuration, spatialWorkflow],
  );
  const valueEditValidation = useMemo(
    () =>
      selectedNode?.value === null || selectedNode?.value === undefined
        ? { value: null, error: null }
        : validateValueEditInput(state.valueEditInput, selectedNode.value),
    [selectedNode?.value, state.valueEditInput],
  );
  const diagnostics = useMemo(() => {
    const present = (
      diagnostic: StudioDiagnostic,
      diagnosticSource: string | null,
    ): DiagnosticPresentation => ({
      diagnostic,
      location:
        diagnostic.span === null || diagnosticSource === null
          ? null
          : sourceLocationLabel(diagnosticSource, diagnostic.span),
      navigable:
        diagnostic.span !== null && diagnosticSource !== null && diagnosticSource === state.source,
    });
    return [
      ...state.compileDiagnostics.map((diagnostic) =>
        present(diagnostic, state.compileDiagnosticSource),
      ),
      ...state.runPlanDiagnostics.map((diagnostic) => present(diagnostic, state.compiledSource)),
      ...state.runDiagnostics.map((diagnostic) => present(diagnostic, state.compiledSource)),
      ...state.valueEditDiagnostics.map((diagnostic) => present(diagnostic, state.compiledSource)),
      ...spatialState.planDiagnostics.map((diagnostic) =>
        present(diagnostic, state.compiledSource),
      ),
      ...spatialState.runDiagnostics.map((diagnostic) => present(diagnostic, state.compiledSource)),
    ];
  }, [
    state.compileDiagnosticSource,
    state.compileDiagnostics,
    state.compiledSource,
    state.runPlanDiagnostics,
    state.runDiagnostics,
    state.valueEditDiagnostics,
    state.source,
    spatialState.planDiagnostics,
    spatialState.runDiagnostics,
  ]);
  const runResult = state.latestRun?.result ?? null;
  const spatialResult = spatialState.latestResult;
  const runConfigurationStale =
    state.latestRun !== null &&
    (runValidation.value === null ||
      runValidation.value.endTime !== state.latestRun.configuration.endTime ||
      runValidation.value.maxStep !== state.latestRun.configuration.maxStep);
  const runRevisionStale =
    runResult !== null && state.document !== null && runResult.digest !== state.document.digest;
  const runPlanCurrent =
    !isSpatialDocument &&
    !sourceEdited &&
    state.runPlanStatus.kind === "ready" &&
    state.document !== null &&
    runValidation.value !== null &&
    state.runPlanStatus.digest === state.document.digest &&
    state.runPlanStatus.configuration.endTime === runValidation.value.endTime &&
    state.runPlanStatus.configuration.maxStep === runValidation.value.maxStep;
  const spatialPlanCurrent =
    spatialWorkflow !== null &&
    state.document !== null &&
    spatialValidation?.value !== null &&
    spatialValidation?.value !== undefined &&
    spatialPlanIsCurrent(spatialState, state.document.digest, spatialValidation.value);
  const spatialResultStale =
    spatialResult !== null &&
    (sourceEdited ||
      state.document === null ||
      spatialResult.digest !== state.document.digest ||
      spatialResult.plan.realizationRevision !== spatialState.realizationRevision ||
      spatialValidation?.value === null ||
      spatialValidation?.value === undefined ||
      spatialResult.plan.discretization.method !== spatialValidation.value.method ||
      spatialResult.plan.discretization.cellsPerAxis !== spatialValidation.value.cellsPerAxis ||
      spatialResult.plan.placement.workers !== spatialValidation.value.workers);
  const revisionNavigationBlocked =
    state.runStatus.kind === "running" ||
    state.runStatus.kind === "cancelling" ||
    spatialState.runStatus.kind === "running" ||
    state.valueEditStatus.kind === "committing";
  const canUndo = state.revisionIndex > 0 && !revisionNavigationBlocked;
  const canRedo =
    state.revisionIndex >= 0 &&
    state.revisionIndex < state.revisionLineage.length - 1 &&
    !revisionNavigationBlocked;
  const valueEditBlock: ValueEditBlock =
    state.runStatus.kind === "running" ||
    state.runStatus.kind === "cancelling" ||
    spatialState.runStatus.kind === "running"
      ? "run"
      : sourceEdited
        ? "source"
        : null;
  const valueEditDisabledReason =
    valueEditBlock === "run"
      ? formatMessage("command.reason.edit-run")
      : valueEditBlock === "source"
        ? formatMessage("command.reason.edit-source")
        : null;
  const valueEditExpanded =
    valueEditValidation.value !== null ||
    state.valueEditStatus.kind === "previewing" ||
    state.valueEditStatus.kind === "ready" ||
    state.valueEditStatus.kind === "committing";

  const revealSourceSpan = useCallback((source: string, span: SourceSpan) => {
    const editor = sourceEditor.current;
    if (editor === null) {
      return;
    }
    const selection = sourceSelection(source, span);
    editor.focus();
    editor.setSelectionRange(selection.start, selection.end, "forward");
  }, []);

  const submitSource = useCallback(
    async (source: string, example: StudioExample | null) => {
      const requestId = ++compileSequence.current;
      if (source !== state.source) {
        latestSource.current = source;
        dispatch({ type: "source-edited", source });
      }
      dispatch({ type: "compile-started", requestId });
      const controlRequest = studioCompileRequest(
        `studio.compile:${requestId}`,
        "untitled.eqi",
        source,
      );
      const response =
        example === null
          ? await studioBridge.compile(controlRequest)
          : await studioBridge.loadReadOnlyExample(example, controlRequest);
      let workspaceLayout: NodeLayout | null = null;
      if (response.result !== null) {
        try {
          workspaceLayout = readWorkspace(window.localStorage, response.result.digest);
        } catch {
          workspaceLayout = null;
        }
      }
      dispatch({
        type: "compile-finished",
        requestId,
        compiledSource: source,
        document: response.result,
        workspaceLayout,
        diagnostics: response.diagnostics,
      });
      const firstSpan = response.diagnostics.find((diagnostic) => diagnostic.span !== null)?.span;
      if (
        response.result === null &&
        firstSpan !== null &&
        firstSpan !== undefined &&
        requestId === compileSequence.current &&
        latestSource.current === source
      ) {
        window.requestAnimationFrame(() => {
          if (requestId === compileSequence.current && latestSource.current === source) {
            revealSourceSpan(source, firstSpan);
          }
        });
      }
    },
    [revealSourceSpan, state.source],
  );

  const compile = useCallback(() => submitSource(state.source, null), [state.source, submitSource]);

  const loadReadOnlyExample = useCallback(
    (example: StudioExample, source: string) => submitSource(source, example),
    [submitSource],
  );

  const openCadExample = useCallback(async () => {
    await loadReadOnlyExample("cad", CAD_EXAMPLE_SOURCE);
    setRequestedWorkspace("geometry");
  }, [loadReadOnlyExample]);

  const openSpatialExample = useCallback(async () => {
    setSelectedFieldWorkflow(null);
    setRequestedWorkspace("relations");
    await loadReadOnlyExample("spatial", SPATIAL_EXAMPLE_SOURCE);
  }, [loadReadOnlyExample]);

  useEffect(() => {
    if (!compiledOnce.current) {
      compiledOnce.current = true;
      void loadReadOnlyExample("decay", EXAMPLE_SOURCE);
    }
  }, [loadReadOnlyExample]);

  useEffect(() => {
    spatialDispatch({
      type: "context-changed",
      digest: spatialWorkflow === null ? null : (state.document?.digest ?? null),
    });
  }, [spatialWorkflow, state.document?.digest]);

  useEffect(() => {
    scalarFieldSession.setContext(spatialResult);
    setSelectedFieldOrdinal(0);
  }, [scalarFieldSession, spatialResult]);

  useEffect(() => {
    const configuration = runValidation.value;
    const document = state.document;
    if (configuration === null || document === null || sourceEdited || isSpatialDocument) {
      return;
    }
    const requestId = ++runPreviewSequence.current;
    dispatch({ type: "run-preview-started", requestId });
    void studioBridge
      .previewRun({
        protocol: BRIDGE_PROTOCOL,
        digest: document.digest,
        endTime: configuration.endTime,
        maxStep: configuration.maxStep,
      })
      .then((response) => {
        dispatch({
          type: "run-preview-finished",
          requestId,
          digest: document.digest,
          configuration,
          plan: response.result,
          diagnostics: response.diagnostics,
        });
      });
  }, [isSpatialDocument, runValidation.value, sourceEdited, state.document]);

  useEffect(() => {
    const configuration = spatialValidation?.value ?? null;
    const document = state.document;
    if (
      configuration === null ||
      document === null ||
      spatialWorkflow === null ||
      sourceEdited ||
      spatialState.contextDigest !== document.digest
    ) {
      return;
    }
    const requestId = ++spatialPreviewSequence.current;
    const digest = document.digest;
    const realizationRevision = spatialState.realizationRevision;
    spatialDispatch({ type: "preview-started", requestId });
    const timer = window.setTimeout(() => {
      void studioBridge
        .previewSpatialRealization({
          protocol: BRIDGE_PROTOCOL,
          digest,
          realizationRevision,
          method: configuration.method,
          cellsPerAxis: configuration.cellsPerAxis,
          workers: configuration.workers,
        })
        .then((response) => {
          spatialDispatch({
            type: "preview-finished",
            requestId,
            digest,
            realizationRevision,
            configuration,
            plan: response.result,
            diagnostics: response.diagnostics,
          });
        });
    }, 180);
    return () => window.clearTimeout(timer);
  }, [
    sourceEdited,
    spatialState.contextDigest,
    spatialState.realizationRevision,
    spatialValidation?.value,
    spatialWorkflow,
    state.document,
  ]);

  useEffect(() => {
    const document = state.document;
    const target = selectedNode;
    const value = valueEditValidation.value;
    if (
      document === null ||
      target === null ||
      value === null ||
      valueEditDisabledReason !== null ||
      !["field", "parameter"].includes(target.kind)
    ) {
      return;
    }
    const requestId = ++valueEditPreviewSequence.current;
    const digest = document.digest;
    const targetId = target.id;
    const input = state.valueEditInput;
    dispatch({ type: "value-edit-preview-started", requestId });
    const timer = window.setTimeout(() => {
      void studioBridge
        .previewValueEdit({
          protocol: BRIDGE_PROTOCOL,
          digest,
          targetId,
          value,
        })
        .then((response) => {
          dispatch({
            type: "value-edit-preview-finished",
            requestId,
            digest,
            targetId,
            input,
            plan: response.result,
            diagnostics: response.diagnostics,
          });
        });
    }, 180);
    return () => window.clearTimeout(timer);
  }, [
    selectedNode,
    state.document,
    state.valueEditInput,
    valueEditDisabledReason,
    valueEditValidation.value,
  ]);

  useEffect(() => {
    const document = state.document;
    if (document === null) return;
    const savedLayout = state.layoutsByDigest[document.digest];
    if (savedLayout === undefined) return;
    const timer = window.setTimeout(() => {
      try {
        writeWorkspace(window.localStorage, document.digest, savedLayout);
      } catch {
        // Workspace persistence is optional and never affects canonical state.
      }
    }, 250);
    return () => window.clearTimeout(timer);
  }, [state.document, state.layoutsByDigest]);

  const run = useCallback(async () => {
    const configuration = runValidation.value;
    if (
      state.document === null ||
      sourceEdited ||
      configuration === null ||
      state.runPlanStatus.kind !== "ready" ||
      !runPlanCurrent
    ) {
      return;
    }
    const requestId = ++runSequence.current;
    const runId = crypto.randomUUID();
    dispatch({
      type: "run-started",
      requestId,
      runId,
      digest: state.document.digest,
      configuration,
    });
    const response = await studioBridge.run(
      {
        protocol: BRIDGE_PROTOCOL,
        digest: state.document.digest,
        endTime: configuration.endTime,
        maxStep: configuration.maxStep,
        runId,
        planKey: state.runPlanStatus.plan.key,
      },
      (progress) => dispatch({ type: "run-progressed", requestId, progress }),
    );
    dispatch({
      type: "run-finished",
      requestId,
      outcome: response.result,
      diagnostics: response.diagnostics,
    });
  }, [runPlanCurrent, runValidation.value, sourceEdited, state.document, state.runPlanStatus]);

  const runSpatial = useCallback(async () => {
    const configuration = spatialValidation?.value ?? null;
    if (
      state.document === null ||
      spatialWorkflow === null ||
      sourceEdited ||
      configuration === null ||
      spatialState.planStatus.kind !== "ready" ||
      !spatialPlanCurrent ||
      state.runStatus.kind === "running" ||
      state.runStatus.kind === "cancelling"
    ) {
      return;
    }
    const requestId = ++spatialRunSequence.current;
    const runId = crypto.randomUUID();
    const digest = state.document.digest;
    const realizationRevision = spatialState.realizationRevision;
    spatialDispatch({
      type: "run-started",
      requestId,
      runId,
      digest,
      realizationRevision,
      configuration,
    });
    const response = await studioBridge.runSpatialRealization({
      protocol: BRIDGE_PROTOCOL,
      digest,
      realizationRevision,
      method: configuration.method,
      cellsPerAxis: configuration.cellsPerAxis,
      workers: configuration.workers,
      runId,
      planKey: spatialState.planStatus.plan.key,
    });
    spatialDispatch({
      type: "run-finished",
      requestId,
      result: response.result,
      diagnostics: response.diagnostics,
    });
  }, [
    sourceEdited,
    spatialPlanCurrent,
    spatialState.planStatus,
    spatialState.realizationRevision,
    spatialValidation?.value,
    spatialWorkflow,
    state.document,
    state.runStatus.kind,
  ]);

  const cancelRun = useCallback(async () => {
    if (state.runStatus.kind !== "running") return;
    const { requestId, runId } = state.runStatus;
    dispatch({ type: "run-cancel-requested", requestId });
    const response = await studioBridge.cancelRun({ protocol: BRIDGE_PROTOCOL, runId });
    if (response.result === null) {
      dispatch({
        type: "run-cancel-failed",
        requestId,
        diagnostics: response.diagnostics,
      });
    }
  }, [state.runStatus]);

  const commitValueEdit = useCallback(async () => {
    if (
      state.document === null ||
      selectedNode === null ||
      valueEditValidation.value === null ||
      state.valueEditStatus.kind !== "ready" ||
      valueEditDisabledReason !== null ||
      state.valueEditStatus.digest !== state.document.digest ||
      state.valueEditStatus.targetId !== selectedNode.id ||
      state.valueEditStatus.input !== state.valueEditInput
    ) {
      return;
    }
    const plan = state.valueEditStatus.plan;
    const requestId = ++valueEditCommitSequence.current;
    dispatch({ type: "value-edit-commit-started", requestId, plan });
    const response = await studioBridge.commitValueEdit({
      protocol: BRIDGE_PROTOCOL,
      digest: state.document.digest,
      targetId: selectedNode.id,
      value: valueEditValidation.value,
      planKey: plan.key,
    });
    dispatch({
      type: "value-edit-commit-finished",
      requestId,
      result: response.result,
      diagnostics: response.diagnostics,
    });
  }, [
    selectedNode,
    state.document,
    state.valueEditInput,
    state.valueEditStatus,
    valueEditDisabledReason,
    valueEditValidation.value,
  ]);

  const commandAvailability = useMemo<CommandAvailability>(() => {
    const runActivity: RunActivity =
      spatialState.runStatus.kind === "running"
        ? "spatial-running"
        : state.runStatus.kind === "running"
          ? "reference-running"
          : state.runStatus.kind === "cancelling"
            ? "reference-cancelling"
            : "idle";
    const runBlock: RunBlock =
      runActivity !== "idle"
        ? "active-run"
        : state.valueEditStatus.kind === "committing"
          ? "committing"
          : state.document === null
            ? "no-document"
            : sourceEdited
              ? "source-edited"
              : application.activeWorkflow === "scalar-elliptic"
                ? spatialValidation?.value === null || spatialValidation?.value === undefined
                  ? "invalid-spatial"
                  : !spatialPlanCurrent
                    ? "spatial-plan"
                    : null
                : runValidation.value === null
                  ? "invalid-time"
                  : !runPlanCurrent
                    ? "time-plan"
                    : null;
    const cadAvailability = application.workflows.find(
      (workflow) => workflow.definition.id === "cad-box",
    )?.availability;
    if (cadAvailability === undefined) {
      throw new Error("Closed Studio registry omitted the CAD workflow");
    }
    const resolved = resolveCommandAvailability({
      activeWorkflow: application.activeWorkflow,
      compiling: state.compileStatus.kind === "compiling",
      documentAccepted: state.document !== null,
      valueEditReady: state.valueEditStatus.kind === "ready",
      valueEditBlock,
      revisionNavigationBlocked,
      canUndo,
      canRedo,
      runBlock,
      runActivity,
      selectedEntity:
        application.activeWorkflow === "cad-authored"
          ? false
          : application.activeWorkflow === "cad-box"
            ? cadSelectionState.accepted !== null
            : application.activeWorkflow === "cylinder-stokes"
              ? cylinderState.kind === "ready"
              : application.activeWorkflow === "packaged-dc-drive"
                ? dcMotorState.kind === "ready"
                : application.activeWorkflow === "structural-elasticity"
                  ? structuralState.kind === "ready"
                  : application.activeWorkflow === "fixed-reference-fsi"
                    ? fsiState.kind === "ready"
                    : activeWorkspace === "field"
                      ? scalarFieldState.kind === "ready"
                      : selectedNode !== null,
      evidenceAvailable:
        application.activeWorkflow === "scalar-elliptic"
          ? spatialResult !== null
          : application.activeWorkflow === "cylinder-stokes"
            ? cylinderState.kind === "ready"
            : application.activeWorkflow === "packaged-dc-drive"
              ? dcMotorState.kind === "ready"
              : application.activeWorkflow === "structural-elasticity"
                ? structuralState.kind === "ready"
                : application.activeWorkflow === "fixed-reference-fsi"
                  ? fsiState.kind === "ready"
                  : application.activeWorkflow === "relations" && runResult !== null,
      fieldAvailable:
        fieldWorkflow === "cylinder-stokes"
          ? cylinderState.kind === "solving" ||
            cylinderState.kind === "loading-field" ||
            cylinderState.kind === "ready"
          : spatialResult?.plan.requirements.spatialDimension === 2,
      cylinderRunning: cylinderState.kind === "solving" || cylinderState.kind === "loading-field",
      trajectoryAvailable: dcMotorState.kind === "running" || dcMotorState.kind === "ready",
      dcMotorRunning: dcMotorState.kind === "running",
      structuralAvailable: structuralState.kind === "running" || structuralState.kind === "ready",
      structuralRunning: structuralState.kind === "running",
      fsiAvailable: fsiState.kind === "running" || fsiState.kind === "ready",
      fsiRunning: fsiState.kind === "running",
      cadAvailability,
    });
    return Object.fromEntries(
      Object.entries(resolved).map(([command, availability]) => [
        command,
        {
          enabled: availability.enabled,
          reason: availability.reason === null ? null : formatMessage(availability.reason),
        },
      ]),
    ) as CommandAvailability;
  }, [
    application.activeWorkflow,
    application.workflows,
    activeWorkspace,
    cadSelectionState.accepted,
    canRedo,
    canUndo,
    revisionNavigationBlocked,
    cylinderState,
    dcMotorState,
    structuralState,
    fsiState,
    fieldWorkflow,
    runPlanCurrent,
    runResult,
    runValidation.value,
    selectedNode,
    sourceEdited,
    spatialPlanCurrent,
    spatialResult,
    spatialState.runStatus.kind,
    scalarFieldState.kind,
    spatialValidation?.value,
    state.compileStatus.kind,
    state.document,
    state.runStatus.kind,
    state.valueEditStatus.kind,
    valueEditBlock,
  ]);

  const focusTarget = useCallback(
    (target: FocusTarget) => {
      switch (target) {
        case "source-editor":
          sourceEditor.current?.focus();
          return;
        case "relation-view":
          relationView.current?.focus();
          return;
        default:
          document
            .getElementById(
              resolveElementFocusId(target, application.activeWorkflow, activeWorkspace),
            )
            ?.focus();
          return;
      }
    },
    [activeWorkspace, application.activeWorkflow],
  );

  const focusCommandTarget = useCallback(
    (command: CommandId) => {
      const target = COMMAND_REGISTRY.find((candidate) => candidate.id === command)?.focusTarget;
      if (target !== null && target !== undefined) {
        window.setTimeout(() => {
          window.requestAnimationFrame(() => focusTarget(target));
        }, 0);
      }
    },
    [focusTarget],
  );

  const openCylinderDemo = useCallback(async () => {
    setSelectedCylinderVertex(0);
    setSelectedFieldWorkflow("cylinder-stokes");
    setRequestedWorkspace("field");
    const terminal = await cylinderSession.run();
    if (terminal.kind === "ready") {
      window.requestAnimationFrame(() => {
        document
          .getElementById(resolveElementFocusId("field-viewport", "cylinder-stokes", "field"))
          ?.focus();
      });
    }
  }, [cylinderSession]);

  const openDcMotorDemo = useCallback(async () => {
    setRequestedWorkspace("trajectory");
    const terminal = await dcMotorSession.run();
    if (terminal.kind === "ready") {
      window.requestAnimationFrame(() => focusTarget("trajectory-viewport"));
    }
  }, [dcMotorSession, focusTarget]);

  const openStructuralDemo = useCallback(async () => {
    setRequestedWorkspace("structure");
    const terminal = await structuralSession.run();
    if (terminal.kind === "ready") {
      window.requestAnimationFrame(() => focusTarget("structural-viewport"));
    }
  }, [focusTarget, structuralSession]);

  const openFsiDemo = useCallback(async () => {
    setRequestedWorkspace("fsi");
    const terminal = await fsiSession.run();
    if (terminal.kind === "ready") {
      window.requestAnimationFrame(() => focusTarget("fsi-viewport"));
    }
  }, [focusTarget, fsiSession]);

  const openScalarField = useCallback(async () => {
    if (spatialResult === null || spatialResult.plan.requirements.spatialDimension !== 2) {
      return;
    }
    setSelectedFieldWorkflow("scalar-elliptic");
    setRequestedWorkspace("field");
    if (
      scalarFieldState.kind === "ready" &&
      scalarFieldState.context.digest === spatialResult.digest &&
      scalarFieldState.context.runId === spatialResult.runId &&
      scalarFieldState.context.plan.key === spatialResult.plan.key
    ) {
      window.requestAnimationFrame(() => focusTarget("field-viewport"));
      return;
    }
    setSelectedFieldOrdinal(0);
    const terminal = await scalarFieldSession.load(spatialResult);
    if (terminal.kind === "ready") {
      window.requestAnimationFrame(() => focusTarget("field-viewport"));
    }
  }, [focusTarget, scalarFieldSession, scalarFieldState, spatialResult]);

  const executeCommand = useCallback(
    (command: CommandId) => {
      if (!commandAvailability[command].enabled) return;
      switch (command) {
        case "model.compile":
          void compile();
          return;
        case "edit.commit":
          void commitValueEdit();
          return;
        case "history.undo":
          dispatch({ type: "revision-undo" });
          return;
        case "history.redo":
          dispatch({ type: "revision-redo" });
          return;
        case "run.execute":
          if (isSpatialDocument) {
            void runSpatial();
          } else {
            void run();
          }
          return;
        case "run.cancel":
          void cancelRun();
          return;
        case "view.reflow":
          dispatch({ type: "layout-reset" });
          return;
        case "workspace.relations":
          setRequestedWorkspace("relations");
          focusCommandTarget(command);
          return;
        case "workspace.geometry":
          setRequestedWorkspace("geometry");
          focusCommandTarget(command);
          return;
        case "workspace.cad-authoring":
          setRequestedWorkspace("cad-authoring");
          focusCommandTarget(command);
          return;
        case "workspace.field":
          if (fieldWorkflow === "cylinder-stokes") {
            setRequestedWorkspace("field");
            focusCommandTarget(command);
          } else {
            void openScalarField();
          }
          return;
        case "workspace.trajectory":
          if (dcMotorState.kind === "idle" || dcMotorState.kind === "failed") {
            void openDcMotorDemo();
          } else {
            setRequestedWorkspace("trajectory");
            focusCommandTarget(command);
          }
          return;
        case "workspace.structure":
          if (structuralState.kind === "idle" || structuralState.kind === "failed") {
            void openStructuralDemo();
          } else {
            setRequestedWorkspace("structure");
            focusCommandTarget(command);
          }
          return;
        case "workspace.fsi":
          if (fsiState.kind === "idle" || fsiState.kind === "failed") {
            void openFsiDemo();
          } else {
            setRequestedWorkspace("fsi");
            focusCommandTarget(command);
          }
          return;
        case "example.cylinder":
          void openCylinderDemo();
          return;
        case "example.dc-drive":
          void openDcMotorDemo();
          return;
        case "example.structural":
          void openStructuralDemo();
          return;
        case "example.fsi":
          void openFsiDemo();
          return;
        case "example.spatial":
          void openSpatialExample();
          return;
        case "example.cad":
          void openCadExample();
          return;
        case "focus.source":
        case "focus.relation":
        case "focus.inspector":
        case "focus.evidence":
          focusCommandTarget(command);
          return;
      }
    },
    [
      cancelRun,
      commandAvailability,
      commitValueEdit,
      compile,
      focusCommandTarget,
      fieldWorkflow,
      dcMotorState.kind,
      fsiState.kind,
      isSpatialDocument,
      openCadExample,
      openCylinderDemo,
      openDcMotorDemo,
      openFsiDemo,
      openStructuralDemo,
      openScalarField,
      openSpatialExample,
      run,
      runSpatial,
      structuralState.kind,
    ],
  );

  useEffect(() => {
    const openCommands = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        dispatch({ type: "command-palette-opened" });
        return;
      }
      const target = event.target;
      const editing =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable);
      if (!editing && (event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "z") {
        event.preventDefault();
        executeCommand(event.shiftKey ? "history.redo" : "history.undo");
      }
    };
    window.addEventListener("keydown", openCommands);
    return () => window.removeEventListener("keydown", openCommands);
  }, [executeCommand]);

  return (
    <>
      <CommandPalette
        availability={commandAvailability}
        onClose={() => dispatch({ type: "command-palette-closed" })}
        onExecute={executeCommand}
        onQuery={(query) => dispatch({ type: "command-query-edited", query })}
        open={state.commandPalette.kind === "open"}
        query={state.commandPalette.kind === "open" ? state.commandPalette.query : ""}
      />
      <div className="studio-shell">
        <header className="app-bar">
          <div className="brand">
            <img alt="" className="brand-mark" height={28} src={iconUrl} width={28} />
            <span>
              <strong>Eqiora</strong>
              <small>Studio</small>
            </span>
          </div>
          <div className="app-bar__context">
            <span className="document-name">
              {application.activeWorkflow === "cylinder-stokes"
                ? "steady-flow-past-cylinder"
                : application.activeWorkflow === "packaged-dc-drive"
                  ? "org.example.dc_motor_control@0.1.0"
                  : application.activeWorkflow === "structural-elasticity"
                    ? "mixed-boundary-elasticity.eqi"
                    : "untitled.eqi"}
            </span>
            <span aria-hidden="true">/</span>
            <span>
              {state.document === null ? "No revision" : `Revision ${state.document.revision}`}
            </span>
            <fieldset aria-label="Revision lineage" className="history-actions">
              <button
                aria-label="Previous revision"
                disabled={!commandAvailability["history.undo"].enabled}
                onClick={() => executeCommand("history.undo")}
                title={
                  commandAvailability["history.undo"].reason ??
                  "Previous revision (Ctrl/⌘ Z outside an editor)"
                }
                type="button"
              >
                ←
              </button>
              <span>
                {state.revisionIndex < 0 ? 0 : state.revisionIndex + 1}/
                {state.revisionLineage.length}
              </span>
              <button
                aria-label="Next revision"
                disabled={!commandAvailability["history.redo"].enabled}
                onClick={() => executeCommand("history.redo")}
                title={
                  commandAvailability["history.redo"].reason ??
                  "Next revision (Ctrl/⌘ Shift Z outside an editor)"
                }
                type="button"
              >
                →
              </button>
            </fieldset>
            <nav aria-label="Workspace view" className="workspace-switcher">
              <button
                aria-current={activeWorkspace === "relations" ? "page" : undefined}
                disabled={!commandAvailability["workspace.relations"].enabled}
                onClick={() => executeCommand("workspace.relations")}
                type="button"
              >
                {formatMessage("workflow.relations.label")}
              </button>
              {commandAvailability["workspace.field"].enabled ? (
                <button
                  aria-current={activeWorkspace === "field" ? "page" : undefined}
                  onClick={() => executeCommand("workspace.field")}
                  type="button"
                >
                  Field
                </button>
              ) : null}
              {commandAvailability["workspace.trajectory"].enabled ? (
                <button
                  aria-current={activeWorkspace === "trajectory" ? "page" : undefined}
                  onClick={() => executeCommand("workspace.trajectory")}
                  type="button"
                >
                  Trajectory
                </button>
              ) : null}
              {commandAvailability["workspace.structure"].enabled ? (
                <button
                  aria-current={activeWorkspace === "structure" ? "page" : undefined}
                  onClick={() => executeCommand("workspace.structure")}
                  type="button"
                >
                  Structure
                </button>
              ) : null}
              {commandAvailability["workspace.fsi"].enabled ? (
                <button
                  aria-current={activeWorkspace === "fsi" ? "page" : undefined}
                  onClick={() => executeCommand("workspace.fsi")}
                  type="button"
                >
                  FSI
                </button>
              ) : null}
              {commandAvailability["workspace.geometry"].enabled ? (
                <button
                  aria-current={activeWorkspace === "geometry" ? "page" : undefined}
                  onClick={() => executeCommand("workspace.geometry")}
                  type="button"
                >
                  {formatMessage("workflow.cad.label")}
                </button>
              ) : null}
              <button
                aria-current={activeWorkspace === "cad-authoring" ? "page" : undefined}
                disabled={!commandAvailability["workspace.cad-authoring"].enabled}
                onClick={() => executeCommand("workspace.cad-authoring")}
                type="button"
              >
                {formatMessage("workflow.cad-authored.label")}
              </button>
            </nav>
          </div>
          <div className="app-bar__actions">
            <ExampleMenu
              availability={commandAvailability}
              cylinderRunning={
                cylinderState.kind === "solving" || cylinderState.kind === "loading-field"
              }
              dcMotorStatus={dcMotorState.kind}
              fsiStatus={fsiState.kind}
              onExecute={executeCommand}
              structuralStatus={structuralState.kind}
            />
            <span className={`runtime-badge runtime-badge--${studioBridge.mode}`}>
              <span aria-hidden="true" />
              {studioBridge.mode === "native" ? "Canonical runtime" : "Browser preview"}
            </span>
            <button
              className="secondary-action command-trigger"
              onClick={() => dispatch({ type: "command-palette-opened" })}
              type="button"
            >
              <Icon name="command" />
              Commands <kbd>Ctrl/⌘ K</kbd>
            </button>
            <button
              className="secondary-action"
              disabled={!commandAvailability["model.compile"].enabled}
              onClick={() => executeCommand("model.compile")}
              title={commandAvailability["model.compile"].reason ?? undefined}
              type="button"
            >
              <Icon name="compile" />
              {state.compileStatus.kind === "compiling" ? "Compiling…" : "Compile model"}
            </button>
          </div>
        </header>

        {studioBridge.mode === "preview" ? (
          <div className="preview-banner" role="status">
            The browser preview demonstrates interaction and layout only. Tauri performs canonical
            compilation and execution through the Rust facade.
          </div>
        ) : null}
        {cylinderState.kind === "failed" ? (
          <DemoFailureBanner
            message={cylinderState.message}
            onRetry={() => void openCylinderDemo()}
          />
        ) : null}
        {dcMotorState.kind === "failed" ? (
          <DemoFailureBanner
            message={dcMotorState.message}
            onRetry={() => void openDcMotorDemo()}
          />
        ) : null}
        {structuralState.kind === "failed" ? (
          <DemoFailureBanner
            message={structuralState.message}
            onRetry={() => void openStructuralDemo()}
          />
        ) : null}
        {fsiState.kind === "failed" ? (
          <DemoFailureBanner message={fsiState.message} onRetry={() => void openFsiDemo()} />
        ) : null}

        <main
          className="cad-authored-workspace-shell"
          hidden={activeWorkspace !== "cad-authoring"}
          id={activeWorkspace === "cad-authoring" ? "workspace" : undefined}
          tabIndex={-1}
        >
          <CadAuthoredWorkspace />
        </main>

        <main
          className="trajectory-workspace-shell"
          hidden={activeWorkspace !== "trajectory"}
          id={activeWorkspace === "trajectory" ? "workspace" : undefined}
          tabIndex={-1}
        >
          {dcMotorState.kind === "ready" ? (
            <DcMotorDemoWorkspace result={dcMotorState.result} />
          ) : (
            <DemoLoadState
              detail="The native runtime is compiling the exact package closure, executing 100 accepted steps, and binding Model and Run identities."
              glyph="⌁"
              title="Running packaged DC drive…"
            />
          )}
        </main>

        <main
          className="structural-workspace-shell"
          hidden={activeWorkspace !== "structure"}
          id={activeWorkspace === "structure" ? "workspace" : undefined}
          tabIndex={-1}
        >
          {structuralState.kind === "ready" ? (
            <StructuralDemoWorkspace result={structuralState.result} />
          ) : (
            <DemoLoadState
              detail="The native runtime is compiling the exact structural Model, resolving the frozen Q1 plan, and accepting the solver-owned displacement and lineage."
              glyph="⌗"
              title="Solving the elastic panel…"
            />
          )}
        </main>

        <main
          className="fsi-workspace-shell"
          hidden={activeWorkspace !== "fsi"}
          id={activeWorkspace === "fsi" ? "workspace" : undefined}
          tabIndex={-1}
        >
          {fsiState.kind === "ready" ? (
            <FsiDemoWorkspace result={fsiState.result} />
          ) : (
            <DemoLoadState
              detail="The native runtime is compiling one exact coupled Model, accepting two consecutive reference steps, and publishing one immutable in-memory trajectory."
              glyph="⇄"
              title="Solving the coupled interface…"
            />
          )}
        </main>

        <main
          className="geometry-workspace-shell"
          hidden={activeWorkspace !== "geometry"}
          id={activeWorkspace === "geometry" ? "workspace" : undefined}
          tabIndex={-1}
        >
          {cadProjection === null ? (
            <section className="geometry-workspace-empty" aria-live="polite">
              <span aria-hidden="true">◇</span>
              <h1>{cadStatus === "loading" ? "Replaying CAD evidence…" : "No CAD plan"}</h1>
              <p>
                {cadStatus === "loading"
                  ? "The native adapter is rebuilding the exact Model, design, geometry, and mesh chain."
                  : "Open the bounded CAD example or return to Relations to edit the source model."}
              </p>
              {cadStatus === "loading" ? null : (
                <button
                  className="primary-action"
                  onClick={() => executeCommand("example.cad")}
                  type="button"
                >
                  Open CAD example
                </button>
              )}
            </section>
          ) : (
            <>
              {cadError === null ? null : (
                <div className="cad-error" role="alert">
                  {cadError}
                </div>
              )}
              <CadWorkspace
                onRequestSelection={(request) => void requestCadSelection(request)}
                projection={cadProjection}
                selection={cadSelectionState.accepted}
                selectionPending={cadSelectionState.status.kind === "resolving"}
              />
            </>
          )}
        </main>
        <main
          className="scalar-field-shell"
          hidden={activeWorkspace !== "field"}
          id={activeWorkspace === "field" ? "workspace" : undefined}
          tabIndex={-1}
        >
          {fieldWorkflow === "cylinder-stokes" ? (
            cylinderState.kind === "ready" ? (
              <CylinderDemoWorkspace
                field={cylinderState.field}
                onSelect={setSelectedCylinderVertex}
                result={cylinderState.result}
                selectedVertex={selectedCylinderVertex}
              />
            ) : (
              <section className="scalar-field-load-state" aria-live="polite" role="status">
                <span aria-hidden="true">◯</span>
                <h1>
                  {cylinderState.kind === "loading-field"
                    ? "Binding accepted pressure field…"
                    : "Solving exact-cylinder Stokes system…"}
                </h1>
                <p>
                  {cylinderState.kind === "loading-field"
                    ? "The accepted Model, Realization, Run, snapshot, mesh, and pressure field identities are being checked before transfer."
                    : "The native runtime is replaying exact geometry, realizing its bounded chordal mesh, and applying the frozen solver plan."}
                </p>
              </section>
            )
          ) : scalarFieldState.kind === "ready" ? (
            <ScalarFieldWorkspace
              descriptor={scalarFieldState.descriptor}
              onSelect={setSelectedFieldOrdinal}
              realizationRevision={scalarFieldState.context.plan.realizationRevision}
              selectedOrdinal={selectedFieldOrdinal}
              stale={spatialResultStale}
              values={scalarFieldState.values}
            />
          ) : (
            <section
              className={
                scalarFieldState.kind === "failed"
                  ? "scalar-field-load-state scalar-field-load-state--failed"
                  : "scalar-field-load-state"
              }
              aria-live="polite"
              role={scalarFieldState.kind === "failed" ? "alert" : "status"}
            >
              <span aria-hidden="true">▦</span>
              <h1>
                {scalarFieldState.kind === "failed"
                  ? "Field view unavailable"
                  : scalarFieldState.kind === "streaming"
                    ? "Reading accepted field…"
                    : "Opening accepted field…"}
              </h1>
              <p>
                {scalarFieldState.kind === "failed"
                  ? (scalarFieldState.failure.cause?.message ?? scalarFieldState.failure.message)
                  : scalarFieldState.kind === "streaming"
                    ? `${scalarFieldState.receivedValueCount.toLocaleString()} of ${scalarFieldState.descriptor.field.valueCount.toLocaleString()} values received in canonical order.`
                    : "Studio is resolving the exact Model, run, and Realization identity before transferring values."}
              </p>
              {scalarFieldState.kind === "failed" ? (
                <div className="scalar-field-load-actions">
                  <button
                    className="primary-action"
                    onClick={() => void openScalarField()}
                    type="button"
                  >
                    Retry exact field
                  </button>
                  <button
                    className="secondary-action"
                    onClick={() => executeCommand("workspace.relations")}
                    type="button"
                  >
                    Return to relations
                  </button>
                </div>
              ) : null}
            </section>
          )}
        </main>
        <main
          className="workspace"
          hidden={activeWorkspace !== "relations"}
          id={activeWorkspace === "relations" ? "workspace" : undefined}
          tabIndex={-1}
        >
          <aside className="workspace__left">
            <SourceEditor
              ref={sourceEditor}
              onChange={(source) => dispatch({ type: "source-edited", source })}
              onCompile={() => executeCommand("model.compile")}
              source={state.source}
              ancestor={sourceAncestor}
              edited={sourceEdited}
            />
            <ModelOutline
              document={state.document}
              onSelect={(nodeId) => dispatch({ type: "node-selected", nodeId })}
              selectedNodeId={state.selectedNodeId}
            />
          </aside>

          <section
            ref={relationView}
            className="canvas-panel"
            aria-labelledby="relation-view-heading"
            tabIndex={-1}
          >
            <div className="canvas-panel__heading">
              <div>
                <span className="eyebrow">Semantic projection</span>
                <h1 id="relation-view-heading">Relation view</h1>
              </div>
              <div>
                <span>{state.document?.nodes.length ?? 0} entities</span>
                <span>{state.document?.edges.length ?? 0} relations</span>
                <button
                  className="icon-action"
                  disabled={!commandAvailability["view.reflow"].enabled}
                  onClick={() => executeCommand("view.reflow")}
                  title={commandAvailability["view.reflow"].reason ?? undefined}
                  type="button"
                >
                  <Icon name="reset" />
                  Reflow view
                </button>
              </div>
            </div>
            <div className="canvas-stage">
              {state.document === null ? (
                <div className="canvas-empty">
                  <span className="canvas-empty__glyph" aria-hidden="true">
                    <Icon name="node" />
                  </span>
                  <h2>A validated model appears here</h2>
                  <p>Compile source to create a projection of its canonical relations.</p>
                </div>
              ) : (
                <ModelCanvas
                  document={state.document}
                  layout={layout}
                  onMove={(nodeId, position) => dispatch({ type: "node-moved", nodeId, position })}
                  onSelect={(nodeId) => dispatch({ type: "node-selected", nodeId })}
                  selectedNodeId={state.selectedNodeId}
                />
              )}
            </div>
            <fieldset className="canvas-legend">
              <legend className="sr-only">View legend</legend>
              <span>
                <i className="entity-dot entity-dot--field" /> state
              </span>
              <span>
                <i className="entity-dot entity-dot--relation" /> relation
              </span>
              <span>
                <i className="entity-dot entity-dot--activation" /> activation
              </span>
              <span>Layout is non-semantic</span>
            </fieldset>
          </section>

          <aside
            className={
              valueEditExpanded
                ? "workspace__right workspace__right--value-edit"
                : "workspace__right"
            }
          >
            <Inspector
              node={selectedNode}
              onNudge={(delta) => {
                if (selectedNode === null) return;
                const position = layout[selectedNode.id] ?? { x: 0, y: 0 };
                dispatch({
                  type: "node-moved",
                  nodeId: selectedNode.id,
                  position: { x: position.x + delta.x, y: position.y + delta.y },
                });
              }}
              position={selectedNode === null ? null : (layout[selectedNode.id] ?? null)}
              valueEdit={{
                input: state.valueEditInput,
                validation: valueEditValidation,
                status: state.valueEditStatus,
                disabledReason: valueEditDisabledReason,
                onChange: (value) => dispatch({ type: "value-edit-input-edited", value }),
                onCommit: () => executeCommand("edit.commit"),
              }}
            />
            {spatialWorkflow === null || spatialValidation === null ? (
              <RunPanel
                configuration={state.runConfiguration}
                digest={state.document?.digest ?? null}
                onCancel={() => executeCommand("run.cancel")}
                onEdit={(field, value) => dispatch({ type: "run-input-edited", field, value })}
                onRun={() => executeCommand("run.execute")}
                planCurrent={runPlanCurrent}
                planStatus={state.runPlanStatus}
                stale={sourceEdited}
                status={state.runStatus}
                validation={runValidation}
              />
            ) : (
              <SpatialRunPanel
                blocked={
                  state.runStatus.kind === "running" ||
                  state.runStatus.kind === "cancelling" ||
                  state.valueEditStatus.kind === "committing"
                }
                configuration={spatialState.configuration}
                onMethodEdit={(value) =>
                  spatialDispatch({ type: "input-edited", field: "method", value })
                }
                onNumericEdit={(field, value) =>
                  spatialDispatch({ type: "input-edited", field, value })
                }
                onRun={() => executeCommand("run.execute")}
                planCurrent={spatialPlanCurrent}
                planStatus={spatialState.planStatus}
                realizationRevision={spatialState.realizationRevision}
                runStatus={spatialState.runStatus}
                stale={sourceEdited}
                validation={spatialValidation}
                workflow={spatialWorkflow}
              />
            )}
          </aside>

          <section className="workspace__bottom">
            <Diagnostics
              diagnostics={diagnostics}
              onNavigate={(diagnostic) => {
                if (diagnostic.span !== null) {
                  revealSourceSpan(state.source, diagnostic.span);
                }
              }}
            />
            {isSpatialDocument ? (
              <SpatialResults
                result={spatialResult}
                onViewField={
                  spatialResult?.plan.requirements.spatialDimension === 2
                    ? () => void openScalarField()
                    : null
                }
                stale={spatialResultStale}
                staleReason={
                  sourceEdited
                    ? "Source has uncompiled changes."
                    : spatialResult?.digest !== state.document?.digest
                      ? "The canonical revision has changed."
                      : "The Realization intent has changed."
                }
              />
            ) : (
              <Results
                configurationStale={runConfigurationStale}
                revisionStale={runRevisionStale}
                result={runResult}
                sourceStale={sourceEdited}
              />
            )}
          </section>
        </main>
      </div>
    </>
  );
}
