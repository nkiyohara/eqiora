import { render as renderTrajectory } from "./trajectory-view";
import type { TrajectoryModel } from "./trajectory-contract";

interface RenderContext {
	model: TrajectoryModel;
	el: HTMLElement;
}

function render(context: RenderContext): () => void {
	if (context.model.get("profile") === "fixed-mesh-scalar-trajectory-2d/v1") {
		return renderTrajectory(context);
	}
	context.el.replaceChildren();
	return () => undefined;
}

export default { render };
