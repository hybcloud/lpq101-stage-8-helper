import { DurableObject } from "cloudflare:workers";

import projectConfig from "../project-config.json";

import {
	STATE_COUNT,
	STAGE_STATES,
	createStageState,
	movementBetween,
	type Movement,
	type StageState,
} from "./gray-code";

const ROOM_CODE_PATTERN = /^[0-9A-Z]{4}$/;
const ROOM_ALPHABET = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const MAX_CREATE_ATTEMPTS = 24;
const ROOM_STORAGE_KEY = "room";
const OWNER_ROOM_STORAGE_KEY = "owner-room-code";
const OWNER_REGISTRY_PREFIX = "owner:";
const OWNER_COOKIE_NAME = "lpq_stage8_owner";
const OWNER_HASH_HEADER = "X-LPQ-Owner-Hash";
const OWNER_COOKIE_MAX_AGE = 365 * 24 * 60 * 60;
const ROOM_RELEASE_AFTER_MS = 5 * 60 * 1000;
const ROOM_RELEASE_AFTER_SECONDS = ROOM_RELEASE_AFTER_MS / 1000;
const OWNER_GUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const OWNER_HASH_PATTERN = /^[0-9a-f]{64}$/;

type RoomAction = "next" | "previous" | "reset" | "sync";

interface StoredRoom {
	code: string;
	ownerHash: string;
	index: number;
	movement: Movement | null;
	revision: number;
	createdAt: string;
	updatedAt: string;
}

interface RoomSnapshot extends StageState {
	code: string;
	revision: number;
	releaseAfterSeconds: number;
	updatedAt: string;
}

interface InitializeBody {
	code: string;
	ownerHash: string;
}

interface ActionBody {
	action?: unknown;
	index?: unknown;
}

function json(data: unknown, status = 200, extraHeaders?: HeadersInit): Response {
	const headers = new Headers(extraHeaders);
	headers.set("Content-Type", "application/json; charset=utf-8");
	headers.set("Cache-Control", "no-store");
	return Response.json(data, { status, headers });
}

function randomRoomCode(): string {
	const bytes = new Uint8Array(4);
	crypto.getRandomValues(bytes);
	return Array.from(bytes, (byte) => ROOM_ALPHABET[byte % ROOM_ALPHABET.length]).join("");
}

async function sha256(value: string): Promise<string> {
	const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
	return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function constantTimeEqual(left: string, right: string): boolean {
	if (left.length !== right.length) {
		return false;
	}
	let difference = 0;
	for (let index = 0; index < left.length; index += 1) {
		difference |= left.charCodeAt(index) ^ right.charCodeAt(index);
	}
	return difference === 0;
}

function roomSnapshot(room: StoredRoom): RoomSnapshot {
	return {
		code: room.code,
		revision: room.revision,
		releaseAfterSeconds: ROOM_RELEASE_AFTER_SECONDS,
		updatedAt: room.updatedAt,
		...createStageState(room.index, room.movement),
	};
}

function internalRequest(path: string, init?: RequestInit): Request {
	return new Request(`https://room.internal${path}`, init);
}

function readCookie(request: Request, name: string): string | null {
	const cookieHeader = request.headers.get("Cookie") ?? "";
	for (const part of cookieHeader.split(";")) {
		const separator = part.indexOf("=");
		if (separator < 0) {
			continue;
		}
		if (part.slice(0, separator).trim() === name) {
			return part.slice(separator + 1).trim();
		}
	}
	return null;
}

function readOwnerGuid(request: Request): string | null {
	const guid = readCookie(request, OWNER_COOKIE_NAME);
	return guid && OWNER_GUID_PATTERN.test(guid) ? guid.toLowerCase() : null;
}

function ownerCookie(guid: string, request: Request): string {
	const secure = new URL(request.url).protocol === "https:" ? "; Secure" : "";
	return `${OWNER_COOKIE_NAME}=${guid}; Path=/; Max-Age=${OWNER_COOKIE_MAX_AGE}; HttpOnly; SameSite=Lax${secure}`;
}

function addCookie(response: Response, cookie: string): Response {
	const headers = new Headers(response.headers);
	headers.append("Set-Cookie", cookie);
	return new Response(response.body, {
		status: response.status,
		statusText: response.statusText,
		headers,
	});
}

export class MyDurableObject extends DurableObject<Env> {
	private readonly state: DurableObjectState;
	private readonly appEnv: Env;
	private room: StoredRoom | null = null;

	constructor(state: DurableObjectState, env: Env) {
		super(state, env);
		this.state = state;
		this.appEnv = env;
		state.blockConcurrencyWhile(async () => {
			this.room = (await state.storage.get<StoredRoom>(ROOM_STORAGE_KEY)) ?? null;
		});
		state.setWebSocketAutoResponse(new WebSocketRequestResponsePair("ping", "pong"));
	}

	async fetch(request: Request): Promise<Response> {
		const path = new URL(request.url).pathname;
		if (path === "/owner-room" && request.method === "POST") {
			return this.state.blockConcurrencyWhile(() => this.resolveOwnerRoom(request, true));
		}
		if (path === "/owner-room" && request.method === "GET") {
			return this.resolveOwnerRoom(request, false);
		}
		if (path === "/initialize" && request.method === "POST") {
			return this.initialize(request);
		}
		if (path === "/state" && request.method === "GET") {
			return this.getState();
		}
		if (path === "/owner" && request.method === "GET") {
			return this.getOwnerState(request);
		}
		if (path === "/action" && request.method === "POST") {
			return this.applyAction(request);
		}
		if (path === "/socket" && request.method === "GET") {
			return this.openSocket(request);
		}
		return json({ error: "Not found" }, 404);
	}

	private async resolveOwnerRoom(request: Request, createIfMissing: boolean): Promise<Response> {
		const ownerHash = request.headers.get(OWNER_HASH_HEADER) ?? "";
		if (!OWNER_HASH_PATTERN.test(ownerHash)) {
			return json({ error: "Invalid owner session" }, 403);
		}

		const existingCode = await this.state.storage.get<string>(OWNER_ROOM_STORAGE_KEY);
		if (existingCode && ROOM_CODE_PATTERN.test(existingCode)) {
			const existingResponse = await this.appEnv.MY_DURABLE_OBJECT.getByName(existingCode).fetch(
				internalRequest("/owner", { headers: { [OWNER_HASH_HEADER]: ownerHash } }),
			);
			if (existingResponse.ok) {
				const payload = await existingResponse.json<{ state: RoomSnapshot }>();
				return json({ ...payload, reused: true });
			}
			if (existingResponse.status >= 500) {
				return json({ error: "Unable to restore the owner room" }, 502);
			}
			await this.state.storage.delete(OWNER_ROOM_STORAGE_KEY);
		}

		if (!createIfMissing) {
			return json({ error: "No active room for this owner" }, 404);
		}

		for (let attempt = 0; attempt < MAX_CREATE_ATTEMPTS; attempt += 1) {
			const code = randomRoomCode();
			const response = await this.appEnv.MY_DURABLE_OBJECT.getByName(code).fetch(
				internalRequest("/initialize", {
					method: "POST",
					headers: { "Content-Type": "application/json" },
					body: JSON.stringify({ code, ownerHash }),
				}),
			);
			if (response.status === 409) {
				continue;
			}
			if (!response.ok) {
				return json({ error: "Unable to create room" }, 500);
			}
			await this.state.storage.put(OWNER_ROOM_STORAGE_KEY, code);
			const payload = await response.json<{ state: RoomSnapshot }>();
			return json({ ...payload, reused: false }, 201);
		}
		return json({ error: "Unable to allocate a room code; please try again" }, 503);
	}

	private async initialize(request: Request): Promise<Response> {
		if (this.room) {
			return json({ error: "Room code is already in use" }, 409);
		}

		let body: InitializeBody;
		try {
			body = await request.json<InitializeBody>();
		} catch {
			return json({ error: "Invalid initialization request" }, 400);
		}
		if (!ROOM_CODE_PATTERN.test(body.code) || !OWNER_HASH_PATTERN.test(body.ownerHash)) {
			return json({ error: "Invalid initialization request" }, 400);
		}

		const now = new Date().toISOString();
		this.room = {
			code: body.code,
			ownerHash: body.ownerHash,
			index: 0,
			movement: null,
			revision: 1,
			createdAt: now,
			updatedAt: now,
		};
		await this.state.storage.put(ROOM_STORAGE_KEY, this.room);
		await this.scheduleRelease();
		return json({ state: roomSnapshot(this.room) }, 201);
	}

	private getState(): Response {
		if (!this.room) {
			return json({ error: "Room not found" }, 404);
		}
		return json({ state: roomSnapshot(this.room) });
	}

	private getOwnerState(request: Request): Response {
		if (!this.room) {
			return json({ error: "Room not found" }, 404);
		}
		if (!this.isOwner(request)) {
			return json({ error: "Room owner credentials are invalid" }, 403);
		}
		return json({ state: roomSnapshot(this.room) });
	}

	private async applyAction(request: Request): Promise<Response> {
		if (!this.room) {
			return json({ error: "Room not found" }, 404);
		}
		if (!this.isOwner(request)) {
			return json({ error: "Only the room owner can control this room" }, 403);
		}

		let body: ActionBody;
		try {
			body = await request.json<ActionBody>();
		} catch {
			return json({ error: "Invalid action" }, 400);
		}
		if (
			body.action !== "next" &&
			body.action !== "previous" &&
			body.action !== "reset" &&
			body.action !== "sync"
		) {
			return json({ error: "Invalid action" }, 400);
		}
		if (
			body.action === "sync" &&
			(typeof body.index !== "number" || !Number.isInteger(body.index) || body.index < 0 || body.index >= STATE_COUNT)
		) {
			return json({ error: "Invalid state index" }, 400);
		}

		const action: RoomAction = body.action;
		let nextIndex = this.room.index;
		let movement: Movement | null = this.room.movement;
		if (action === "next" && nextIndex < STATE_COUNT - 1) {
			const before = STAGE_STATES[nextIndex];
			nextIndex += 1;
			movement = movementBetween(before, STAGE_STATES[nextIndex]);
		} else if (action === "previous" && nextIndex > 0) {
			const before = STAGE_STATES[nextIndex];
			nextIndex -= 1;
			movement = movementBetween(before, STAGE_STATES[nextIndex]);
		} else if (action === "reset") {
			nextIndex = 0;
			movement = null;
		} else if (action === "sync") {
			nextIndex = body.index as number;
			movement = null;
		}

		this.room = {
			...this.room,
			index: nextIndex,
			movement,
			revision: this.room.revision + 1,
			updatedAt: new Date().toISOString(),
		};
		await this.state.storage.put(ROOM_STORAGE_KEY, this.room);
		const snapshot = roomSnapshot(this.room);
		this.broadcast(snapshot);
		return json({ state: snapshot });
	}

	private async openSocket(request: Request): Promise<Response> {
		if (!this.room) {
			return json({ error: "Room not found" }, 404);
		}
		if (request.headers.get("Upgrade")?.toLowerCase() !== "websocket") {
			return json({ error: "WebSocket upgrade required" }, 426, { Upgrade: "websocket" });
		}

		const pair = new WebSocketPair();
		const client = pair[0];
		const server = pair[1];
		this.state.acceptWebSocket(server, ["viewer"]);
		await this.state.storage.deleteAlarm();
		server.send(JSON.stringify({ type: "state", state: roomSnapshot(this.room) }));
		return new Response(null, { status: 101, webSocket: client });
	}

	webSocketMessage(socket: WebSocket, message: string | ArrayBuffer): void {
		if (message === "sync" && this.room) {
			socket.send(JSON.stringify({ type: "state", state: roomSnapshot(this.room) }));
		}
	}

	async webSocketClose(socket: WebSocket, code: number, reason: string): Promise<void> {
		socket.close(code, reason);
		await this.scheduleRelease();
	}

	async webSocketError(socket: WebSocket): Promise<void> {
		socket.close(1011, "WebSocket error");
		await this.scheduleRelease();
	}

	async alarm(): Promise<void> {
		if (this.state.getWebSockets().length > 0) {
			return;
		}
		await this.state.storage.deleteAll();
		this.room = null;
	}

	private broadcast(snapshot: RoomSnapshot): void {
		const message = JSON.stringify({ type: "state", state: snapshot });
		for (const socket of this.state.getWebSockets("viewer")) {
			try {
				socket.send(message);
			} catch {
				// Stale sockets are discarded by the runtime after their close event.
			}
		}
	}

	private isOwner(request: Request): boolean {
		if (!this.room) {
			return false;
		}
		const ownerHash = request.headers.get(OWNER_HASH_HEADER) ?? "";
		return OWNER_HASH_PATTERN.test(ownerHash) && constantTimeEqual(ownerHash, this.room.ownerHash);
	}

	private scheduleRelease(): Promise<void> {
		return this.state.storage.setAlarm(Date.now() + ROOM_RELEASE_AFTER_MS);
	}
}

async function ownerRoomResponse(request: Request, env: Env, createIfMissing: boolean): Promise<Response> {
	let guid = readOwnerGuid(request);
	if (!guid && !createIfMissing) {
		return json({ error: "No owner session cookie" }, 404);
	}
	guid ??= crypto.randomUUID();
	const ownerHash = await sha256(guid);
	const registry = env.MY_DURABLE_OBJECT.getByName(`${OWNER_REGISTRY_PREFIX}${guid}`);
	const response = await registry.fetch(
		internalRequest("/owner-room", {
			method: createIfMissing ? "POST" : "GET",
			headers: { [OWNER_HASH_HEADER]: ownerHash },
		}),
	);
	return createIfMissing ? addCookie(response, ownerCookie(guid, request)) : response;
}

async function routeRoomRequest(request: Request, env: Env, code: string, operation?: string): Promise<Response> {
	const stub = env.MY_DURABLE_OBJECT.getByName(code);
	if (!operation && request.method === "GET") {
		return stub.fetch(internalRequest("/state"));
	}
	if (operation === "action" && request.method === "POST") {
		const guid = readOwnerGuid(request);
		if (!guid) {
			return json({ error: "Only the room owner can control this room" }, 403);
		}
		const body = await request.text();
		if (body.length > 256) {
			return json({ error: "Request is too large" }, 413);
		}
		return stub.fetch(
			internalRequest("/action", {
				method: "POST",
				headers: {
					[OWNER_HASH_HEADER]: await sha256(guid),
					"Content-Type": "application/json",
				},
				body,
			}),
		);
	}
	if (operation === "socket" && request.method === "GET") {
		return stub.fetch(
			internalRequest("/socket", {
				headers: { Upgrade: request.headers.get("Upgrade") ?? "" },
			}),
		);
	}
	return json({ error: "Method not allowed" }, 405, { Allow: operation === "action" ? "POST" : "GET" });
}

export default {
	async fetch(request, env): Promise<Response> {
		const url = new URL(request.url);
		if (url.pathname === "/api/config") {
			if (request.method !== "GET") {
				return json({ error: "Method not allowed" }, 405, { Allow: "GET" });
			}
			return json({
				workerName: projectConfig.workerName,
				serviceOrigin: projectConfig.serviceOrigin,
			});
		}
		if (url.pathname === "/api/rooms") {
			if (request.method !== "POST") {
				return json({ error: "Method not allowed" }, 405, { Allow: "POST" });
			}
			return ownerRoomResponse(request, env, true);
		}
		if (url.pathname === "/api/owner/room") {
			if (request.method !== "GET") {
				return json({ error: "Method not allowed" }, 405, { Allow: "GET" });
			}
			return ownerRoomResponse(request, env, false);
		}

		const match = url.pathname.match(/^\/api\/rooms\/([0-9A-Z]{4})(?:\/(action|socket))?$/);
		if (match) {
			return routeRoomRequest(request, env, match[1], match[2]);
		}
		return json({ error: "Not found" }, 404);
	},
} satisfies ExportedHandler<Env>;
