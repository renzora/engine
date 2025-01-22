import { FastifyInstance, FastifyReply, FastifyRequest } from 'fastify';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

async function downloadPluginFiles(
  owner: string,
  repo: string,
  pluginPath: string,
  localPath: string
): Promise<void> {
  const apiUrl = `https://api.github.com/repos/${owner}/${repo}/contents/${pluginPath}`;

  console.log(`[downloadPluginFiles] Fetching from: ${apiUrl}`);
  const res = await fetch(apiUrl);
  if (!res.ok) {
    throw new Error(`GitHub API responded with status ${res.status}`);
  }

  const data = await res.json();

  if (Array.isArray(data)) {
    console.log(`[downloadPluginFiles] "${pluginPath}" is a directory with ${data.length} item(s).`);

    if (!fs.existsSync(localPath)) {
      fs.mkdirSync(localPath, { recursive: true });
      console.log(`[downloadPluginFiles] Created directory: ${localPath}`);
    }

    for (const item of data) {
      const itemLocalPath = path.join(localPath, item.name);

      if (item.type === 'dir') {
        console.log(`[downloadPluginFiles] Found subdirectory: ${item.path}`);
        await downloadPluginFiles(owner, repo, item.path, itemLocalPath);
      } else if (item.type === 'file') {
        const parentDir = path.dirname(itemLocalPath);
        if (!fs.existsSync(parentDir)) {
          fs.mkdirSync(parentDir, { recursive: true });
          console.log(`[downloadPluginFiles] Created parent directory for file: ${parentDir}`);
        }

        console.log(
          `[downloadPluginFiles] Downloading file:\n  from: ${item.download_url}\n  to:   ${itemLocalPath}`
        );
        const fileRes = await fetch(item.download_url);
        const fileContent = await fileRes.text();
        fs.writeFileSync(itemLocalPath, fileContent, 'utf8');
      } else {
        console.log(
          `[downloadPluginFiles] Skipping unknown item type "${item.type}" at path: ${item.path}`
        );
      }
    }
  } else {
    console.log(`[downloadPluginFiles] "${pluginPath}" is a single file object.`);

    const dirPath = path.dirname(localPath);
    if (!fs.existsSync(dirPath)) {
      fs.mkdirSync(dirPath, { recursive: true });
      console.log(`[downloadPluginFiles] Created directory for single file: ${dirPath}`);
    }

    console.log(
      `[downloadPluginFiles] Downloading single file:\n  from: ${data.download_url}\n  to:   ${localPath}`
    );

    const fileRes = await fetch(data.download_url);
    const fileContent = await fileRes.text();
    fs.writeFileSync(localPath, fileContent, 'utf8');
  }
}

function removePluginDir(pluginDirPath: string): void {
  console.log(`[removePluginDir] Removing folder: ${pluginDirPath}`);
  if (fs.existsSync(pluginDirPath)) {
    fs.rmSync(pluginDirPath, { recursive: true, force: true });
  }
}

export async function pluginRoutes(fastify: FastifyInstance) {
  const localPluginsPath = path.join(__dirname, '..', '..', 'client', 'plugins');
  console.log(`[pluginRoutes] localPluginsPath: ${localPluginsPath}`);

  fastify.get('/list', async (_request: FastifyRequest, reply: FastifyReply) => {
    try {
      const apiUrl = 'https://api.github.com/repos/renzora/plugins/contents';
      console.log(`[GET /list] Fetching GitHub repo contents from: ${apiUrl}`);

      const res = await fetch(apiUrl);
      if (!res.ok) {
        console.error(`[GET /list] GitHub API error: ${res.status}`);
        return reply.status(500).send({ message: `GitHub API error: ${res.status}` });
      }

      const data = await res.json();
      const pluginDirs = (data as any[]).filter(item => item.type === 'dir');

      const responseData = pluginDirs.map(dir => {
        const pluginDirPath = path.join(localPluginsPath, dir.name);
        const isInstalled = fs.existsSync(pluginDirPath);
        return {
          name: dir.name,
          installed: isInstalled,
        };
      });

      return reply.send(responseData);
    } catch (err) {
      console.error(`[GET /list] Error listing plugins:`, err);
      return reply.status(500).send({ message: 'Failed to list plugins.' });
    }
  });

  fastify.get('/download/:pluginName', async (request, reply) => {
    const { pluginName } = request.params as { pluginName?: string };
    if (!pluginName) {
      console.warn(`[GET /download] Missing pluginName param`);
      return reply.status(400).send({ message: 'Missing pluginName.' });
    }

    try {
      const owner = 'renzora';
      const repo = 'plugins';
      const pluginLocalPath = path.join(localPluginsPath, pluginName);

      console.log(`\n[GET /download/:pluginName] Downloading "${pluginName}" to: ${pluginLocalPath}`);
      await downloadPluginFiles(owner, repo, pluginName, pluginLocalPath);

      console.log(`[GET /download/:pluginName] Plugin "${pluginName}" downloaded successfully.`);
      return reply.send({ message: `Plugin "${pluginName}" downloaded successfully.` });
    } catch (err) {
      console.error(`[GET /download/:pluginName] Error while downloading plugin:`, err);
      return reply.status(500).send({ message: 'Failed to download plugin.' });
    }
  });

  fastify.delete('/uninstall/:pluginName', async (request, reply) => {
    const { pluginName } = request.params as { pluginName?: string };
    if (!pluginName) {
      console.warn(`[DELETE /uninstall] Missing pluginName param`);
      return reply.status(400).send({ message: 'Missing pluginName.' });
    }

    try {
      const pluginDirPath = path.join(localPluginsPath, pluginName);

      console.log(`[DELETE /uninstall/:pluginName] Uninstalling "${pluginName}".`);
      removePluginDir(pluginDirPath);

      return reply.send({ message: `Plugin "${pluginName}" uninstalled successfully.` });
    } catch (err) {
      console.error(`[DELETE /uninstall/:pluginName] Error while uninstalling plugin:`, err);
      return reply.status(500).send({ message: 'Failed to uninstall plugin.' });
    }
  });
}
