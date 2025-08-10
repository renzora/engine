import fs from 'node:fs/promises'
import fastify from 'fastify'
import fastifyCompress from '@fastify/compress'
import fastifyStatic from '@fastify/static'
import { generateHydrationScript } from 'solid-js/web'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const isProduction = process.env.NODE_ENV === 'production'
const port = process.env.PORT || 5173
const base = process.env.BASE || '/'

const templateHtml = isProduction ? await fs.readFile('./dist/client/index.html', 'utf-8') : ''

const app = fastify({
  logger: true
})

if (isProduction) {
  await app.register(fastifyCompress)
  await app.register(fastifyStatic, {
    root: path.join(fileURLToPath(import.meta.url), '../dist/client'),
    prefix: base,
    wildcard: false,
    decorateReply: false
  })
}

let vite
if (!isProduction) {
  const { createServer } = await import('vite')
  vite = await createServer({
    server: { middlewareMode: true },
    appType: 'custom',
    base,
  })
  app.addHook('onRequest', (req, reply, next) => {
    vite.middlewares(req.raw, reply.raw, next)
  })
}

app.get('*', async (req, reply) => {
  try {
    const url = req.url.replace(base, '')

    let template
    let render
    
    if (!isProduction) {
      template = await fs.readFile('./index.html', 'utf-8')
      template = await vite.transformIndexHtml(url, template)
      render = (await vite.ssrLoadModule('/src/entry-server.jsx')).render
    } else {
      template = templateHtml
      render = (await import('./dist/server/entry-server.js')).render
    }

    const rendered = await render(url)
    const head = (rendered.head ?? '') + generateHydrationScript()

    const html = template
      .replace(`<!--app-head-->`, head)
      .replace(`<!--app-html-->`, rendered.html ?? '')

    reply.type('text/html').send(html)
  } catch (e) {
    vite?.ssrFixStacktrace(e)
    app.log.error(e)
    reply.status(500).send(e.stack)
  }
})

app.listen({ port }, (err) => {
  if (err) {
    app.log.error(err)
    process.exit(1)
  }
  console.log(`Server started at http://localhost:${port}`)
})