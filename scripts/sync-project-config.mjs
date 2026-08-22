import { readFile, writeFile } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const projectConfigUrl = new URL("project-config.json", root);
const wranglerConfigUrl = new URL("wrangler.jsonc", root);

const projectConfig = JSON.parse(await readFile(projectConfigUrl, "utf8"));
if (typeof projectConfig.workerName !== "string" || !/^[a-z0-9-]+$/.test(projectConfig.workerName)) {
	throw new Error("project-config.json workerName must contain lowercase letters, digits, or dashes");
}
if (typeof projectConfig.serviceOrigin !== "string") {
	throw new Error("project-config.json serviceOrigin must be a string");
}

const serviceOrigin = new URL(projectConfig.serviceOrigin);
if (
	serviceOrigin.protocol !== "https:" ||
	serviceOrigin.pathname !== "/" ||
	serviceOrigin.search ||
	serviceOrigin.hash
) {
	throw new Error("project-config.json serviceOrigin must be an HTTPS origin without a path, query, or hash");
}

const jsonc = await readFile(wranglerConfigUrl, "utf8");
const wranglerConfig = JSON.parse(jsonc);
wranglerConfig.name = projectConfig.workerName;
wranglerConfig.routes = [{ pattern: serviceOrigin.hostname, custom_domain: true }];

const next = `${JSON.stringify(wranglerConfig, null, "\t")}\n`;
if (jsonc !== next) {
	await writeFile(wranglerConfigUrl, next);
}

process.stdout.write(`Synced ${projectConfig.workerName} → ${serviceOrigin.origin}\n`);
