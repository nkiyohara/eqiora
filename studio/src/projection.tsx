import {
  type AriaLabelConfig,
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  Handle,
  MarkerType,
  MiniMap,
  type Node,
  type NodeProps,
  Position,
  ReactFlow,
} from "@xyflow/react";
import { memo, useMemo } from "react";
import type { DocumentProjection, ProjectionNode, ProjectionNodeKind } from "./protocol";
import type { NodeLayout, Point } from "./state";

type SemanticNodeData = ProjectionNode & Record<string, unknown>;
type SemanticFlowNode = Node<SemanticNodeData, "semantic">;

const KIND_LABELS: Record<ProjectionNodeKind, string> = {
  domain: "Domain",
  representation: "Representation",
  field: "Field",
  parameter: "Parameter",
  port: "Port",
  relation: "Relation",
  activation: "Activation",
  connection: "Connection",
  "clock-domain": "Clock domain",
};

const NODE_COLORS: Record<ProjectionNodeKind, string> = {
  domain: "#8fbaa6",
  representation: "#75a695",
  field: "#e9c979",
  parameter: "#dba368",
  port: "#87b8c8",
  relation: "#ef866f",
  activation: "#c3a4d9",
  connection: "#7fabc6",
  "clock-domain": "#a29bd2",
};

const FLOW_ARIA_LABELS: Partial<AriaLabelConfig> = {
  "node.a11yDescription.default":
    "Press enter or space to select this entity. Use the arrow keys to move its non-semantic view position.",
  "edge.a11yDescription.default":
    "Press enter or space to select this canonical relation. Editing is unavailable in this projection.",
};

const SemanticNode = memo(function SemanticNode({ data, selected }: NodeProps<SemanticFlowNode>) {
  return (
    <article
      className={`model-node model-node--${data.kind}${selected ? " is-selected" : ""}`}
      aria-label={`${KIND_LABELS[data.kind]} ${data.name}`}
    >
      <Handle
        className="model-node__handle"
        isConnectable={false}
        position={Position.Left}
        type="target"
      />
      <div className="model-node__kind">
        <span aria-hidden="true" className="model-node__mark" />
        {KIND_LABELS[data.kind]}
      </div>
      <strong>{data.name}</strong>
      <p>{data.summary}</p>
      {data.dimension !== null ? (
        <span className="model-node__unit">[{data.dimension}]</span>
      ) : null}
      <Handle
        className="model-node__handle"
        isConnectable={false}
        position={Position.Right}
        type="source"
      />
    </article>
  );
});

const nodeTypes = { semantic: SemanticNode };

function flowNodes(
  document: DocumentProjection,
  layout: NodeLayout,
  selectedNodeId: string | null,
): SemanticFlowNode[] {
  return document.nodes.map((node) => ({
    id: node.id,
    type: "semantic",
    position: layout[node.id] ?? { x: 0, y: 0 },
    data: node,
    selected: node.id === selectedNodeId,
    ariaLabel: `${KIND_LABELS[node.kind]} ${node.name}`,
  }));
}

function flowEdges(document: DocumentProjection): Edge[] {
  const nodeNames = new Map(document.nodes.map((node) => [node.id, node.name]));
  return document.edges.map((edge) => ({
    id: edge.id,
    source: edge.source,
    target: edge.target,
    label: edge.label,
    ariaLabel: `${edge.label}: ${nodeNames.get(edge.source) ?? edge.source} to ${nodeNames.get(edge.target) ?? edge.target}`,
    className: `model-edge model-edge--${edge.kind}`,
    markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
  }));
}

interface ModelCanvasProps {
  readonly document: DocumentProjection;
  readonly layout: NodeLayout;
  readonly selectedNodeId: string | null;
  readonly onSelect: (nodeId: string | null) => void;
  readonly onMove: (nodeId: string, position: Point) => void;
}

export function ModelCanvas({
  document,
  layout,
  selectedNodeId,
  onSelect,
  onMove,
}: ModelCanvasProps) {
  const nodes = useMemo(
    () => flowNodes(document, layout, selectedNodeId),
    [document, layout, selectedNodeId],
  );
  const edges = useMemo(() => flowEdges(document), [document]);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      fitView
      fitViewOptions={{ padding: 0.22, minZoom: 0.6, maxZoom: 1.15 }}
      minZoom={0.25}
      maxZoom={1.75}
      ariaLabelConfig={FLOW_ARIA_LABELS}
      autoPanOnNodeFocus
      connectOnClick={false}
      deleteKeyCode={null}
      edgesReconnectable={false}
      nodesFocusable
      edgesFocusable
      elementsSelectable
      nodesDraggable
      nodesConnectable={false}
      selectNodesOnDrag={false}
      onPaneClick={() => onSelect(null)}
      onNodeClick={(_, node) => onSelect(node.id)}
      onNodesChange={(changes) => {
        for (const change of changes) {
          if (change.type === "select" && change.selected) {
            onSelect(change.id);
          }
          if (change.type === "position" && change.position) {
            onMove(change.id, change.position);
          }
        }
      }}
      aria-label="Canonical model relation view"
      colorMode="dark"
    >
      <Background color="rgba(188, 208, 194, 0.13)" gap={24} variant={BackgroundVariant.Dots} />
      <MiniMap
        pannable
        zoomable
        ariaLabel="Model overview map"
        style={{ width: 142, height: 92 }}
        nodeColor={(node) => NODE_COLORS[(node.data as SemanticNodeData).kind]}
        maskColor="rgba(11, 18, 14, 0.72)"
      />
      <Controls showInteractive={false} aria-label="Canvas zoom controls" />
      {selectedNodeId === null ? null : <span className="sr-only">Selected {selectedNodeId}</span>}
    </ReactFlow>
  );
}
