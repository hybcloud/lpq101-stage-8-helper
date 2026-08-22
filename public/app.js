const ROOM_CODE_PATTERN = /^[0-9A-Z]{4}$/;
const STATE_COUNT = 126;
const GITHUB_URL = "https://github.com/hybcloud/lpq101-stage-8-helper";
const projectConfig = {
	serviceOrigin: location.origin,
};

const app = document.querySelector("#app");
const toast = document.querySelector("#toast");

const runtime = {
	routeId: 0,
	code: null,
	isHost: false,
	roomState: null,
	socket: null,
	reconnectTimer: null,
	pingTimer: null,
	reconnectDelay: 500,
	actionPending: false,
	toastTimer: null,
};

try {
	localStorage.removeItem("lpq-stage8-owner");
} catch {
	// The legacy JavaScript-accessible owner token is no longer used.
}

const LAYOUT_WIDTH = 268;
const LAYOUT_HEIGHT = 119;
const CHAIR_WIDTH = 63;
const CHAIR_HEIGHT = 41;
const CHAIR_POSITIONS = [
	[0, 0],
	[67, 0],
	[0, 39],
	[67, 39],
	[134, 39],
	[1, 78],
	[67, 78],
	[134, 78],
	[205, 78],
];
const layoutImage = new Image();
let layoutSourcePixels = null;

layoutImage.addEventListener("load", () => {
	const source = document.createElement("canvas");
	source.width = LAYOUT_WIDTH;
	source.height = LAYOUT_HEIGHT;
	const context = source.getContext("2d", { willReadFrequently: true });
	context.drawImage(layoutImage, 0, 0);
	layoutSourcePixels = context.getImageData(0, 0, LAYOUT_WIDTH, LAYOUT_HEIGHT);
	if (runtime.roomState) {
		renderStageLayout(runtime.roomState);
	}
});
layoutImage.addEventListener("error", () => {
	const canvas = document.querySelector("#stage-layout");
	if (canvas) {
		canvas.setAttribute("aria-label", "The Stage 8 layout image failed to load");
	}
});
layoutImage.src = "/stage8-chairs-layout.png";

function normalizeRoomCode(value) {
	return value.toUpperCase().replace(/[^0-9A-Z]/g, "").slice(0, 4);
}

function navigate(path, replace = false) {
	if (replace) {
		history.replaceState(null, "", path);
	} else {
		history.pushState(null, "", path);
	}
	void route();
}

function showToast(message, error = false) {
	clearTimeout(runtime.toastTimer);
	toast.textContent = message;
	toast.classList.toggle("border-rose-500/60", error);
	toast.classList.toggle("text-rose-100", error);
	toast.dataset.visible = "true";
	runtime.toastTimer = setTimeout(() => {
		toast.dataset.visible = "false";
	}, 2400);
}

async function readJson(response) {
	let payload;
	try {
		payload = await response.json();
	} catch {
		payload = {};
	}
	if (!response.ok) {
		const error = new Error(typeof payload.error === "string" ? payload.error : "Request failed");
		error.status = response.status;
		throw error;
	}
	return payload;
}

async function loadProjectConfig() {
	try {
		const payload = await readJson(await fetch("/api/config"));
		const origin = new URL(payload.serviceOrigin);
		if ((origin.protocol === "https:" || origin.protocol === "http:") && origin.pathname === "/") {
			projectConfig.serviceOrigin = origin.origin;
		}
	} catch {
		// Static-only previews fall back to the current origin.
	}
}

async function copyText(text) {
	if (navigator.clipboard?.writeText) {
		try {
			await navigator.clipboard.writeText(text);
			return true;
		} catch {
			// Fall through for browsers that require a different clipboard path.
		}
	}

	const textarea = document.createElement("textarea");
	textarea.value = text;
	textarea.setAttribute("readonly", "");
	textarea.style.position = "fixed";
	textarea.style.opacity = "0";
	document.body.append(textarea);
	textarea.select();
	let copied = false;
	try {
		copied = document.execCommand("copy");
	} finally {
		textarea.remove();
	}
	return copied;
}

function githubReference(className = "") {
	return `<a class="github-reference ${className}" href="${GITHUB_URL}" target="_blank" rel="noreferrer">Source on GitHub <span aria-hidden="true">↗</span></a>`;
}

function renderLobby() {
	document.title = "Ludibrium Party Quest · Stage 8 Helper";
	app.innerHTML = `
		<div class="flex min-h-dvh items-center justify-center px-4 py-[max(1.5rem,env(safe-area-inset-top))] sm:px-6">
			<section class="w-full max-w-md rounded-[2rem] border border-slate-800/90 bg-slate-950/75 p-5 shadow-2xl shadow-black/30 backdrop-blur sm:p-8">
				<div class="mb-8">
					<h1 class="tracking-tight text-white">
						<span class="block text-base font-semibold tracking-[0.12em] text-sky-400">Ludibrium Party Quest</span>
						<span class="mt-2 block text-4xl font-bold sm:text-5xl">Stage 8</span>
						<span class="block text-4xl font-bold sm:text-5xl">Helper</span>
					</h1>
				</div>

				<button id="create-room" class="flex min-h-12 w-full items-center justify-center rounded-2xl bg-sky-500 px-5 py-3 font-semibold text-slate-950 shadow-lg shadow-sky-950/30 transition hover:bg-sky-400 active:scale-[0.99] disabled:cursor-wait disabled:opacity-60">
					Create Room
				</button>

				<div class="my-7 flex items-center gap-3 text-xs text-slate-600" aria-hidden="true">
					<span class="h-px flex-1 bg-slate-800"></span>
					<span>or join a room</span>
					<span class="h-px flex-1 bg-slate-800"></span>
				</div>

				<form id="join-form" novalidate>
					<label for="room-code" class="room-code-label mb-2 block text-sm font-medium text-slate-300">4-character room code</label>
					<div class="flex gap-2.5">
						<input id="room-code" class="min-h-12 min-w-0 flex-1 rounded-2xl border border-slate-700 bg-slate-900 px-4 text-center font-mono text-xl font-semibold tracking-[0.22em] text-white uppercase focus:border-sky-400" maxlength="4" autocomplete="off" autocapitalize="characters" spellcheck="false" inputmode="text" aria-describedby="join-error" />
						<button id="join-room" class="join-button min-h-12 shrink-0 rounded-2xl border px-5 font-semibold transition active:scale-[0.98] disabled:cursor-not-allowed" disabled>Join</button>
					</div>
					<p id="join-error" class="mt-2 min-h-5 text-sm text-rose-300" role="alert"></p>
				</form>

				${githubReference("mt-7 flex w-fit items-center gap-1 text-xs font-medium")}
			</section>
		</div>
	`;

	const createButton = document.querySelector("#create-room");
	const form = document.querySelector("#join-form");
	const input = document.querySelector("#room-code");
	const joinButton = document.querySelector("#join-room");
	const joinError = document.querySelector("#join-error");

	createButton.addEventListener("click", async () => {
		createButton.disabled = true;
		createButton.textContent = "Creating…";
		try {
			await readJson(await fetch("/api/rooms", { method: "POST", credentials: "same-origin" }));
			navigate("/host");
		} catch {
			createButton.disabled = false;
			createButton.textContent = "Create Room";
			showToast("Could not create a room. Please try again.", true);
		}
	});

	input.addEventListener("input", () => {
		input.value = normalizeRoomCode(input.value);
		joinButton.disabled = input.value.length !== 4;
		joinError.textContent = "";
	});
	form.addEventListener("submit", (event) => {
		event.preventDefault();
		const code = normalizeRoomCode(input.value);
		if (!ROOM_CODE_PATTERN.test(code)) {
			joinError.textContent = "Enter a complete 4-character room code.";
			return;
		}
		navigate(`/room/${code}`);
	});
}

function renderMissingHost() {
	document.title = "Owner Session Unavailable · Stage 8 Helper";
	app.innerHTML = `
		<div class="flex min-h-dvh items-center justify-center px-4 py-8">
			<section class="w-full max-w-md rounded-[2rem] border border-slate-800 bg-slate-950/80 p-7 text-center shadow-2xl shadow-black/30">
				<p class="text-sm font-semibold text-amber-300">Owner session unavailable</p>
				<h1 class="mt-3 text-2xl font-semibold text-white">Owner access could not be restored</h1>
				<p class="mt-3 text-sm leading-6 text-slate-400">This browser has no active owner cookie, or its previous room lease has ended. Create a room to continue.</p>
				<button data-home class="mt-7 min-h-12 w-full rounded-2xl bg-sky-500 px-5 font-semibold text-slate-950 transition hover:bg-sky-400">Back to Home</button>
				${githubReference("mx-auto mt-5 flex w-fit items-center gap-1 text-xs font-medium")}
			</section>
		</div>
	`;
	document.querySelector("[data-home]").addEventListener("click", () => navigate("/"));
}

function renderRoomShell(code, isHost) {
	document.title = `${isHost ? "Owner" : "Viewer"} · ${code} · Stage 8 Helper`;
	const controls = isHost
		? `
			<div class="grid grid-cols-3 gap-2.5" aria-label="Owner controls">
				<button data-action="previous" class="min-h-12 rounded-2xl border border-slate-700 bg-slate-800 px-3 text-sm font-semibold text-slate-100 transition hover:border-slate-600 hover:bg-slate-700 active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-35">← Previous</button>
				<button data-action="reset" class="min-h-12 rounded-2xl border border-slate-700 bg-slate-900 px-3 text-sm font-semibold text-slate-300 transition hover:border-slate-600 hover:bg-slate-800 active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-35">Reset</button>
				<button data-action="next" class="min-h-12 rounded-2xl bg-sky-500 px-3 text-sm font-semibold text-slate-950 shadow-lg shadow-sky-950/30 transition hover:bg-sky-400 active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-35">Next →</button>
			</div>
			<p class="mt-2 text-center text-xs text-slate-500">The current instruction is copied after each action.</p>
		`
		: `
			<div class="rounded-2xl border border-slate-800 bg-slate-900/55 px-4 py-3 text-center text-sm text-slate-400">
				Viewer mode · Controlled by the room owner
			</div>
		`;

	app.innerHTML = `
		<div class="parchment room-shell min-h-dvh px-3 py-[max(1rem,env(safe-area-inset-top))] sm:px-6 sm:py-6">
			<div class="mx-auto w-full max-w-xl">
				<header class="mb-3 flex items-center justify-between gap-3">
					<button data-home class="min-h-11 rounded-xl px-2 text-sm font-medium text-slate-400 transition hover:text-white">← Back to Home</button>
					<div id="connection" class="flex items-center gap-2 rounded-full border border-slate-800 bg-slate-950/70 px-3 py-1.5 text-xs font-medium text-slate-400">
						<span class="size-2 rounded-full bg-amber-400" data-connection-dot></span>
						<span data-connection-label>Connecting</span>
					</div>
				</header>

				<section class="rounded-[1.75rem] border border-slate-800/90 bg-slate-950/75 p-3 shadow-2xl shadow-black/25 backdrop-blur sm:p-5">
					<div class="mb-4 flex items-start justify-between gap-4 px-1">
						<div>
							<div class="flex items-center gap-2">
								<span class="rounded-full ${isHost ? "bg-sky-400/15 text-sky-300" : "bg-slate-800 text-slate-300"} px-2.5 py-1 text-[0.65rem] font-bold tracking-wider uppercase">${isHost ? "Owner" : "Viewer"}</span>
								<span class="text-xs text-slate-600">Room</span>
							</div>
							<p class="mt-1 font-mono text-3xl font-bold tracking-[0.18em] text-white sm:text-4xl">${code}</p>
						</div>
						<div class="text-right">
							<p class="text-[0.65rem] font-semibold tracking-wider text-slate-600 uppercase">Progress</p>
							<p class="mt-1 text-xl font-semibold tabular-nums text-slate-200"><span id="state-number">—</span><span class="text-sm text-slate-600"> / ${STATE_COUNT}</span></p>
						</div>
					</div>

					<div class="mb-3 flex flex-wrap gap-x-3 gap-y-1 px-1 text-[0.65rem] font-semibold sm:text-xs" aria-label="Color legend">
						<span class="text-slate-500">● Empty</span>
						<span class="text-sky-300">● Occupied</span>
						<span class="text-rose-300">● Leave</span>
						<span class="text-emerald-300">● Enter</span>
					</div>

					<div class="stage-frame mx-auto w-full rounded-2xl border border-slate-800 bg-slate-900/55 p-2 sm:p-3">
						<canvas id="stage-layout" class="stage-canvas block h-auto w-full" width="268" height="119" role="img" aria-label="Loading the Stage 8 layout"></canvas>
					</div>

					<div class="mt-4 rounded-2xl border border-slate-800 bg-slate-900/65 p-3.5 sm:p-4">
						<p class="mb-1 text-[0.65rem] font-semibold tracking-wider text-slate-600 uppercase">Current Instruction</p>
						<p id="instruction" class="min-h-11 break-words text-sm leading-5 font-medium text-slate-200 sm:text-base sm:leading-6" aria-live="polite">Loading room state…</p>
					</div>

					<div class="mt-3">${controls}</div>
				</section>

				<button id="copy-invite" class="invite-button mt-3 min-h-11 w-full rounded-2xl border px-4 text-sm font-semibold transition active:scale-[0.99]">Copy Viewer Invite Link</button>
				${githubReference("mx-auto mt-4 flex w-fit items-center gap-1 text-xs font-medium")}
			</div>
		</div>
	`;

	document.querySelector("[data-home]").addEventListener("click", () => navigate("/"));
	document.querySelector("#copy-invite").addEventListener("click", async () => {
		const inviteUrl = new URL(`/room/${code}`, projectConfig.serviceOrigin).toString();
		const copied = await copyText(inviteUrl);
		showToast(copied ? "Viewer invite link copied" : "Clipboard unavailable", !copied);
	});
	for (const button of document.querySelectorAll("[data-action]")) {
		button.addEventListener("click", () => void runAction(button.dataset.action));
	}
}

function setConnectionStatus(kind, label) {
	const container = document.querySelector("#connection");
	if (!container) {
		return;
	}
	const dot = container.querySelector("[data-connection-dot]");
	container.querySelector("[data-connection-label]").textContent = label;
	dot.className = `size-2 rounded-full ${
		kind === "connected" ? "bg-emerald-400" : kind === "error" ? "bg-rose-400" : "bg-amber-400"
	}`;
}

function setActionsDisabled() {
	if (!runtime.isHost || !runtime.roomState) {
		return;
	}
	for (const button of document.querySelectorAll("[data-action]")) {
		const action = button.dataset.action;
		button.disabled =
			runtime.actionPending ||
			(action === "previous" && runtime.roomState.index === 0) ||
			(action === "next" && runtime.roomState.index === STATE_COUNT - 1);
	}
}

function scaledChannel(value, factor) {
	return Math.floor((value * factor) / 255);
}

function renderStageLayout(state) {
	const canvas = document.querySelector("#stage-layout");
	if (!canvas || !layoutSourcePixels) {
		return;
	}

	const output = new ImageData(
		new Uint8ClampedArray(layoutSourcePixels.data),
		LAYOUT_WIDTH,
		LAYOUT_HEIGHT,
	);
	const source = layoutSourcePixels.data;
	const occupied = new Set(state.occupied);

	for (let index = 0; index < CHAIR_POSITIONS.length; index += 1) {
		const box = index + 1;
		const visual =
			state.movement?.fromBox === box
				? "source"
				: state.movement?.toBox === box
					? "target"
					: occupied.has(box)
						? "occupied"
						: "empty";
		const [originX, originY] = CHAIR_POSITIONS[index];
		for (let y = originY; y < originY + CHAIR_HEIGHT; y += 1) {
			for (let x = originX; x < originX + CHAIR_WIDTH; x += 1) {
				const offset = (y * LAYOUT_WIDTH + x) * 4;
				if (source[offset + 3] === 0) {
					continue;
				}
				const gray = Math.floor(
					(source[offset] * 299 + source[offset + 1] * 587 + source[offset + 2] * 114) / 1000,
				);
				if (visual === "occupied") {
					output.data[offset] = scaledChannel(gray, 36);
					output.data[offset + 1] = scaledChannel(gray, 188);
					output.data[offset + 2] = gray;
				} else if (visual === "source") {
					output.data[offset] = gray;
					output.data[offset + 1] = scaledChannel(gray, 44);
					output.data[offset + 2] = scaledChannel(gray, 44);
				} else if (visual === "target") {
					output.data[offset] = scaledChannel(gray, 36);
					output.data[offset + 1] = gray;
					output.data[offset + 2] = scaledChannel(gray, 68);
				} else {
					const empty = scaledChannel(gray, 112);
					output.data[offset] = empty;
					output.data[offset + 1] = empty;
					output.data[offset + 2] = empty;
				}
			}
		}
	}

	const context = canvas.getContext("2d");
	context.clearRect(0, 0, LAYOUT_WIDTH, LAYOUT_HEIGHT);
	context.putImageData(output, 0, 0);
	const transition = state.movement
		? `; box ${state.movement.fromBox} leaves and box ${state.movement.toBox} enters`
		: "";
	canvas.setAttribute("aria-label", `Occupied boxes: ${state.occupied.join(", ")}${transition}`);
}

function updateRoomState(state) {
	if (!state || state.code !== runtime.code || !Array.isArray(state.occupied)) {
		return;
	}
	if (runtime.roomState && state.revision < runtime.roomState.revision) {
		return;
	}
	runtime.roomState = state;

	document.querySelector("#state-number").textContent = String(state.stateNumber);
	document.querySelector("#instruction").textContent = state.instruction;
	renderStageLayout(state);
	setActionsDisabled();
}

async function runAction(action) {
	if (!runtime.isHost || runtime.actionPending || !["previous", "reset", "next"].includes(action)) {
		return;
	}
	runtime.actionPending = true;
	setActionsDisabled();
	try {
		const response = await fetch(`/api/rooms/${runtime.code}/action`, {
			method: "POST",
			credentials: "same-origin",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ action }),
		});
		const payload = await readJson(response);
		updateRoomState(payload.state);
		const copied = await copyText(payload.state.instruction);
		showToast(copied ? "State synced and instruction copied" : "State synced, but the clipboard is unavailable", !copied);
	} catch (error) {
		if (error.status === 403) {
			showToast("Owner access has expired", true);
			navigate("/", true);
		} else {
			showToast("Action failed. Check your connection.", true);
		}
	} finally {
		runtime.actionPending = false;
		setActionsDisabled();
	}
}

function disconnectSocket() {
	clearTimeout(runtime.reconnectTimer);
	clearInterval(runtime.pingTimer);
	runtime.reconnectTimer = null;
	runtime.pingTimer = null;
	const socket = runtime.socket;
	runtime.socket = null;
	if (socket && socket.readyState < WebSocket.CLOSING) {
		socket.close(1000, "Leaving room");
	}
}

function connectSocket(routeId) {
	if (routeId !== runtime.routeId || !runtime.code) {
		return;
	}
	const protocol = location.protocol === "https:" ? "wss:" : "ws:";
	const socket = new WebSocket(`${protocol}//${location.host}/api/rooms/${runtime.code}/socket`);
	runtime.socket = socket;
	setConnectionStatus("connecting", "Connecting");

	socket.addEventListener("open", () => {
		if (runtime.socket !== socket) {
			return;
		}
		runtime.reconnectDelay = 500;
		setConnectionStatus("connected", "Synced");
		socket.send("sync");
		runtime.pingTimer = setInterval(() => {
			if (socket.readyState === WebSocket.OPEN) {
				socket.send("ping");
			}
		}, 25000);
	});
	socket.addEventListener("message", (event) => {
		if (event.data === "pong") {
			return;
		}
		try {
			const message = JSON.parse(event.data);
			if (message.type === "state") {
				updateRoomState(message.state);
			}
		} catch {
			// Ignore malformed messages and wait for the next complete snapshot.
		}
	});
	socket.addEventListener("close", () => {
		if (runtime.socket !== socket || routeId !== runtime.routeId) {
			return;
		}
		clearInterval(runtime.pingTimer);
		runtime.socket = null;
		setConnectionStatus("error", "Reconnecting");
		runtime.reconnectTimer = setTimeout(() => connectSocket(routeId), runtime.reconnectDelay);
		runtime.reconnectDelay = Math.min(runtime.reconnectDelay * 2, 10000);
	});
	socket.addEventListener("error", () => {
		setConnectionStatus("error", "Connection error");
	});
}

function activateRoom(code, isHost, state, routeId) {
	runtime.code = code;
	runtime.isHost = isHost;
	runtime.roomState = null;
	runtime.actionPending = false;
	renderRoomShell(code, isHost);
	updateRoomState(state);
	connectSocket(routeId);
}

async function openOwnerRoom(routeId) {
	document.title = "Restoring Owner Session · Stage 8 Helper";
	app.innerHTML = `
		<div class="flex min-h-dvh items-center justify-center px-4 py-8">
			<p class="text-sm font-semibold text-[#765d43]">Restoring owner session…</p>
		</div>
	`;
	try {
		const payload = await readJson(
			await fetch("/api/owner/room", { credentials: "same-origin" }),
		);
		if (routeId !== runtime.routeId) {
			return;
		}
		activateRoom(payload.state.code, true, payload.state, routeId);
	} catch {
		if (routeId === runtime.routeId) {
			renderMissingHost();
		}
	}
}

async function openViewerRoom(code, routeId) {
	runtime.code = code;
	runtime.isHost = false;
	runtime.roomState = null;
	runtime.actionPending = false;
	renderRoomShell(code, false);

	try {
		const payload = await readJson(await fetch(`/api/rooms/${code}`));
		if (routeId !== runtime.routeId) {
			return;
		}
		updateRoomState(payload.state);
		connectSocket(routeId);
	} catch (error) {
		if (routeId !== runtime.routeId) {
			return;
		}
		setConnectionStatus("error", "Room unavailable");
		document.querySelector("#instruction").textContent =
			error.status === 404 ? "Room not found. Check the room code." : "The room is temporarily unavailable. Please try again.";
		for (const button of document.querySelectorAll("[data-action]")) {
			button.disabled = true;
		}
	}
}

async function route() {
	const routeId = ++runtime.routeId;
	disconnectSocket();
	runtime.code = null;
	runtime.isHost = false;
	runtime.roomState = null;

	const path = location.pathname.replace(/\/+$/, "") || "/";
	if (path === "/") {
		renderLobby();
		return;
	}
	if (path === "/host") {
		await openOwnerRoom(routeId);
		return;
	}

	const roomMatch = path.match(/^\/room\/([0-9A-Za-z]{4})$/);
	if (roomMatch) {
		const code = normalizeRoomCode(roomMatch[1]);
		if (path !== `/room/${code}`) {
			history.replaceState(null, "", `/room/${code}`);
		}
		await openViewerRoom(code, routeId);
		return;
	}

	navigate("/", true);
}

window.addEventListener("popstate", () => void route());
window.addEventListener("keydown", (event) => {
	if (!runtime.isHost || event.metaKey || event.ctrlKey || event.altKey) {
		return;
	}
	const target = event.target;
	if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target?.isContentEditable) {
		return;
	}
	const action = event.key === "PageUp" ? "previous" : event.key === "PageDown" ? "next" : event.key === "Home" ? "reset" : null;
	if (action) {
		event.preventDefault();
		void runAction(action);
	}
});

async function initialize() {
	await loadProjectConfig();
	await route();
}

void initialize();
