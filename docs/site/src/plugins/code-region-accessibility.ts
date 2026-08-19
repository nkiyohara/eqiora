/**
 * Private Expressive Code plugin: static accessibility for locally scrollable
 * code regions.
 *
 * Expressive Code only makes an overflowing `pre` keyboard-focusable through a
 * client script, and a CSS pseudo-element cannot supply an accessible name. This
 * plugin runs after the locked Frames plugin, inside the official
 * `postprocessRenderedBlock` hook, and emits the semantics in the server HTML:
 *
 *  - every unwrapped block (`codeBlock.props.wrap !== true`, equivalently a `pre`
 *    without the `wrap` class) gets `role="region"`, `tabindex="0"`, and an
 *    `aria-label` on its `pre`;
 *  - the label is the non-empty visible Frames title when there is one; otherwise
 *    it is the raw rendered `data-language` identifier, which is also inserted
 *    into the existing frame header as a real visible text node with the private
 *    class `eq-code-region-label`, and the frame receives the private marker
 *    class `has-code-region-label` so the stylesheet can show the header.
 *
 * Wrapped blocks are left untouched. Any ambiguity in the locked frame shape (no
 * or several frames, headers, titles, or `pre` elements; wrap prop/class
 * disagreement; missing label source; pre-existing region attributes) fails the
 * build with a bounded diagnostic instead of emitting partial semantics. The
 * plugin walks each rendered block once, adds at most one label node and one
 * marker class, and never rewrites serialized HTML, reads files, or runs on the
 * client.
 */
import { definePlugin, type ExpressiveCodeBlock } from '@astrojs/starlight/expressive-code';
import { addClassName, getClassNames, h, setProperty } from '@astrojs/starlight/expressive-code/hast';
import type { Element, ElementContent } from '@astrojs/starlight/expressive-code/hast';

const PLUGIN_NAME = 'Eqiora code region accessibility';
const LABEL_CLASS = 'eq-code-region-label';
const MARKER_CLASS = 'has-code-region-label';

function normalizeText(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}

/** Concatenates the text nodes below `node` (one pass, no HTML serialization). */
function textContent(node: ElementContent): string {
  if (node.type === 'text') return node.value;
  if (node.type !== 'element') return '';
  let text = '';
  for (const child of node.children) text += textContent(child);
  return text;
}

function hasClass(element: Element, className: string): boolean {
  return getClassNames(element).includes(className);
}

interface FrameShape {
  frames: Element[];
  headers: Element[];
  pres: Element[];
}

/** Collects every frame figure, header figcaption, and `pre` in one traversal. */
function collectShape(root: Element): FrameShape {
  const shape: FrameShape = { frames: [], headers: [], pres: [] };
  const visit = (element: Element): void => {
    if (element.tagName === 'figure' && hasClass(element, 'frame')) shape.frames.push(element);
    if (element.tagName === 'figcaption' && hasClass(element, 'header')) shape.headers.push(element);
    if (element.tagName === 'pre') shape.pres.push(element);
    for (const child of element.children) {
      if (child.type === 'element') visit(child);
    }
  };
  visit(root);
  return shape;
}

function describe(codeBlock: ExpressiveCodeBlock): string {
  const document = codeBlock.parentDocument;
  const source = document?.sourceFilePath || '<unknown source>';
  const index = document?.positionInDocument?.groupIndex;
  const position = index === undefined ? '' : `, block ${index + 1}`;
  return `${source}${position}, language "${codeBlock.language}"`;
}

function fail(codeBlock: ExpressiveCodeBlock, predicate: string): never {
  throw new Error(`${PLUGIN_NAME}: ${predicate} (${describe(codeBlock)})`);
}

export const codeRegionAccessibilityPlugin = definePlugin({
  name: PLUGIN_NAME,
  hooks: {
    postprocessRenderedBlock: ({ codeBlock, renderData }) => {
      const root = renderData.blockAst;
      const { frames, headers, pres } = collectShape(root);
      if (frames.length !== 1 || frames[0] !== root) {
        fail(codeBlock, `expected the rendered block to be exactly one Frames figure, found ${frames.length}`);
      }
      if (headers.length !== 1) {
        fail(codeBlock, `expected exactly one frame header, found ${headers.length}`);
      }
      if (pres.length !== 1) {
        fail(codeBlock, `expected exactly one pre element, found ${pres.length}`);
      }
      const frame = frames[0];
      const header = headers[0];
      const pre = pres[0];
      if (frame === undefined || header === undefined || pre === undefined) {
        fail(codeBlock, 'frame shape is incomplete');
      }

      // Eligibility: the prop and the rendered class must agree; wrapped blocks
      // are outside this plugin.
      const wrapProp = codeBlock.props.wrap === true;
      const wrapClass = hasClass(pre, 'wrap');
      if (wrapProp !== wrapClass) {
        fail(
          codeBlock,
          `wrap prop (${String(codeBlock.props.wrap)}) and rendered pre.wrap class (${String(wrapClass)}) disagree`,
        );
      }
      if (wrapProp) return;

      for (const property of ['role', 'tabIndex', 'ariaLabel'] as const) {
        if (pre.properties[property] !== undefined) {
          fail(codeBlock, `pre already carries "${property}" before this plugin runs`);
        }
      }

      // Label precedence: a non-empty visible Frames title wins; otherwise the
      // raw rendered language identifier is inserted as a real visible label.
      const titleSpans = header.children.filter(
        (child): child is Element => child.type === 'element' && child.tagName === 'span' && hasClass(child, 'title'),
      );
      if (titleSpans.length > 1) {
        fail(codeBlock, `expected at most one frame title, found ${titleSpans.length}`);
      }
      const titleSpan = titleSpans[0];
      const titleText = titleSpan === undefined ? '' : normalizeText(textContent(titleSpan));
      const hasTitleClass = hasClass(frame, 'has-title');
      if (hasTitleClass !== (titleText.length > 0)) {
        fail(codeBlock, `frame has-title class (${String(hasTitleClass)}) and visible title text disagree`);
      }

      let label: string;
      if (titleText.length > 0) {
        label = titleText;
      } else {
        const language = pre.properties.dataLanguage;
        if (typeof language !== 'string' || normalizeText(language).length === 0) {
          fail(codeBlock, 'block has neither a visible title nor a rendered data-language identifier');
        }
        label = normalizeText(language);
        header.children.push(h('span', { className: LABEL_CLASS }, label));
        addClassName(frame, MARKER_CLASS);
      }

      setProperty(pre, 'role', 'region');
      setProperty(pre, 'tabIndex', '0');
      setProperty(pre, 'ariaLabel', label);
    },
  },
});
