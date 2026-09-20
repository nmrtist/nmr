import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import linksValidator from 'starlight-links-validator';

export default defineConfig({
  site: 'https://nmr.nmrtist.space',
  integrations: [
    starlight({
      title: 'nmr',
      description: 'Read and process supported NMR data in Rust.',
      customCss: ['./src/styles/custom.css'],
      editLink: {
        baseUrl: 'https://github.com/nmrtist/nmr/edit/main/docs/',
      },
      lastUpdated: true,
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/nmrtist/nmr' },
      ],
      sidebar: [
        { label: 'Overview', link: '/' },
        { label: 'Getting started', link: '/getting-started/' },
        {
          label: 'Guides',
          items: [
            { label: 'Reading data', link: '/reading/' },
            { label: 'Processing and export', link: '/processing/' },
            { label: 'Spectrum operations and estimation', link: '/spectrum-operations/' },
            { label: 'Automatic NUS reconstruction', link: '/nus-automatic/' },
            { label: 'Host integration and snapshots', link: '/host-integration/' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Format support', link: '/formats/' },
            { label: 'JEOL conventions and limitations', link: '/jeol-conventions/' },
            { label: 'Data model and conventions', link: '/data-model/' },
            { label: 'Processing contracts', link: '/processing-reference/' },
            { label: 'Errors and resource limits', link: '/errors-limits/' },
            { label: 'Scientific contracts and evidence', link: '/scientific-contract/' },
            { label: 'Execution reports and comparison', link: '/reproducibility/' },
          ],
        },
        { label: 'Contributing', link: '/contributing/' },
      ],
      plugins: [linksValidator()],
    }),
  ],
});
