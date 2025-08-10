import Fastify from 'fastify'
import FastifyVite from '@fastify/vite'
import projectRoutes from './server/routes/projects.js'
import { readFileSync, existsSync } from 'fs'
import { resolve } from 'path'

console.log('\x1b[36m' + `
        ██████╗ ███████╗███╗   ██╗███████╗ ██████╗ ██████╗  █████╗ 
        ██╔══██╗██╔════╝████╗  ██║╚══███╔╝██╔═══██╗██╔══██╗██╔══██╗
        ██████╔╝█████╗  ██╔██╗ ██║  ███╔╝ ██║   ██║██████╔╝███████║
        ██╔══██╗██╔══╝  ██║╚██╗██║ ███╔╝  ██║   ██║██╔══██╗██╔══██║
        ██║  ██║███████╗██║ ╚████║███████╗╚██████╔╝██║  ██║██║  ██║
        ╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝

                ███████╗███╗   ██╗ ██████╗ ██╗███╗   ██╗███████╗
                ██╔════╝████╗  ██║██╔════╝ ██║████╗  ██║██╔════╝
                █████╗  ██╔██╗ ██║██║  ███╗██║██╔██╗ ██║█████╗  
                ██╔══╝  ██║╚██╗██║██║   ██║██║██║╚██╗██║██╔══╝  
                ███████╗██║ ╚████║╚██████╔╝██║██║ ╚████║███████╗
                ╚══════╝╚═╝  ╚═══╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝╚══════╝
` + '\x1b[0m')

const port = process.env.PORT || 3000

const keyPath = resolve(process.cwd(), 'localhost+2-key.pem')
const certPath = resolve(process.cwd(), 'localhost+2.pem')
const hasSSL = existsSync(keyPath) && existsSync(certPath)

const httpsOptions = hasSSL ? {
  key: readFileSync(keyPath),
  cert: readFileSync(certPath)
} : null

const server = Fastify({
  logger: {
    transport: {
      target: '@fastify/one-line-logger'
    }
  },
  https: httpsOptions
})

await server.register(projectRoutes)

await server.register(FastifyVite, {
  root: import.meta.dirname,
  renderer: '@fastify/react',
})

server.setErrorHandler((error, req, reply) => {
  console.error(error)
  reply.send({ error })
})

await server.vite.ready()

const listenOptions = {
  port: port,
  host: '0.0.0.0'
}

await server.listen(listenOptions)

const protocol = hasSSL ? 'https' : 'http'

console.log(`🚀 Server running on ${protocol}://localhost:${port}`)
if (hasSSL) {
  console.log(`🔒 HTTPS enabled - WebGPU should work!`)
} else {
  console.log(`⚠️  Running on HTTP - WebGPU requires HTTPS. Run setup-https.bat to enable SSL.`)
}