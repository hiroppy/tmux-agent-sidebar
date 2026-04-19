// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import sitemap from '@astrojs/sitemap';

const SITE = 'https://hiroppy.github.io';
const BASE = '/tmux-agent-sidebar';
const OG_IMAGE = `${SITE}${BASE}/og-image.png`;
const DESCRIPTION =
  'tmux-agent-sidebar — one tmux sidebar that tracks every Claude Code and Codex pane across every session and window. See status, prompts, Git state, activity, and worktrees without switching windows.';

export default defineConfig({
  site: SITE,
  base: BASE,
  integrations: [
    sitemap(),
    starlight({
      title: 'tmux-agent-sidebar',
      description: DESCRIPTION,
      favicon: '/favicon.svg',
      logo: {
        src: './src/assets/logo.svg',
        replacesTitle: false,
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/hiroppy/tmux-agent-sidebar',
        },
        {
          // Starlight's icon enum has no `x` — `twitter` renders the
          // bird glyph, which is the conventional stand-in for X.
          icon: 'twitter',
          label: 'X',
          href: 'https://x.com/about_hiroppy',
        },
      ],
      editLink: {
        baseUrl:
          'https://github.com/hiroppy/tmux-agent-sidebar/edit/main/website/',
      },
      customCss: ['./src/styles/custom.css'],
      components: {
        Hero: './src/overrides/Hero.astro',
        ThemeSelect: './src/overrides/ThemeSelect.astro',
      },
      // Dark-only site. Clear any previously-stored theme preference
      // from localStorage and pin data-theme before Starlight's own
      // theme script runs, so returning visitors who used the light
      // toggle before this change don't stay stuck in light mode.
      head: [
        {
          tag: 'script',
          content:
            "try{localStorage.removeItem('starlight-theme');}catch(e){}document.documentElement.dataset.theme='dark';",
        },
        // OG image (set once globally; Starlight already emits og:title,
        // og:url, og:description per page).
        { tag: 'meta', attrs: { property: 'og:image', content: OG_IMAGE } },
        { tag: 'meta', attrs: { property: 'og:image:width', content: '1200' } },
        { tag: 'meta', attrs: { property: 'og:image:height', content: '630' } },
        { tag: 'meta', attrs: { name: 'twitter:image', content: OG_IMAGE } },
        { tag: 'meta', attrs: { name: 'twitter:creator', content: '@about_hiroppy' } },
        // Dark UI hint so address bars and PWA chrome match the theme.
        { tag: 'meta', attrs: { name: 'theme-color', content: '#0b1220' } },
        // Structured data: position this as a developer tool so Google
        // can render a richer result (name, description, homepage, repo).
        {
          tag: 'script',
          attrs: { type: 'application/ld+json' },
          content: JSON.stringify({
            '@context': 'https://schema.org',
            '@type': 'SoftwareApplication',
            name: 'tmux-agent-sidebar',
            applicationCategory: 'DeveloperApplication',
            operatingSystem: 'macOS, Linux',
            description: DESCRIPTION,
            url: `${SITE}${BASE}/`,
            image: OG_IMAGE,
            codeRepository: 'https://github.com/hiroppy/tmux-agent-sidebar',
            programmingLanguage: 'Rust',
            license: 'https://github.com/hiroppy/tmux-agent-sidebar/blob/main/LICENSE',
            offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
            author: {
              '@type': 'Person',
              name: 'Yuta Hiroto',
              url: 'https://hiroppy.me',
            },
          }),
        },
      ],
      lastUpdated: true,
      pagination: true,
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { slug: 'getting-started/installation' },
            { slug: 'getting-started/claude-code' },
            { slug: 'getting-started/codex' },
          ],
        },
        {
          label: 'Features',
          items: [
            { slug: 'features/agent-pane' },
            { slug: 'features/worktree' },
            { slug: 'features/activity-log' },
            { slug: 'features/git-status' },
            { slug: 'features/notifications' },
          ],
        },
        {
          label: 'Agents',
          items: [
            { slug: 'agents' },
            { slug: 'agents/claude-code' },
            { slug: 'agents/codex' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { slug: 'reference/keybindings' },
            { slug: 'reference/tmux-options' },
            { slug: 'reference/scripting' },
          ],
        },
      ],
    }),
  ],
});
