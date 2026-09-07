import { fileURLToPath } from 'node:url';
import { relative, resolve } from 'node:path';
import type { MdastPluginDefinition } from 'satteri';

const root = fileURLToPath(new URL('../../../../', import.meta.url));
const guideDirectory = resolve(root, 'docs/python');
const guideNames = new Set(['modeling.md', 'execution-and-arrays.md', 'differentiation.md']);

function isGuide(fileURL: URL | undefined): boolean {
  return !!fileURL && guideNames.has(relative(guideDirectory, fileURLToPath(fileURL)));
}

export const canonicalGuideLinks: MdastPluginDefinition = {
  name: 'eqiora-canonical-guide-presentation',
  heading(node, context) {
    // The Starlight route supplies the page title; keep every substantive heading.
    if (isGuide(context.fileURL) && node.depth === 1) context.removeNode(node);
  },
  link(node, context) {
    if (!isGuide(context.fileURL) || !node.url || /^[a-z]+:|^[/#]/i.test(node.url)) return;
    const target = new URL(node.url, context.fileURL);
    const filename = fileURLToPath(new URL(target.pathname, 'file://'));
    const guide = relative(guideDirectory, filename);
    if (guideNames.has(guide)) {
      context.setProperty(node, 'url', `/guides/${guide.replace(/\.md$/, '')}/${target.hash}`);
      return;
    }
    const sourcePath = relative(root, filename);
    if (sourcePath.startsWith('../') || sourcePath === '..') {
      throw new Error(`Canonical guide link escapes the repository: ${node.url}`);
    }
    const sha = process.env.EQIORA_SITE_SOURCE_SHA;
    if (!/^[0-9a-f]{40}$/.test(sha ?? '')) throw new Error('Canonical guide links require the exact source SHA');
    context.setProperty(node, 'url', `https://github.com/nkiyohara/eqiora/blob/${sha}/${sourcePath.split('/').map(encodeURIComponent).join('/')}${target.hash}`);
  },
};
