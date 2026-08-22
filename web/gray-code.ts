export const STATE_COUNT = 126;

const BOX_COUNT = 9;
const OCCUPIED_COUNT = 5;
const INITIAL_MASK = (1 << OCCUPIED_COUNT) - 1;
const LOGICAL_TO_BOX = [4, 2, 7, 5, 3, 6, 8, 9, 1] as const;

export interface Movement {
	fromBox: number;
	toBox: number;
}

export interface StageState {
	index: number;
	stateNumber: number;
	stateCount: number;
	occupied: number[];
	movement: Movement | null;
	instruction: string;
}

function bitCount(value: number): number {
	let remaining = value;
	let count = 0;
	while (remaining !== 0) {
		count += remaining & 1;
		remaining >>>= 1;
	}
	return count;
}

function remapToBoxes(logicalMask: number): number {
	let mask = 0;
	for (let logicalIndex = 0; logicalIndex < LOGICAL_TO_BOX.length; logicalIndex += 1) {
		if ((logicalMask & (1 << logicalIndex)) !== 0) {
			mask |= 1 << (LOGICAL_TO_BOX[logicalIndex] - 1);
		}
	}
	return mask;
}

function generateStates(): readonly number[] {
	const states: number[] = [];
	for (let binary = 0; binary < 1 << BOX_COUNT; binary += 1) {
		const gray = binary ^ (binary >> 1);
		if (bitCount(gray) === OCCUPIED_COUNT) {
			states.push(remapToBoxes(gray));
		}
	}

	const start = states.indexOf(INITIAL_MASK);
	if (start < 0) {
		throw new Error("Gray-code sequence is missing its initial state");
	}

	const rotated = [...states.slice(start), ...states.slice(0, start)];
	const result = [rotated[0], ...rotated.slice(1).reverse()];
	validateStates(result);
	return Object.freeze(result);
}

function validateStates(states: readonly number[]): void {
	if (states.length !== STATE_COUNT || new Set(states).size !== STATE_COUNT) {
		throw new Error("Gray-code sequence must contain 126 unique states");
	}
	if (states[0] !== INITIAL_MASK) {
		throw new Error("Gray-code sequence must start with boxes 1-5 occupied");
	}
	for (let index = 1; index < states.length; index += 1) {
		if (bitCount(states[index - 1] ^ states[index]) !== 2) {
			throw new Error("Adjacent Gray-code states must exchange exactly one box");
		}
	}
	const firstMove = movementBetween(states[0], states[1]);
	if (firstMove.fromBox !== 4 || firstMove.toBox !== 7) {
		throw new Error("Gray-code sequence must begin with move 4 to 7");
	}
}

function boxesForMask(mask: number): number[] {
	const boxes: number[] = [];
	for (let box = 1; box <= BOX_COUNT; box += 1) {
		if ((mask & (1 << (box - 1))) !== 0) {
			boxes.push(box);
		}
	}
	return boxes;
}

export function movementBetween(beforeMask: number, afterMask: number): Movement {
	const changed = beforeMask ^ afterMask;
	if (bitCount(changed) !== 2) {
		throw new Error("A movement must exchange exactly one occupied box");
	}
	const source = changed & beforeMask;
	const target = changed & afterMask;
	return {
		fromBox: Math.log2(source) + 1,
		toBox: Math.log2(target) + 1,
	};
}

export const STAGE_STATES = generateStates();

export function createStageState(index: number, movement: Movement | null = null): StageState {
	if (!Number.isInteger(index) || index < 0 || index >= STATE_COUNT) {
		throw new RangeError(`State index must be between 0 and ${STATE_COUNT - 1}`);
	}

	const occupied = boxesForMask(STAGE_STATES[index]);
	const resolvedMovement =
		movement ?? (index > 0 ? movementBetween(STAGE_STATES[index - 1], STAGE_STATES[index]) : null);
	const instruction = resolvedMovement
		? `State ${index + 1}/${STATE_COUNT} · Occupied {${occupied.join(",")}} · Move box ${resolvedMovement.fromBox} to box ${resolvedMovement.toBox}`
		: `State 1/${STATE_COUNT} · Occupied {${occupied.join(",")}} · Initial setup`;

	return {
		index,
		stateNumber: index + 1,
		stateCount: STATE_COUNT,
		occupied,
		movement: resolvedMovement,
		instruction,
	};
}
