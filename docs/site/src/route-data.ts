import { defineRouteMiddleware } from '@astrojs/starlight/route-data';
import { canonicalGuide } from './canonical-guides';

export const onRequest = defineRouteMiddleware((context) => {
  const route = context.locals.starlightRoute;
  const section = context.url.pathname.split('/')[1];
  // Keep Starlight's generated links and active state, selecting only the group
  // for this reader destination. The global Header remains available everywhere.
  const active = route.sidebar.find((entry) =>
    entry.type === 'group' && entry.entries.some((child) =>
      child.type === 'link' && child.href.startsWith(`/${section}/`)),
  );
  route.sidebar = active ? [active] : [];
  route.hasSidebar = route.hasSidebar && !!active;
  if (section !== 'learn') route.pagination = { prev: undefined, next: undefined };
  else {
    for (const direction of ['prev', 'next'] as const) {
      if (!route.pagination[direction]?.href.startsWith('/learn/')) route.pagination[direction] = undefined;
    }
  }

  const name = context.url.pathname.match(/^\/guides\/([^/]+)\/$/)?.[1];
  const guide = name && canonicalGuide(name);
  if (guide) {
    route.headings = guide.getHeadings();
    if (route.toc) {
      const overview = route.toc.items[0];
      route.toc.items = [overview, ...route.headings.filter(({ depth }) => depth === 2)
        .map((heading) => ({ ...heading, children: [] }))];
    }
    route.editUrl = new URL(`https://github.com/nkiyohara/eqiora/edit/main/docs/python/${name}.md`);
  }
});
