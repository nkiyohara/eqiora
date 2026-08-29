import type { Render } from "@anywidget/types";
import { mountEqioraViewer, showViewerFailure } from "./viewer";

const render: Render = ({ model, el }) => {
	try {
		const metadata = model.get("scene_metadata");
		const buffers = model.get("buffers");
		if (typeof metadata !== "string" || !Array.isArray(buffers)) {
			throw new Error(
				"Notebook host omitted the private scene metadata or binary buffers",
			);
		}
		const mounted = mountEqioraViewer(el, metadata, buffers);
		const cleanup = () => mounted.cleanup();
		model.on("destroy", cleanup);
		model.on("comm:close", cleanup);
		return () => {
			model.off("destroy", cleanup);
			model.off("comm:close", cleanup);
			cleanup();
		};
	} catch (error) {
		showViewerFailure(el, error);
		return () => el.replaceChildren();
	}
};

export default { render };
export { decodeScene } from "./contract";
export { mountEqioraViewer } from "./viewer";
