const esbuild = require('esbuild');

esbuild.build({
  entryPoints: ['src/index.ts'],
  define: { 'process.env.NODE_ENV': '"production"' },
  bundle: true,
  platform: 'node',
  sourcemap: true,
  outfile: 'dist/index.js',
  external: ['fsevents'],
}).catch(() => process.exit(1));
